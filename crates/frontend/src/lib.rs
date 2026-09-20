//! Tauri side: one engine per tab, all behind one hub/socket. Output and lifecycle
//! events reach the webview tagged with the session id.

mod agent_setup;
mod integrations;
mod diagnostics;
pub mod automation;
mod windows;
mod surface_commands;
mod extension_commands;
mod extensions;

mod updates;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde_json::{json, Value};
use conn_core::affordance::Affordance;
use conn_core::approval::Decision;
use conn_core::ipc::{AdmissionPolicy, Hub, ServerGuard, SharedHub};
use conn_core::session::{AgentMode, ServerEvent};
use conn_core::{Engine, EngineConfig, Pacing, LaunchSpec};
use std::path::PathBuf;
use conn_core::backend::{Profile, Availability};

struct PendingSession {
    profile_id: String,
    profile_name: String,
    rows: u16,
    cols: u16,
    attached: bool,
    cancelled: bool,
}

struct AppState {
    pending_sessions: parking_lot::Mutex<HashMap<String, PendingSession>>,
    startup: parking_lot::Mutex<()>,
    automation: automation::Automation,
    extensions: extensions::Extensions,
    windows: parking_lot::Mutex<windows::Windows>,
    output: parking_lot::Mutex<HashMap<String, Option<Vec<Value>>>>,
    integration_home: Option<PathBuf>,
    config_dir: PathBuf,
    socket: PathBuf,
    engines: parking_lot::Mutex<HashMap<String, Arc<Engine>>>,
    hub: SharedHub,
    server: parking_lot::Mutex<Option<ServerGuard>>,
    seq: parking_lot::Mutex<u32>,
    /// Settings applied to every new tab: `{mode, gate, pacing, mask}` (all optional).
    defaults: parking_lot::Mutex<Value>,
}

pub type Emit = Arc<dyn Fn(&str, Value) + Send + Sync>;
#[derive(Clone)]
struct AppHandle { state: std::sync::Weak<AppState>, emit: Emit }
impl AppHandle {
    fn emit(&self, name: &str, mut value: Value) -> Result<(), String> {
        if let Some(session) = value["session"].as_str() {
            let owner = self.state.upgrade().and_then(|s| s.windows.lock().owner(session));
            let Some(owner) = owner else { return Ok(()); };
            value["window"] = json!(owner);
        }
        (self.emit)(name, value); Ok(())
    }
}

pub struct Harness { state: Arc<AppState>, app: AppHandle }
impl Harness {
    pub fn new(config_dir: PathBuf, socket: PathBuf, emit: Emit) -> Self {
        Self::with_setup_home(config_dir, socket, emit, None)
    }
    /// The browser harness uses the same installer against disposable client files.
    pub fn with_setup_home(config_dir: PathBuf, socket: PathBuf, emit: Emit, integration_home: Option<PathBuf>) -> Self {
        let defaults = std::fs::read_to_string(config_dir.join("app.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(json!({}));
        let automation = automation::Automation::load(&config_dir);
        let extensions = extensions::Extensions::load(&config_dir);
        let state = Arc::new(AppState { pending_sessions: Default::default(), startup: Default::default(), automation, extensions, windows: Default::default(), output: Default::default(), integration_home, config_dir, socket, engines: Default::default(), hub: Hub::new(), server: Default::default(), seq: parking_lot::Mutex::new(0), defaults: parking_lot::Mutex::new(defaults) });
        let app = AppHandle { state: Arc::downgrade(&state), emit };
        state.hub.set_admission_policy(admission_policy(&state.defaults.lock()));
        let notify = app.clone();
        // Connection-level, so every owner window hears it; any of them may answer.
        state.hub.set_admission_listener(Arc::new(move |change| { let _ = notify.emit("ss:admission", serde_json::to_value(change).unwrap_or(Value::Null)); }));
        Self { state, app }
    }
    pub fn shutdown(&self) {
        cancel_all_pending(&self.state);
        self.state.automation.stop_all();
        self.state.server.lock().take();
        for id in self.state.hub.ids() { self.state.hub.remove(&id); }
        for e in self.state.engines.lock().drain().map(|(_,e)|e) { let _ = e.terminate(); }
    }
    /// Trusted single-window adapter (the token-protected browser harness).
    /// Agent IPC never reaches this owner command boundary.
    pub fn invoke(&self, name: &str, args: Value) -> Result<Value, String> {
        self.invoke_in_window("main", name, args)
    }
    /// Native adapters supply the real window label, never a webview argument.
    pub fn invoke_in_window(&self, window: &str, name: &str, args: Value) -> Result<Value, String> {
        if !self.state.windows.lock().available(window) { return Err("Window closed".into()); }
        if let Some(session) = args.get("session").and_then(Value::as_str) {
            if self.state.windows.lock().owner(session).as_deref() != Some(window) {
                return Err("Session does not belong to this window".into());
            }
        }
        match name {
            "start" => start(&self.app, &self.state, window, arg(&args, "rows")?, arg(&args, "cols")?),
            "open_tab" => open_tab(&self.app, &self.state, window, arg(&args, "rows")?, arg(&args, "cols")?, arg(&args, "profileId")?).map(|id| json!(id)),
            "ui_ready" => { self.state.windows.lock().mark_ready(window); Ok(Value::Null) },
            "attend" => {
                let session: String = arg(&args, "session")?;
                self.state.windows.lock().select(window, &session);
                attend(&self.state, session).map(|v| json!(v))
            },
            "close_tab" => {
                let session: String = arg(&args, "session")?;
                cancel_pending(&self.state, &session);
                let _startup = self.state.startup.lock();
                close_tab(&self.state, session).map(|v| json!(v))
            },
            _ => dispatch(&self.app, &self.state, name, args),
        }
    }
    /// Reserve a native window before its webview starts. No fallback shell is created.
    pub fn prepare_automation_window(&self, window: &str) -> Result<(), String> {
        let mut windows = self.state.windows.lock();
        if !windows.available(window) { return Err("Window unavailable".into()); }
        windows.prepare(window);
        Ok(())
    }
    pub fn focus_window(&self, window: &str) {
        let active = self.state.windows.lock().active(window);
        if let Some(id) = active {
            if self.state.hub.attended_id().as_deref() != Some(&id) { self.state.hub.set_attended(&id); }
        }
    }
    /// Closing one native window must not terminate another window's shell.
    pub fn close_window(&self, window: &str) {
        let ids = {
            let mut windows = self.state.windows.lock();
            windows.close(window);
            windows.sessions(window)
        };
        for id in &ids { cancel_pending(&self.state, id); }
        let _startup = self.state.startup.lock();
        for id in ids { let _ = close_tab(&self.state, id); }
    }
    /// Native adapters authorize OS-resolved executables independently of window focus.
    pub fn linux_automation_allowed(&self, executable: &std::path::Path) -> bool {
        self.state.automation.allows_linux_executable(executable)
    }
    /// Narrow external adapter boundary. Never exposes trusted UI dispatch.
    pub fn automate(&self, caller: automation::Caller, operation: &str, args: Value) -> Result<Value, String> {
        self.automate_in_window("main", caller, operation, args)
    }
    pub fn automate_in_window(&self, window: &str, caller: automation::Caller, operation: &str, args: Value) -> Result<Value, String> {
        automation::dispatch(&self.app, &self.state, window, caller, operation, args)
    }
}
impl Drop for Harness { fn drop(&mut self) { self.shutdown(); } }
fn defaults_path(state: &AppState) -> PathBuf { state.config_dir.join("app.json") }
/// Apply the stored new-tab defaults to a fresh session.
fn apply_defaults(defaults: &Value, engine: &Engine) {
    let s_arc = engine.session();
    let mut s = s_arc.lock();
    if let Some(m) = defaults.get("mode").and_then(|m| serde_json::from_value::<AgentMode>(m.clone()).ok()) {
        let _ = s.set_mode(m); // Fresh sessions have no pending shell input.
    }
    if let Some(g) = defaults.get("gate").and_then(|g| g.as_bool()) {
        s.set_control_gate(g);
    }
    if let Some(p) = defaults.get("pacing").and_then(|p| p.as_object()) {
        let mut cur = serde_json::to_value(s.pacing()).unwrap();
        if let Some(c) = cur.as_object_mut() {
            for (k, v) in p {
                c.insert(k.clone(), v.clone());
            }
        }
        if let Ok(p) = serde_json::from_value::<Pacing>(cur) {
            s.set_pacing(p);
        }
    }
    match defaults.get("mask") {
        Some(Value::Array(a)) => s.set_affordance_mask(Some(a.iter().filter_map(|v| serde_json::from_value::<Affordance>(v.clone()).ok()).collect::<HashSet<_>>())),
        Some(Value::Null) => s.set_affordance_mask(None),
        _ => {}
    }
}

fn engine(state: &AppState, session: &str) -> Result<Arc<Engine>, String> {
    state.engines.lock().get(session).cloned().ok_or_else(|| format!("no session {session}"))
}

/// Installed before the PTY reader starts. Never locks Session from its callback.
fn terminal_output(app: AppHandle, session: String) -> conn_core::session::OutputFrameSink {
    Box::new(move |frame| {
        use base64::Engine as _;
        if let Some(state) = app.state.upgrade() {
            let data = base64::engine::general_purpose::STANDARD.encode(&frame.data);
            let value = json!({"session":session,"data":data,"outputSeq":frame.output_seq,"generation":frame.generation});
            let mut output = state.output.lock();
            match output.get_mut(&session) {
                Some(Some(buffer)) => {
                    buffer.push(value);
                    while buffer.len() > 1 && buffer.iter().map(|v| v["data"].as_str().map_or(0, str::len)).sum::<usize>() > 1_400_000 {
                        buffer.remove(0);
                    }
                }
                Some(None) => { let _ = app.emit("ss:output", value); }
                None => {},
            }
        }
    })
}

fn spawn_tab(app: &AppHandle, state: &AppState, window: &str, rows: u16, cols: u16, profile_id: Option<&str>) -> Result<String, String> {
    spawn_tab_with(app, state, window, rows, cols, profile_id, false, LaunchSpec::ProfileDefault, None)
}

fn spawn_tab_with(app: &AppHandle, state: &AppState, window: &str, mut rows: u16, mut cols: u16, profile_id: Option<&str>, external_private: bool, launch: LaunchSpec, caller_alive: Option<&(dyn Fn() -> bool + Send + Sync)>) -> Result<String, String> {
    let profiles = conn_core::profiles::Profiles::load(&state.config_dir.join("profiles.json"))?;
    let profile = profiles.profiles.iter().find(|p| p.id == profile_id.unwrap_or(&profiles.default_profile)).ok_or("Unknown profile")?.clone();
    let availability = profile.availability();
    if !availability.available { return Err(format!("{}: {}", profile.name, availability.message)); }
    let id = { let mut s = state.seq.lock(); *s += 1; format!("t{}-{}", *s, uuid::Uuid::new_v4()) };
    let audit = if external_private { conn_core::audit::Audit::null() } else { conn_core::audit::Audit::open(&state.config_dir.join("audit.jsonl")).map_err(|e|e.to_string())?.for_session(id.clone()) };
    let policy_path = state.config_dir.join("policy.yaml");
    let policy = match conn_core::policy::PolicyStore::open(&policy_path) {
        Ok(p) => p,
        Err(e) => {
            audit.record("system", "policy_load_failed", json!({"error": e.to_string()}));
            return Err(format!("Could not load policy {}: {e}. Fix the policy before starting a shell.", policy_path.display()));
        }
    };
    if !state.windows.lock().available(window) { return Err("Window closed".into()); }
    state.windows.lock().add(window, &id);
    state.output.lock().insert(id.clone(), Some(Vec::new()));
    if external_private {
        state.pending_sessions.lock().insert(id.clone(), PendingSession {
            profile_id: profile.id.clone(), profile_name: profile.name.clone(), rows, cols,
            attached: false, cancelled: false,
        });
        let prepared = (|| {
            app.emit("ss:tab_opened", json!({"session":id,"focus":true,"shared":false,"externalOrigin":true,"externalStarting":true}))?;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            loop {
                if caller_alive.is_some_and(|alive| !alive()) || !state.windows.lock().ready(window) {
                    return Err("External session unavailable".to_string());
                }
                let pending = state.pending_sessions.lock();
                let p = pending.get(&id).ok_or("External session unavailable")?;
                if p.cancelled { return Err("External session cancelled".to_string()); }
                if p.attached { return Ok((p.rows, p.cols)); }
                drop(pending);
                if std::time::Instant::now() >= deadline { return Err("Terminal renderer not ready".to_string()); }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        })();
        match prepared {
            Ok(size) => { rows = size.0; cols = size.1; },
            Err(error) => { abort_pending(app, state, window, &id); return Err(error); },
        }
    }
    // Freeze the preparation decision while starting. Human input that wins this
    // lock cancels the launch; input after startup is delivered to the live PTY.
    let mut pending = external_private.then(|| state.pending_sessions.lock());
    if external_private && (caller_alive.is_some_and(|alive| !alive())
        || !state.windows.lock().ready(window)
        || pending.as_ref().and_then(|p| p.get(&id)).is_none_or(|p| p.cancelled || !p.attached))
    {
        drop(pending); abort_pending(app, state, window, &id); return Err("External session unavailable".into());
    }
    let spawned = Engine::spawn(EngineConfig {
        profile: Some(profile), rows, cols, render_prompt: false, external_private, launch,
        output_frame: Some(terminal_output(app.clone(), id.clone())),
        // Both graphical frontends render with xterm, regardless of the launcher TERM.
        // Explicit profile environment overrides still take precedence in Engine.
        env: vec![("TERM".into(), "xterm-256color".into())],
        cwd: conn_core::profiles::home_dir(), audit: Some(audit), policy: Some(policy),
        ..EngineConfig::default()
    });
    let engine = match spawned {
        Ok(engine) => Arc::new(engine),
        Err(error) => {
            drop(pending);
            if external_private { abort_pending(app, state, window, &id); }
            else { state.output.lock().remove(&id); state.windows.lock().remove(&id); }
            return Err(error.to_string());
        },
    };
    let handle = app.clone();
    let sid = id.clone();
    engine.subscribe(
        "frontend",
        Box::new(move |ev: ServerEvent| match ev {
            ServerEvent::Output { .. } => {}, // Raw output uses the early writer above.
            other => {
                let mut v = serde_json::to_value(&other).unwrap_or(Value::Null);
                if let Some(o) = v.as_object_mut() {
                    o.insert("session".into(), Value::String(sid.clone()));
                }
                let _ = handle.emit("ss:event", v);
            }
        }),
        false,
    );
    if !external_private { apply_defaults(&state.defaults.lock(), engine.as_ref()); }
    state.hub.add(&id, engine.session());
    state.engines.lock().insert(id.clone(), engine);
    if let Some(pending) = pending.as_mut() { pending.remove(&id); }
    Ok(id)
}

fn cancel_pending(state: &AppState, session: &str) -> bool {
    if let Some(pending) = state.pending_sessions.lock().get_mut(session) {
        pending.cancelled = true; return true;
    }
    false
}

fn cancel_all_pending(state: &AppState) {
    for pending in state.pending_sessions.lock().values_mut() { pending.cancelled = true; }
}

fn abort_pending(app: &AppHandle, state: &AppState, window: &str, session: &str) {
    state.pending_sessions.lock().remove(session);
    state.output.lock().remove(session);
    let _ = app.emit("ss:tab_aborted", json!({"session":session}));
    let mut windows = state.windows.lock();
    windows.remove(session);
    windows.finish_prepare(window);
}

/// The desktop app asks before a new agent connection joins. Ordinary tabs are open
/// to every admitted connection, so this is where the owner consents. `"allow"` opts out.
fn admission_policy(defaults: &Value) -> AdmissionPolicy {
    if defaults.get("admission").and_then(Value::as_str) == Some("allow") { AdmissionPolicy::Allow } else { AdmissionPolicy::Ask }
}

fn set_admission(state: &AppState, ask: bool) -> Result<Value, String> {
    let mut defaults = state.defaults.lock().clone();
    if !defaults.is_object() { defaults = json!({}); }
    defaults["admission"] = json!(if ask { "ask" } else { "allow" });
    set_defaults(state, defaults)?;
    Ok(json!({"ask": ask}))
}

fn decide_admission(state: &AppState, conn: u64, allow: bool) -> Result<Value, String> {
    if state.hub.decide_admission(conn, allow) { Ok(json!({"decided": true})) } else { Err("Connection is no longer waiting".into()) }
}

/// Store `{mode?, gate?, pacing?, mask?, admission?}`. All but `admission` apply to new tabs.
fn set_defaults(state: &AppState, mut defaults: Value) -> Result<(), String> {
    // Tab defaults are saved as a whole; the connection-level choice is kept unless named.
    if !defaults.is_object() { defaults = json!({}); }
    if defaults.get("admission").is_none() {
        if let Some(kept) = state.defaults.lock().get("admission").cloned() { defaults["admission"] = kept; }
    }
    state.hub.set_admission_policy(admission_policy(&defaults));
    *state.defaults.lock() = defaults.clone();
    std::fs::write(defaults_path(state), serde_json::to_string_pretty(&defaults).unwrap()).map_err(|e| e.to_string())
}

/// Connected agents on a session and what each may do right now.
fn agents(state: &AppState, session: String) -> Result<Vec<Value>, String> {
    Ok(engine(&state, &session)?.session().lock().agent_affordances().into_iter().map(|(conn, id, aff)| json!({ "conn": conn, "agentId": id, "affordances": aff })).collect())
}

fn policy_rules(state: &AppState, session: String) -> Result<Value, String> {
    Ok(engine(&state, &session)?.session().lock().policy_description())
}

/// Local setup detection and actual open connections are intentionally separate.
fn diagnostics(state: &AppState, session: String) -> Result<Value, String> {
    let policy_path = engine(&state, &session)?.session().lock().policy_path().unwrap_or_else(|| state.config_dir.join("policy.yaml"));
    let connections = diagnostics::connections(&state.hub);
    let connected = connections.iter().filter_map(|c| c["agentId"].as_str().map(str::to_owned)).collect::<Vec<_>>();
    let catalog = integrations::catalog(&state.config_dir, &state.socket, state.integration_home.as_deref(), &connected);
    let (clients, config_error) = match catalog {
        Ok(c) => (c["clients"].clone(), None),
        Err(e) => (json!([]), Some(e)),
    };
    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "socket": state.socket.clone(),
        "configDir": state.config_dir.clone(),
        "auditPath": state.config_dir.join("audit.jsonl"),
        "policyPath": policy_path,
        "defaultsPath": defaults_path(state),
        "cli": agent_setup::cli_status(),
        "clients": clients,
        "configError": config_error,
        "agentConnections": connections,
        "sessions": state.hub.ids().len(),
    }))
}

fn log(msg: String) {
    eprintln!("[webview] {msg}");
}

fn start(app: &AppHandle, state: &AppState, window: &str, rows: u16, cols: u16) -> Result<Value, String> {
    let _startup = state.startup.lock();
    if !state.windows.lock().available(window) { return Err("Window closed".into()); }
    ensure_runtime(app, state)?;
    if state.windows.lock().pending(window) {
        return Ok(json!({"session":null,"sessions":[],"externalPending":true}));
    }
    let current = state.windows.lock().active(window);
    let session = match current { Some(id) => id, None => spawn_tab(app, state, window, rows, cols, None)? };
    let e = engine(state, &session)?;
    let sessions = state.windows.lock().sessions(window);
    let status = e.session().lock().status();
    Ok(json!({ "socket": state.socket, "shell": e.shell(), "session": session, "sessions": sessions, "shared": status.shared, "externalOrigin": status.external_origin }))
}

fn ensure_runtime(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let socket = state.socket.clone();
    if state.server.lock().is_none() {
        let guard = conn_core::ipc::serve_in_background(socket.clone(), state.hub.clone()).map_err(|e| e.to_string())?;
        *state.server.lock() = Some(guard);
        // Agents may open tabs. The tab appears in the strip, unattended, with the
        // agent's reason; the human decides whether to look at it.
        let handle = app.clone();
        state.hub.set_opener(std::sync::Arc::new(move |agent_id, reason| {
            let st = handle.state.upgrade().ok_or("frontend closed")?;
            let _startup = st.startup.lock();
            let window = st.hub.attended_id().and_then(|id| st.windows.lock().owner(&id)).unwrap_or_else(|| "main".into());
            let (rows, cols) = st
                .hub
                .attended_id()
                .and_then(|id| st.engines.lock().get(&id).cloned())
                .map(|e| { let s = e.session().lock().status().size; (s.rows, s.cols) })
                .unwrap_or((24, 80));
            let id = spawn_tab(&handle, &st, &window, rows, cols, None)?;
            let _ = handle.emit("ss:tab_opened", json!({ "session": id, "agentId": agent_id, "reason": reason }));
            Ok(id)
        }));
    }
    Ok(())
}

fn open_tab(app: &AppHandle, state: &AppState, window: &str, rows: u16, cols: u16, profile_id: Option<String>) -> Result<String, String> {
    let _startup = state.startup.lock();
    if state.windows.lock().pending(window) { return Err("Window is preparing an external session".into()); }
    let id = spawn_tab(app, state, window, rows, cols, profile_id.as_deref())?;
    state.windows.lock().select(window, &id);
    state.hub.set_attended(&id);
    Ok(id)
}

fn close_tab(state: &AppState, session: String) -> Result<Option<String>, String> {
    state.pending_sessions.lock().remove(&session);
    state.automation.stop_session(&session);
    state.output.lock().remove(&session);
    state.hub.remove(&session);
    if let Some(e) = state.engines.lock().remove(&session) {
        e.terminate().map_err(|e| e.to_string())?;
    }
    state.windows.lock().remove(&session);
    Ok(state.hub.attended_id())
}

fn attend(state: &AppState, session: String) -> Result<bool, String> {
    Ok(state.hub.set_attended(&session))
}

fn input(state: &AppState, session: String, data: String) -> Result<(), String> {
    if cancel_pending(state, &session) { return Ok(()); }
    engine(state, &session)?.session().lock().human_input(data.as_bytes());
    Ok(())
}

fn terminal_response(state: &AppState, session: String, data: String) -> Result<(), String> {
    if state.pending_sessions.lock().contains_key(&session) { return Ok(()); }
    engine(state, &session)?.session().lock().write_terminal_response(data.as_bytes()).map_err(|_| "Terminal response unavailable".into())
}

fn resize(state: &AppState, session: String, rows: u16, cols: u16) -> Result<(), String> {
    if let Some(pending) = state.pending_sessions.lock().get_mut(&session) {
        pending.rows = rows.max(1); pending.cols = cols.max(1); return Ok(());
    }
    engine(state, &session)?.session().lock().resize(rows, cols);
    Ok(())
}

fn status(state: &AppState, session: String) -> Result<Value, String> {
    if let Some(pending) = state.pending_sessions.lock().get(&session) {
        return Ok(json!({"shared":false,"externalOrigin":true,"externalStarting":true,"externalInputAvailable":false,
            "profileId":pending.profile_id,"profileName":pending.profile_name,"processAlive":false,
            "attended":true,"size":{"rows":pending.rows,"cols":pending.cols}}));
    }
    Ok(serde_json::to_value(engine(&state, &session)?.session().lock().status()).unwrap())
}

fn take(state: &AppState, session: String) -> Result<Option<String>, String> {
    if cancel_pending(state, &session) { return Ok(None); }
    Ok(engine(state, &session)?.session().lock().human_take())
}

fn approve(state: &AppState, session: String, approval_id: String, decision: String) -> Result<Value, String> {
    let d = match decision.as_str() { "deny" => Decision::Deny, "allow_session" => Decision::AllowSession, _ => Decision::Grant };
    engine(&state, &session)?.session().lock().resolve_approval(&approval_id, d, "frontend").map(|a| serde_json::to_value(a).unwrap()).map_err(|e| e.to_string())
}

fn cancel_exec(state: &AppState, session: String, exec_id: String) -> Result<(), String> {
    engine(&state, &session)?.session().lock().cancel_exec(&exec_id).map_err(|e| e.to_string())
}

fn execute_now(state: &AppState, session: String, exec_id: String) -> Result<(), String> {
    engine(&state, &session)?.session().lock().execute_now_scheduled(&exec_id).map_err(|e| e.to_string())
}

fn set_mode(state: &AppState, session: String, mode: AgentMode) -> Result<(), String> {
    engine(&state, &session)?.session().lock().set_mode(mode).map_err(|e| e.to_string())
}

fn set_control_gate(state: &AppState, session: String, ask: bool) -> Result<(), String> {
    engine(&state, &session)?.session().lock().set_control_gate(ask);
    Ok(())
}

fn decide_control(state: &AppState, session: String, request_id: String, grant: bool) -> Result<Value, String> {
    engine(&state, &session)?.session().lock().decide_control(&request_id, grant).map(|r| serde_json::to_value(r).unwrap()).map_err(|e| e.to_string())
}

fn accept_proposal(state: &AppState, session: String, proposal_id: String) -> Result<Value, String> {
    engine(&state, &session)?.session().lock().accept_proposal(&proposal_id).map(|r| serde_json::to_value(r).unwrap()).map_err(|e| e.to_string())
}

fn reject_proposal(state: &AppState, session: String, proposal_id: String) -> Result<(), String> {
    engine(&state, &session)?.session().lock().reject_proposal(&proposal_id).map_err(|e| e.to_string())
}

fn hand_back(state: &AppState, session: String) -> Result<Value, String> {
    engine(&state, &session)?.session().lock().hand_back().map(|r| serde_json::to_value(r).unwrap()).map_err(|e| e.to_string())
}

fn revoke_session_allow(state: &AppState, session: String, label: String) -> Result<bool, String> {
    Ok(engine(&state, &session)?.session().lock().revoke_session_allow(&label))
}

fn set_pacing(state: &AppState, session: String, patch: Value) -> Result<Pacing, String> {
    let e = engine(&state, &session)?;
    let s_arc = e.session();
    let mut s = s_arc.lock();
    let mut cur = serde_json::to_value(s.pacing()).unwrap();
    if let (Some(c), Some(p)) = (cur.as_object_mut(), patch.as_object()) {
        for (k, v) in p { c.insert(k.clone(), v.clone()); }
    }
    let pacing: Pacing = serde_json::from_value(cur).map_err(|e| e.to_string())?;
    s.set_pacing(pacing.clone());
    Ok(pacing)
}

fn set_affordances(state: &AppState, session: String, allow: Option<Vec<Affordance>>) -> Result<(), String> {
    engine(&state, &session)?.session().lock().set_affordance_mask(allow.map(|v| v.into_iter().collect::<HashSet<_>>()));
    Ok(())
}

fn policy_read(state: &AppState, session: String) -> Result<Value, String> {
    let path = engine(&state, &session)?.session().lock().policy_path().unwrap_or_else(|| state.config_dir.join("policy.yaml"));
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(json!({ "path": path, "text": text }))
}

fn policy_write(state: &AppState, session: String, text: String) -> Result<(), String> {
    conn_core::policy::Policy::parse(&text).map_err(|e| e.to_string())?;
    let path = engine(&state, &session)?.session().lock().policy_path().unwrap_or_else(|| state.config_dir.join("policy.yaml"));
    std::fs::write(&path, text).map_err(|e| e.to_string())
}

fn policy_test(state: &AppState, session: String, cmd: String) -> Result<Value, String> {
    Ok(serde_json::to_value(engine(&state, &session)?.session().lock().analyse_line(&cmd)).unwrap())
}

fn audit_tail(state: &AppState, n: usize) -> Result<Vec<Value>, String> {
    let events = conn_core::audit::read_events(&state.config_dir.join("audit.jsonl")).unwrap_or_default();
    let start = events.len().saturating_sub(n);
    Ok(events[start..].iter().map(|e| serde_json::to_value(e).unwrap()).collect())
}

fn profiles_catalog(state: &AppState) -> Result<conn_core::profiles::Catalog,String> {
    Ok(conn_core::profiles::Profiles::load(&state.config_dir.join("profiles.json"))?.catalog())
}
fn profiles_discover(_state: &AppState) -> Result<Vec<Profile>,String> { Ok(conn_core::profiles::discover()) }
fn profiles_save(state: &AppState, config: conn_core::profiles::Profiles) -> Result<conn_core::profiles::Catalog,String> {
    cancel_all_pending(state);
    let _startup = state.startup.lock();
    config.save(&state.config_dir.join("profiles.json"))?; state.automation.stop_all(); profiles_catalog(state)
}
fn profiles_test(_state: &AppState, profile: Profile) -> Result<Availability,String> { conn_core::profiles::test(&profile) }

fn arg<T: serde::de::DeserializeOwned>(args: &Value, key: &str) -> Result<T,String> {
    serde_json::from_value(args.get(key).cloned().unwrap_or(Value::Null)).map_err(|e|format!("{key}: {e}"))
}
fn out<T: serde::Serialize>(value: T) -> Result<Value,String> { serde_json::to_value(value).map_err(|e|e.to_string()) }
fn dispatch(app: &AppHandle, state: &AppState, name: &str, args: Value) -> Result<Value,String> {
    if let Some(result) = surface_commands::dispatch(state, name, &args) { return result; }
    if let Some(result) = extension_commands::dispatch(state, name, &args) { return result; }
    match name {
        "automation_settings" => Ok(state.automation.settings()),
        "automation_save" => automation::save(state, arg(&args, "config")?),
        "automation_revoke" => { cancel_all_pending(state); state.automation.stop_all(); Ok(Value::Null) },
        "attach_output" => {
            let id: String = arg(&args, "session")?;
            {
                let mut output = state.output.lock();
                if let Some(entry) = output.get_mut(&id) {
                    for frame in entry.take().unwrap_or_default() { app.emit("ss:output", frame)?; }
                }
            }
            if let Some(pending) = state.pending_sessions.lock().get_mut(&id) {
                if pending.cancelled { return Err("External session unavailable".into()); }
                pending.attached = true;
            }
            Ok(Value::Null)
        },
        "update_info" => Ok(json!({"version": env!("CARGO_PKG_VERSION"), "os": std::env::consts::OS, "arch": std::env::consts::ARCH})),
        "open_release" => updates::open(&arg::<String>(&args, "url")?).map(|_| Value::Null),
        "pending_admissions" => Ok(json!(state.hub.pending_admissions().into_iter().map(|a| json!({"connId":a.conn_id,"agentId":a.agent_id})).collect::<Vec<_>>())),
        "admission_policy" => Ok(json!({"ask": state.hub.admission_policy() == AdmissionPolicy::Ask})),
        "set_admission" => set_admission(state, arg::<bool>(&args, "ask")?),
        "decide_admission" => decide_admission(state, arg::<u64>(&args, "connId")?, arg::<bool>(&args, "allow")?),
        "set_defaults" => out(set_defaults(state, arg::<Value>(&args, "defaults")?)?),
        "agents" => out(agents(state, arg::<String>(&args, "session")?)?),
        "policy_rules" => out(policy_rules(state, arg::<String>(&args, "session")?)?),
        "diagnostics" => out(diagnostics(state, arg::<String>(&args, "session")?)?),
        "log" => out(log(arg::<String>(&args, "msg")?)),
        "input" => out(input(state, arg::<String>(&args, "session")?, arg::<String>(&args, "data")?)?),
        "terminal_response" => { terminal_response(state, arg(&args, "session")?, arg(&args, "data")?)?; Ok(Value::Null) },
        "resize" => out(resize(state, arg::<String>(&args, "session")?, arg::<u16>(&args, "rows")?, arg::<u16>(&args, "cols")?)?),
        "status" => out(status(state, arg::<String>(&args, "session")?)?),
        "take" => out(take(state, arg::<String>(&args, "session")?)?),
        "approve" => out(approve(state, arg::<String>(&args, "session")?, arg::<String>(&args, "approvalId")?, arg::<String>(&args, "decision")?)?),
        "cancel_exec" => out(cancel_exec(state, arg::<String>(&args, "session")?, arg::<String>(&args, "execId")?)?),
        "execute_now" => out(execute_now(state, arg::<String>(&args, "session")?, arg::<String>(&args, "execId")?)?),
        "set_mode" => out(set_mode(state, arg::<String>(&args, "session")?, arg::<AgentMode>(&args, "mode")?)?),
        "set_control_gate" => out(set_control_gate(state, arg::<String>(&args, "session")?, arg::<bool>(&args, "ask")?)?),
        "decide_control" => out(decide_control(state, arg::<String>(&args, "session")?, arg::<String>(&args, "requestId")?, arg::<bool>(&args, "grant")?)?),
        "accept_proposal" => out(accept_proposal(state, arg::<String>(&args, "session")?, arg::<String>(&args, "proposalId")?)?),
        "reject_proposal" => out(reject_proposal(state, arg::<String>(&args, "session")?, arg::<String>(&args, "proposalId")?)?),
        "hand_back" => out(hand_back(state, arg::<String>(&args, "session")?)?),
        "revoke_session_allow" => out(revoke_session_allow(state, arg::<String>(&args, "session")?, arg::<String>(&args, "label")?)?),
        "set_pacing" => out(set_pacing(state, arg::<String>(&args, "session")?, arg::<Value>(&args, "patch")?)?),
        "set_affordances" => out(set_affordances(state, arg::<String>(&args, "session")?, arg::<Option<Vec<Affordance>>>(&args, "allow")?)?),
        "policy_read" => out(policy_read(state, arg::<String>(&args, "session")?)?),
        "policy_write" => out(policy_write(state, arg::<String>(&args, "session")?, arg::<String>(&args, "text")?)?),
        "policy_test" => out(policy_test(state, arg::<String>(&args, "session")?, arg::<String>(&args, "cmd")?)?),
        "audit_tail" => out(audit_tail(state, arg::<usize>(&args, "n")?)?),
        "cli_status" => Ok(agent_setup::cli_status()),
        "agent_integrations" => {
            let connected: Vec<String> = state.engines.lock().values().flat_map(|e| e.session().lock().agent_affordances().into_iter().map(|(_, id, _)| id)).collect();
            integrations::catalog(&state.config_dir, &state.socket, state.integration_home.as_deref(), &connected)
        },
        "agent_integration_install" => { integrations::configure(&state.config_dir, &state.socket, state.integration_home.as_deref(), &arg::<String>(&args, "client")?)?; Ok(Value::Null) },
        "agent_integration_remove" => { integrations::remove(&state.config_dir, state.integration_home.as_deref(), &arg::<String>(&args, "client")?)?; Ok(Value::Null) },
        "agent_integration_manual" => integrations::manual(&state.config_dir, &state.socket, &arg::<String>(&args, "client")?),
        "agent_configuration" => agent_setup::configuration(&state.config_dir, &state.socket),
        "install_cli" => out(agent_setup::install_cli(&state.config_dir)?),
        "profiles_catalog" => out(profiles_catalog(state)?),
        "profiles_discover" => out(profiles_discover(state)?),
        "profiles_save" => out(profiles_save(state, arg::<conn_core::profiles::Profiles>(&args, "config")?)?),
        "profiles_test" => out(profiles_test(state, arg::<conn_core::backend::Profile>(&args, "profile")?)?),
        _ => Err(format!("Unknown frontend command: {name}")),
    }
}
