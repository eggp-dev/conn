//! Cocoa commands enter the same platform-neutral automation service as future adapters.
use conn_frontend::{automation::Caller, Harness};
use serde_json::{json, Value};
use std::{
    ffi::{c_char, CStr, CString},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock, Weak,
    },
};
struct Adapter {
    harness: Weak<Harness>,
    app: tauri::AppHandle,
}
static ADAPTER: OnceLock<Adapter> = OnceLock::new();
static WINDOW_SEQUENCE: AtomicU64 = AtomicU64::new(1);
extern "C" {
    fn conn_script_link();
    fn conn_script_sender_alive(identity: *const c_char) -> bool;
}
pub fn install(app: &tauri::AppHandle, harness: &Arc<Harness>) {
    let _ = ADAPTER.set(Adapter {
        harness: Arc::downgrade(harness),
        app: app.clone(),
    });
    unsafe {
        conn_script_link();
    }
}
#[no_mangle]
pub unsafe extern "C" fn conn_automation_call(payload: *const c_char) -> *mut c_char {
    let result = std::panic::catch_unwind(|| -> Result<Value, String> {
        if payload.is_null() {
            return Err("Missing request".into());
        }
        let bytes = unsafe { CStr::from_ptr(payload) }.to_bytes();
        if bytes.len() > 65_536 {
            return Err("Request too large".into());
        }
        let request: Value = serde_json::from_slice(bytes).map_err(|_| "Invalid native request")?;
        let get = |key| {
            request
                .get(key)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("Missing {key}"))
        };
        let adapter = ADAPTER.get().ok_or("Conn is starting or closing")?;
        let harness = adapter.harness.upgrade().ok_or("Conn is closing")?;
        let operation = get("operation")?;
        let new_window = operation == "window.create";
        let label = if new_window {
            format!(
                "automation-{}",
                WINDOW_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            )
        } else {
            crate::windows::active_label(&adapter.app)
        };
        let identity = get("identity")?.to_owned();
        let owner = identity.clone();
        let caller = Caller {
            identity,
            name: get("name")?.into(),
            still_alive: Arc::new(move || CString::new(owner.as_str()).ok()
                .is_some_and(|id| unsafe { conn_script_sender_alive(id.as_ptr()) })),
        };
        if new_window { harness.prepare_automation_window(&label)?; }
        if new_window || operation == "session.create" {
            if crate::windows::show(&adapter.app, &label).is_err() {
                if new_window { crate::windows::discard(&adapter.app, &label); harness.close_window(&label); }
                return Err("Window unavailable".into());
            }
        }
        let result = match harness.automate_in_window(&label, caller, operation, request["params"].clone()) {
            Ok(value) => value,
            Err(error) => {
                if new_window { crate::windows::discard(&adapter.app, &label); harness.close_window(&label); }
                return Err(error);
            }
        };
        // Existing launcher templates ignore the result. It is an opaque session ID,
        // not iTerm's window object model.
        Ok(if new_window {
            result["session"].clone()
        } else {
            result
        })
    });
    let reply = match result {
        Ok(Ok(v)) => json!({"result":v}),
        Ok(Err(e)) => json!({"error":e}),
        Err(_) => json!({"error":"Automation failed"}),
    };
    CString::new(reply.to_string())
        .unwrap_or_default()
        .into_raw()
}
#[no_mangle]
pub unsafe extern "C" fn conn_automation_free(payload: *mut c_char) {
    if !payload.is_null() {
        drop(unsafe { CString::from_raw(payload) });
    }
}
