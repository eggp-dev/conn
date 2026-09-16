//! Cocoa commands enter the same platform-neutral automation service as future adapters.
use conn_frontend::{automation::Caller, Harness};
use serde_json::{json, Value};
use std::{
    ffi::{c_char, CStr, CString},
    sync::{Arc, OnceLock, Weak},
};
static HARNESS: OnceLock<Weak<Harness>> = OnceLock::new();
extern "C" {
    fn conn_script_link();
}
pub fn install(harness: &Arc<Harness>) {
    let _ = HARNESS.set(Arc::downgrade(harness));
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
        let harness = HARNESS
            .get()
            .and_then(Weak::upgrade)
            .ok_or("Conn is starting or closing")?;
        harness.automate(
            Caller {
                identity: get("identity")?.into(),
                name: get("name")?.into(),
            },
            get("operation")?,
            request["params"].clone(),
        )
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
