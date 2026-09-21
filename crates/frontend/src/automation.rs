//! Native external automation. Private sessions never enter agent activity paths.
use crate::{AppHandle, AppState};
use conn_core::{LaunchSpec, session::SharedSession};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Weak};
use std::time::{Duration, Instant};

const MAX_REQUESTS: usize = 256;
const MAX_QUEUE: usize = 16;
const TIMEOUT: Duration = Duration::from_secs(120);

/// Constructed by the native adapter using OS sender metadata, not script arguments.
#[derive(Clone)]
pub struct Caller {
    pub identity: String,
    pub name: String,
    pub still_alive: Arc<dyn Fn() -> bool + Send + Sync>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config { pub enabled: bool, pub profiles: Vec<String>, #[serde(default)] pub linux_executables: Vec<String> }
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedConfig { version: u32, enabled: bool, profiles: Vec<String>, #[serde(default)] linux_executables: Vec<String> }

struct Payload(Vec<u8>);
impl Drop for Payload {
    fn drop(&mut self) {
        for byte in &mut self.0 { unsafe { std::ptr::write_volatile(byte, 0); } }
    }
}
struct Binding {
    owner: String,
    window: String,
    session_id: String,
    handle: String,
    session: SharedSession,
    alive: Arc<dyn Fn() -> bool + Send + Sync>,
    stopped: AtomicBool,
    operation: Mutex<()>,
    queue: Mutex<VecDeque<Arc<Job>>>,
    wake: parking_lot::Condvar,
}
impl Binding {
    fn stop(&self) {
        let _operation = self.operation.lock();
        self.stop_locked();
    }
    // Caller holds operation; no worker can deliver a chunk during the transition.
    fn stop_locked(&self) {
        self.stopped.store(true, Ordering::Release);
        self.session.lock().revoke_external();
        for job in self.queue.lock().drain(..) {
            job.payload.lock().take();
            job.update("cancelled", Some("input_revoked"));
        }
        self.wake.notify_all();
    }
}
struct Job {
    id: String,
    owner: String,
    handle: String,
    binding: Weak<Binding>,
    fingerprint: [u8; 32],
    payload: Mutex<Option<Payload>>,
    created: Instant,
    cancelled: AtomicBool,
    result: Mutex<Value>,
}
impl Job {
    fn update(&self, state: &str, error: Option<&str>) {
        *self.result.lock() = json!({"requestId":self.id,"session":self.handle,"state":state,"error":error});
    }
    fn done(&self) -> bool {
        matches!(self.result.lock()["state"].as_str(), Some("delivered" | "cancelled" | "failed"))
    }
}

pub(crate) struct Automation {
    config: Mutex<Config>,
    requires_reenable: AtomicBool,
    bindings: Mutex<HashMap<String, Arc<Binding>>>,
    requests: Mutex<HashMap<String, Arc<Job>>>,
    digest_key: [u8; 32],
}
impl Automation {
    pub(crate) fn load(dir: &std::path::Path) -> Self {
        let value: Value = std::fs::read(dir.join("automation.json")).ok()
            .and_then(|v| serde_json::from_slice(&v).ok()).unwrap_or(Value::Null);
        let current = value["version"] == 2;
        let enabled = value["enabled"] == true;
        let profiles = serde_json::from_value(value["profiles"].clone()).unwrap_or_default();
        let mut digest_key = [0; 32];
        digest_key[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
        digest_key[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
        Self { config: Mutex::new(Config { enabled: current && enabled, profiles, linux_executables: serde_json::from_value(value["linuxExecutables"].clone()).unwrap_or_default() }),
            requires_reenable: AtomicBool::new(!current && enabled), bindings: Default::default(),
            requests: Default::default(), digest_key }
    }
    pub(crate) fn allows_linux_executable(&self, executable: &std::path::Path) -> bool {
        let config = self.config.lock();
        config.enabled && config.linux_executables.iter().any(|p| {
            std::path::Path::new(p).is_absolute()
                && std::fs::canonicalize(p).ok().as_deref() == Some(executable)
        })
    }
    pub(crate) fn settings(&self) -> Value {
        json!({"config":self.config.lock().clone(),"nativeSupported":cfg!(any(target_os="macos", target_os="linux")),"linuxSupported":cfg!(target_os="linux"),
            "requiresReenable":self.requires_reenable.load(Ordering::Acquire)})
    }
    pub(crate) fn stop_all(&self) {
        let bindings = std::mem::take(&mut *self.bindings.lock());
        for b in bindings.into_values() { b.stop(); }
    }
    pub(crate) fn stop_session(&self, id: &str) {
        let stopped = {
            let mut bindings = self.bindings.lock();
            let handles: Vec<_> = bindings.iter().filter(|(_, b)| b.session_id == id).map(|(handle, _)| handle.clone()).collect();
            handles.into_iter().filter_map(|handle| bindings.remove(&handle)).collect::<Vec<_>>()
        };
        for b in stopped { b.stop(); }
    }
    /// Fence in-flight external writes, commit the session change, then drain.
    /// A failed precondition leaves writer handles and queued payloads untouched.
    pub(crate) fn transition_session<T>(&self, id: &str, commit: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        let mut bindings: Vec<_> = self.bindings.lock().values().filter(|b| b.session_id == id).cloned().collect();
        bindings.sort_by(|a,b| a.handle.cmp(&b.handle));
        let _operations: Vec<_> = bindings.iter().map(|b| b.operation.lock()).collect();
        let result = commit()?;
        for binding in &bindings { binding.stop_locked(); }
        let mut current = self.bindings.lock();
        for binding in &bindings { current.remove(&binding.handle); }
        Ok(result)
    }
    fn stop_binding(&self, binding: &Binding) {
        self.bindings.lock().remove(&binding.handle);
        binding.stop();
    }
}

pub(crate) fn save(state: &AppState, config: Config) -> Result<Value, String> {
    crate::cancel_all_pending(state);
    if config.linux_executables.len() > 32 || config.linux_executables.iter().any(|p| {
        let path = std::path::Path::new(p);
        !path.is_absolute() || !path.is_file() || p.len() > 4096
    }) { return Err("Choose at most 32 existing absolute executable paths".into()); }
    let mut current = state.automation.config.lock();
    let profiles = conn_core::profiles::Profiles::load(&state.config_dir.join("profiles.json"))?;
    if config.profiles.iter().any(|id| !profiles.profiles.iter().any(|p| &p.id == id)) {
        return Err("Unknown automation profile".into());
    }
    std::fs::create_dir_all(&state.config_dir).map_err(|_| "Cannot save automation settings")?;
    let mut file = tempfile::NamedTempFile::new_in(&state.config_dir).map_err(|_| "Cannot save automation settings")?;
    use std::io::Write;
    let saved = SavedConfig { version: 2, enabled: config.enabled, profiles: config.profiles.clone(), linux_executables: config.linux_executables.clone() };
    file.write_all(&serde_json::to_vec(&saved).map_err(|_| "Cannot save automation settings")?)
        .map_err(|_| "Cannot save automation settings")?;
    file.persist(state.config_dir.join("automation.json")).map_err(|_| "Cannot save automation settings")?;
    state.automation.stop_all();
    *current = config;
    state.automation.requires_reenable.store(false, Ordering::Release);
    drop(current);
    Ok(state.automation.settings())
}

fn validate_text(text: &str) -> Result<(), String> {
    if text.len() > 16_384 || text.chars().any(char::is_control) {
        return Err("Text must be one line without control characters (maximum 16 KiB)".into());
    }
    Ok(())
}
fn field(args: &Value, key: &str) -> Result<String, String> {
    args.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned)
        .ok_or_else(|| format!("Missing {key}"))
}
fn binding(state: &AppState, caller: &Caller, id: &str) -> Result<Arc<Binding>, String> {
    state.automation.bindings.lock().get(id)
        .filter(|b| b.owner == caller.identity && !b.stopped.load(Ordering::Acquire))
        .cloned().ok_or_else(|| "Session unavailable for this caller".into())
}
fn request(state: &AppState, caller: &Caller, id: &str) -> Result<Arc<Job>, String> {
    state.automation.requests.lock().get(id).filter(|j| j.owner == caller.identity)
        .cloned().ok_or_else(|| "Unknown request for this caller".into())
}

/// POSIX word splitting only. Expansion/operators require an explicitly supplied shell.
fn launch_spec(command: &str) -> Result<LaunchSpec, String> {
    validate_text(command)?;
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = None;
    let mut started = false;
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        match quoted {
            Some('\'') => { if c == '\'' { quoted = None; } else { word.push(c); } },
            Some('"') => {
                if c == '"' { quoted = None; }
                else if c == '\\' {
                    let next = chars.next().ok_or("Invalid command quoting")?;
                    if !matches!(next, '$' | '`' | '"' | '\\') { word.push('\\'); }
                    word.push(next);
                } else { word.push(c); }
            },
            _ => match c {
                '\'' | '"' => { quoted = Some(c); started = true; },
                '\\' => { word.push(chars.next().ok_or("Invalid command quoting")?); started = true; },
                ' ' => { if started { words.push(std::mem::take(&mut word)); started = false; } },
                ';' | '&' | '|' | '<' | '>' | '(' | ')' | '`' => return Err("Shell operators require an explicit shell argument".into()),
                _ => { word.push(c); started = true; },
            }
        }
    }
    if quoted.is_some() { return Err("Invalid command quoting".into()); }
    if started { words.push(word); }
    if words.is_empty() || words[0].is_empty() { return Err("Missing executable".into()); }
    Ok(LaunchSpec::Program { executable: words.remove(0), argv: words })
}

// HMAC-SHA256 with a per-run random key; never persist payload fingerprints.
fn fingerprint(key: &[u8; 32], owner: &str, session: &str, text: &str, newline: bool) -> [u8; 32] {
    let mut inner_pad = [0x36; 64]; let mut outer_pad = [0x5c; 64];
    for (i, b) in key.iter().enumerate() { inner_pad[i] ^= b; outer_pad[i] ^= b; }
    let mut inner = Sha256::new(); inner.update(inner_pad);
    for part in [owner.as_bytes(), session.as_bytes(), text.as_bytes()] {
        inner.update((part.len() as u64).to_le_bytes()); inner.update(part);
    }
    inner.update([u8::from(newline)]);
    let mut outer = Sha256::new(); outer.update(outer_pad); outer.update(inner.finalize());
    outer.finalize().into()
}

pub(crate) fn dispatch(app: &AppHandle, state: &Arc<AppState>, window: &str, caller: Caller, operation: &str, mut args: Value) -> Result<Value, String> {
    if caller.identity.is_empty() || caller.name.is_empty() || caller.name.len() > 256 || !(caller.still_alive)() {
        return Err("Native caller unavailable".into());
    }
    // Deprecated presentation metadata is deliberately never retained.
    if let Some(object) = args.as_object_mut() { object.remove("intent"); }
    match operation {
        "window.create" | "session.create" => {
            let launch = match args.get("command") {
                None | Some(Value::Null) => LaunchSpec::ProfileDefault,
                Some(Value::String(s)) if s.is_empty() => LaunchSpec::ProfileDefault,
                Some(Value::String(s)) => launch_spec(s)?,
                _ => return Err("Command must be text".into()),
            };
            // Native adapter presents the reserved window first. No child runs unseen.
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                if !state.windows.lock().available(window) || !(caller.still_alive)() { return Err("Window unavailable".into()); }
                if state.windows.lock().ready(window) { break; }
                if Instant::now() >= deadline { return Err("Window not ready".into()); }
                std::thread::sleep(Duration::from_millis(20));
            }
            let config = state.automation.config.lock();
            if !config.enabled { return Err("Enable external automation in Settings → Automation first".into()); }
            let _startup = state.startup.lock();
            let profiles = conn_core::profiles::Profiles::load(&state.config_dir.join("profiles.json"))
                .map_err(|_| "Profile unavailable")?;
            let profile = args.get("profile").and_then(Value::as_str).filter(|v| !v.is_empty()).unwrap_or(&profiles.default_profile);
            if !config.profiles.iter().any(|id| id == profile) { return Err("Profile is not enabled for automation".into()); }
            if state.automation.bindings.lock().values().filter(|b| !b.stopped.load(Ordering::Acquire) && b.session.lock().process_alive()).count() >= 32 {
                return Err("Too many automation sessions".into());
            }
            crate::ensure_runtime(app, state).map_err(|_| "Cannot initialize terminal")?;
            if !(caller.still_alive)() || !state.windows.lock().ready(window) { return Err("External session unavailable".into()); }
            let id = crate::spawn_tab_with(app, state, window, 24, 80, Some(profile), true, false, launch, Some(caller.still_alive.as_ref()))
                .map_err(|_| "Cannot start external session")?;
            let session = state.hub.get(&id).ok_or("Session unavailable")?;
            let handle = uuid::Uuid::new_v4().to_string();
            let b = Arc::new(Binding { owner: caller.identity, window: window.into(), session_id: id.clone(), handle: handle.clone(), session,
                alive: caller.still_alive, stopped: AtomicBool::new(false), operation: Default::default(), queue: Default::default(), wake: Default::default() });
            state.automation.bindings.lock().insert(handle.clone(), b.clone());
            { let mut windows = state.windows.lock(); windows.finish_prepare(window); windows.select(window, &id); }
            state.hub.set_attended(&id);
            let weak = Arc::downgrade(state);
            std::thread::spawn(move || worker(weak, b));
            app.emit("ss:tab_opened", json!({"session":id,"focus":true,"shared":false,"externalOrigin":true}))?;
            Ok(if operation == "window.create" { json!({"session":handle,"requestId":null}) } else { json!(handle) })
        },
        "session.write" => {
            let id = field(&args, "session")?;
            let b = binding(state, &caller, &id)?;
            let text = field(&args, "text")?; validate_text(&text)?;
            let newline = args.get("newline").and_then(Value::as_bool).unwrap_or(true);
            let rid = args.get("requestId").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            if rid.is_empty() || rid.len() > 128 || rid.chars().any(char::is_control) { return Err("Invalid request ID".into()); }
            let digest = fingerprint(&state.automation.digest_key, &caller.identity, &id, &text, newline);
            let _operation = b.operation.lock();
            if b.stopped.load(Ordering::Acquire) || !b.session.lock().external_writer_active() { return Err("External input revoked".into()); }
            let mut requests = state.automation.requests.lock();
            if let Some(j) = requests.get(&rid) {
                return if j.owner == caller.identity && j.handle == id && j.fingerprint == digest { Ok(json!(rid)) }
                    else { Err("Request ID conflicts with another request".into()) };
            }
            if requests.len() >= MAX_REQUESTS { return Err("Request capacity reached; restart Conn before submitting more automation".into()); }
            let mut queue = b.queue.lock();
            if queue.len() >= MAX_QUEUE { return Err("Session request queue is full".into()); }
            let mut bytes = text.into_bytes(); if newline { bytes.push(b'\r'); }
            let job = Arc::new(Job { id: rid.clone(), owner: caller.identity.clone(), handle: b.handle.clone(), binding: Arc::downgrade(&b), fingerprint: digest, payload: Mutex::new(Some(Payload(bytes))), created: Instant::now(), cancelled: AtomicBool::new(false), result: Mutex::new(Value::Null) });
            job.update("queued", None); requests.insert(rid.clone(), job.clone()); queue.push_back(job); b.wake.notify_one();
            Ok(json!(rid))
        },
        "request.status" => Ok(request(state, &caller, &field(&args, "requestId")?)?.result.lock().clone()),
        "request.cancel" => {
            let job = request(state, &caller, &field(&args, "requestId")?)?;
            if job.done() { return Ok(json!(false)); }
            job.cancelled.store(true, Ordering::Release);
            if let Some(binding) = job.binding.upgrade() { state.automation.stop_binding(&binding); }
            else { job.payload.lock().take(); job.update("cancelled", Some("input_revoked")); }
            Ok(json!(true))
        },
        "session.release" => { let b = binding(state, &caller, &field(&args, "session")?)?; state.automation.stop_binding(&b); Ok(json!(true)) },
        "session.status" => {
            let b = binding(state, &caller, &field(&args, "session")?)?;
            let s = b.session.lock();
            Ok(json!({"session":b.handle,"processAlive":s.process_alive(),"shared":false,"externalOrigin":true,"inputAvailable":s.external_writer_active()}))
        },
        _ => Err("Unsupported automation operation".into()),
    }
}

fn worker(state: Weak<AppState>, binding: Arc<Binding>) {
    loop {
        let job = {
            let mut queue = binding.queue.lock();
            if queue.is_empty() && !binding.stopped.load(Ordering::Acquire) { binding.wake.wait_for(&mut queue, Duration::from_millis(100)); }
            queue.pop_front()
        };
        let Some(job) = job else {
            if binding.stopped.load(Ordering::Acquire) || state.strong_count() == 0 || !(binding.alive)() || !binding.session.lock().external_writer_active() {
                if let Some(state) = state.upgrade() { state.automation.stop_binding(&binding); } else { binding.stop(); }
                break;
            }
            continue;
        };
        let payload = job.payload.lock().take();
        let result = payload.ok_or("input_revoked").and_then(|bytes| {
            job.update("delivering", None);
            for chunk in bytes.0.chunks(1024) {
                let _operation = binding.operation.lock();
                if job.cancelled.load(Ordering::Acquire) || binding.stopped.load(Ordering::Acquire) || job.created.elapsed() >= TIMEOUT || !(binding.alive)() {
                    return Err("input_revoked");
                }
                let Some(state) = state.upgrade() else { return Err("session_unavailable"); };
                if !state.windows.lock().ready(&binding.window) { return Err("window_unavailable"); }
                binding.session.lock().write_external(chunk).map_err(|_| "input_revoked")?;
            }
            Ok(())
        });
        match result {
            Ok(()) => job.update("delivered", None),
            Err(code) => {
                job.update("cancelled", Some(code));
                if let Some(state) = state.upgrade() { state.automation.stop_binding(&binding); } else { binding.stop(); }
            },
        }
    }
}

#[cfg(all(test, unix))]
mod tests;
