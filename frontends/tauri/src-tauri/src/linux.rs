//! D-Bus adapter. Sender credentials come from the bus, never request JSON.
use conn_frontend::{automation::Caller, Harness};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Weak,
    },
};
use zbus::{fdo, message::Header, Connection};
static SEQUENCE: AtomicU64 = AtomicU64::new(1);
struct Adapter {
    app: tauri::AppHandle,
    harness: Weak<Harness>,
    inflight: Arc<AtomicUsize>,
}
struct Permit(Arc<AtomicUsize>);
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Release);
    }
}
fn denied() -> fdo::Error {
    fdo::Error::AccessDenied("External caller is not authorized".into())
}
fn process(pid: u32) -> Option<(PathBuf, String)> {
    let root = PathBuf::from(format!("/proc/{pid}"));
    let exe = std::fs::read_link(root.join("exe")).ok()?;
    let stat = std::fs::read_to_string(root.join("stat")).ok()?;
    let start = stat
        .rsplit_once(") ")?
        .1
        .split_whitespace()
        .nth(19)?
        .to_owned();
    Some((exe, start))
}
#[zbus::interface(name = "dev.eggp.Conn.Automation1")]
impl Adapter {
    async fn call(
        &self,
        operation: &str,
        payload: &str,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<String> {
        if payload.len() > 65_536
            || !matches!(
                operation,
                "window.create"
                    | "session.create"
                    | "session.write"
                    | "session.status"
                    | "session.release"
                    | "request.status"
                    | "request.cancel"
            )
        {
            return Err(fdo::Error::InvalidArgs("Invalid automation request".into()));
        }
        let old = self.inflight.fetch_add(1, Ordering::AcqRel);
        let permit = Permit(self.inflight.clone());
        if old >= 8 {
            return Err(fdo::Error::LimitsExceeded("Too many requests".into()));
        }
        let sender = header.sender().ok_or_else(denied)?.to_owned();
        let bus = fdo::DBusProxy::new(conn).await?;
        if bus.get_connection_unix_user(sender.clone().into()).await? != unsafe { libc::geteuid() }
        {
            return Err(denied());
        }
        let pid = bus
            .get_connection_unix_process_id(sender.clone().into())
            .await?;
        let (exe, start) = process(pid).ok_or_else(denied)?;
        let harness = self.harness.upgrade().ok_or_else(denied)?;
        if !harness.linux_automation_allowed(&exe) {
            return Err(denied());
        }
        let args: Value = serde_json::from_str(payload)
            .map_err(|_| fdo::Error::InvalidArgs("Invalid request JSON".into()))?;
        let app = self.app.clone();
        let operation = operation.to_owned();
        let connection = zbus::blocking::Connection::from(conn.clone());
        let owner = format!("linux:{sender}:{pid}:{start}");
        tauri::async_runtime::spawn_blocking(move || {
            let _permit = permit;
            let label = if matches!(operation.as_str(), "window.create" | "session.create") {
                format!("automation-{}", SEQUENCE.fetch_add(1, Ordering::Relaxed))
            } else {
                crate::windows::active_label(&app)
            };
            let creating = matches!(operation.as_str(), "window.create" | "session.create");
            let caller = Caller {
                identity: owner,
                name: "External application".into(),
                still_alive: Arc::new(move || {
                    if process(pid).as_ref() != Some(&(exe.clone(), start.clone())) {
                        return false;
                    }
                    zbus::blocking::fdo::DBusProxy::new(&connection)
                        .ok()
                        .and_then(|b| b.name_has_owner(sender.clone().into()).ok())
                        .unwrap_or(false)
                }),
            };
            if creating {
                harness.prepare_automation_window(&label)?;
                if crate::windows::show(&app, &label).is_err() {
                    crate::windows::discard(&app, &label);
                    harness.close_window(&label);
                    return Err("Window unavailable".to_owned());
                }
            }
            let result = harness.automate_in_window(&label, caller, &operation, args);
            if creating && result.is_err() {
                crate::windows::discard(&app, &label);
                harness.close_window(&label);
            }
            result.map(|v| v.to_string())
        })
        .await
        .map_err(|_| fdo::Error::Failed("Automation unavailable".into()))?
        .map_err(fdo::Error::Failed)
    }
}
pub fn install(app: &tauri::AppHandle, harness: &Arc<Harness>) {
    let adapter = Adapter {
        app: app.clone(),
        harness: Arc::downgrade(harness),
        inflight: Arc::new(AtomicUsize::new(0)),
    };
    std::thread::spawn(move || {
        let result = zbus::blocking::connection::Builder::session()
            .and_then(|b| b.name("dev.eggp.Conn"))
            .and_then(|b| b.serve_at("/dev/eggp/Conn/Automation", adapter))
            .and_then(|b| b.build());
        match result {
            Ok(connection) => connection.closed(),
            Err(_) => eprintln!("Conn: Linux automation unavailable"),
        }
    });
}
