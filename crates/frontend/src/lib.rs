//! Tauri side: one engine per tab, all behind one hub/socket. Output and lifecycle
//! events reach the webview tagged with the session id.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde_json::{json, Value};
use conn_core::affordance::Affordance;
use conn_core::approval::Decision;
use conn_core::ipc::{Hub, ServerGuard, SharedHub};
use conn_core::session::{AgentMode, ServerEvent};
use conn_core::{Engine, EngineConfig, Pacing};
use std::path::PathBuf;
use conn_core::backend::{Profile, Availability};

struct AppState {
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
impl AppHandle { fn emit(&self, name: &str, value: Value) -> Result<(), String> { (self.emit)(name, value); Ok(()) } }

pub struct Harness { state: Arc<AppState>, app: AppHandle }
impl Harness {
    pub fn new(config_dir: PathBuf, socket: PathBuf, emit: Emit) -> Self {
        let defaults = std::fs::read_to_string(config_dir.join("app.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(json!({}));
        let state = Arc::new(AppState { config_dir, socket, engines: Default::default(), hub: Hub::new(), server: Default::default(), seq: parking_lot::Mutex::new(0), defaults: parking_lot::Mutex::new(defaults) });
        let app = AppHandle { state: Arc::downgrade(&state), emit };
        Self { state, app }
    }
    pub fn shutdown(&self) {
        self.state.server.lock().take();
        for id in self.state.hub.ids() { self.state.hub.remove(&id); }
        for e in self.state.engines.lock().drain().map(|(_,e)|e) { let _ = e.terminate(); }
    }
    pub fn invoke(&self, name: &str, args: Value) -> Result<Value, String> {
        dispatch(&self.app, &self.state, name, args)
    }
}
impl Drop for Harness { fn drop(&mut self) { self.shutdown(); } }
fn defaults_path(state: &AppState) -> PathBuf { state.config_dir.join("app.json") }
/// Apply the stored new-tab defaults to a fresh session.
fn apply_defaults(defaults: &Value, engine: &Engine) {
    let s_arc = engine.session();
    let mut s = s_arc.lock();
    if let Some(m) = defaults.get("mode").and_then(|m| serde_json::from_value::<AgentMode>(m.clone()).ok()) {
        s.set_mode(m);
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

fn spawn_tab(app: &AppHandle, state: &AppState, rows: u16, cols: u16, profile_id: Option<&str>) -> Result<String, String> {
    let profiles = conn_core::profiles::Profiles::load(&state.config_dir.join("profiles.json"))?;
    let profile = profiles.profiles.iter().find(|p| p.id == profile_id.unwrap_or(&profiles.default_profile)).ok_or("Unknown profile")?.clone();
    let availability = profile.availability();
    if !availability.available { return Err(format!("{}: {}", profile.name, availability.message)); }
    let id = { let mut s = state.seq.lock(); *s += 1; format!("t{}", *s) };
    let audit = conn_core::audit::Audit::open(&state.config_dir.join("audit.jsonl")).map_err(|e|e.to_string())?;
    let policy_path = state.config_dir.join("policy.yaml");
    let policy = match conn_core::policy::PolicyStore::open(&policy_path) {
        Ok(p) => p,
        Err(e) => {
            audit.record("system", "policy_load_failed", json!({"error": e.to_string()}));
            return Err(format!("Could not load policy {}: {e}. Fix the policy before starting a shell.", policy_path.display()));
        }
    };
    let engine = Arc::new(Engine::spawn(EngineConfig {
        profile: Some(profile), rows, cols, render_prompt: false, output: None,
        // Both graphical frontends render with xterm, regardless of the launcher TERM.
        // Explicit profile environment overrides still take precedence in Engine.
        env: vec![("TERM".into(), "xterm-256color".into())],
        cwd: conn_core::profiles::home_dir(), audit: Some(audit), policy: Some(policy),
        ..EngineConfig::default()
    }).map_err(|e|e.to_string())?);
    let handle = app.clone();
    let sid = id.clone();
    engine.subscribe(
        "frontend",
        Box::new(move |ev: ServerEvent| match ev {
            ServerEvent::Output { data } => {
                let _ = handle.emit("ss:output", json!({ "session": sid, "data": data }));
            }
            other => {
                let mut v = serde_json::to_value(&other).unwrap_or(Value::Null);
                if let Some(o) = v.as_object_mut() {
                    o.insert("session".into(), Value::String(sid.clone()));
                }
                let _ = handle.emit("ss:event", v);
            }
        }),
        true,
    );
    apply_defaults(&state.defaults.lock(), engine.as_ref());
    state.hub.add(&id, engine.session());
    state.engines.lock().insert(id.clone(), engine);
    Ok(id)
}

fn get_defaults(state: &AppState) -> Value {
    state.defaults.lock().clone()
}

/// Store `{mode?, gate?, pacing?, mask?}` as the defaults for new tabs.
fn set_defaults(state: &AppState, defaults: Value) -> Result<(), String> {
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

/// Where things are and which harnesses have the plugin.
fn diagnostics(state: &AppState, session: String) -> Result<Value, String> {
    let home = conn_core::profiles::home_dir().unwrap_or_default();
    let has = |rel: &str, needle: &str| std::fs::read_to_string(home.join(rel)).map(|t| t.contains(needle)).unwrap_or(false);
    let policy_path = engine(&state, &session)?.session().lock().policy_path().unwrap_or_else(|| state.config_dir.join("policy.yaml"));
    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "socket": state.socket.clone(),
        "configDir": state.config_dir.clone(),
        "auditPath": state.config_dir.join("audit.jsonl"),
        "policyPath": policy_path,
        "defaultsPath": defaults_path(state),
        "cli": cli_status(),
        "plugins": {
            "claude": has(".claude/plugins/installed_plugins.json", "\"conn@conn\""),
            "copilot": has(".copilot/config.json", "conn@conn") || has(".copilot/config.json", "\"conn\"") || has(".copilot/mcp-config.json", "\"conn\""),
            "codex": has(".codex/config.toml", "plugins.\"conn@conn\"") || has(".codex/config.toml", "[mcp_servers.conn]"),
        },
        "sessions": state.hub.ids().len(),
    }))
}

fn log(msg: String) {
    eprintln!("[webview] {msg}");
}

fn start(app: &AppHandle, state: &AppState, rows: u16, cols: u16) -> Result<Value, String> {
    let socket = state.socket.clone();
    if state.server.lock().is_none() {
        let guard = conn_core::ipc::serve_in_background(socket.clone(), state.hub.clone()).map_err(|e| e.to_string())?;
        *state.server.lock() = Some(guard);
        // Agents may open tabs. The tab appears in the strip, unattended, with the
        // agent's reason; the human decides whether to look at it or entrust it.
        let handle = app.clone();
        state.hub.set_opener(std::sync::Arc::new(move |agent_id, reason| {
            let st = handle.state.upgrade().ok_or("frontend closed")?;
            let (rows, cols) = st
                .hub
                .attended_id()
                .and_then(|id| st.engines.lock().get(&id).cloned())
                .map(|e| { let s = e.session().lock().status().size; (s.rows, s.cols) })
                .unwrap_or((24, 80));
            let id = spawn_tab(&handle, &st, rows, cols, None)?;
            let _ = handle.emit("ss:tab_opened", json!({ "session": id, "agentId": agent_id, "reason": reason }));
            Ok(id)
        }));
    }
    let first = state.engines.lock().is_empty();
    let session = if first { spawn_tab(&app, &state, rows, cols, None)? } else { state.hub.attended_id().unwrap_or_default() };
    let e = engine(&state, &session)?;
    Ok(json!({ "socket": socket, "shell": e.shell(), "session": session, "sessions": state.hub.ids() }))
}

fn open_tab(app: &AppHandle, state: &AppState, rows: u16, cols: u16, profile_id: Option<String>) -> Result<String, String> {
    let id = spawn_tab(&app, &state, rows, cols, profile_id.as_deref())?;
    state.hub.set_attended(&id);
    Ok(id)
}

fn close_tab(state: &AppState, session: String) -> Result<Option<String>, String> {
    state.hub.remove(&session);
    if let Some(e) = state.engines.lock().remove(&session) {
        e.terminate().map_err(|e| e.to_string())?;
    }
    Ok(state.hub.attended_id())
}

fn attend(state: &AppState, session: String) -> Result<bool, String> {
    Ok(state.hub.set_attended(&session))
}

fn entrust(state: &AppState, session: String) -> Result<String, String> {
    engine(&state, &session)?.session().lock().entrust().map_err(|e| e.to_string())
}

fn input(state: &AppState, session: String, data: String) -> Result<(), String> {
    engine(&state, &session)?.write_input(data.as_bytes());
    Ok(())
}

fn resize(state: &AppState, session: String, rows: u16, cols: u16) -> Result<(), String> {
    engine(&state, &session)?.resize(rows, cols);
    Ok(())
}

fn status(state: &AppState, session: String) -> Result<Value, String> {
    Ok(serde_json::to_value(engine(&state, &session)?.session().lock().status()).unwrap())
}

fn take(state: &AppState, session: String) -> Result<Option<String>, String> {
    Ok(engine(&state, &session)?.session().lock().human_take())
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
    engine(&state, &session)?.session().lock().set_mode(mode);
    Ok(())
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

fn sidecar_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let p = exe.parent()?.join(if cfg!(windows) {"conn.exe"} else {"conn"});
    p.exists().then_some(p)
}

fn cli_status() -> Value {
    let on_path = conn_core::backend::executable("conn").map(|p| p.display().to_string());
    json!({ "bundled": sidecar_path(), "onPath": on_path })
}

#[cfg(unix)]
fn install_cli() -> Result<String, String> {
    let src = sidecar_path().ok_or("bundled CLI not found")?;
    let candidates = [Some(std::path::PathBuf::from("/usr/local/bin")), std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/bin"))];
    for dir in candidates.iter().flatten() {
        if std::fs::create_dir_all(dir).is_err() { continue; }
        let dst = dir.join("conn");
        let _ = std::fs::remove_file(&dst);
        if std::os::unix::fs::symlink(&src, &dst).is_ok() { return Ok(dst.display().to_string()); }
    }
    Err("no writable PATH directory (/usr/local/bin or ~/.local/bin)".into())
}

#[cfg(windows)]
fn install_cli() -> Result<String, String> {
    Err("Run cargo install --path crates/cli --locked to install conn.exe on your Cargo PATH".into())
}

fn profiles_catalog(state: &AppState) -> Result<conn_core::profiles::Catalog,String> {
    Ok(conn_core::profiles::Profiles::load(&state.config_dir.join("profiles.json"))?.catalog())
}
fn profiles_discover(_state: &AppState) -> Result<Vec<Profile>,String> { Ok(conn_core::profiles::discover()) }
fn profiles_save(state: &AppState, config: conn_core::profiles::Profiles) -> Result<conn_core::profiles::Catalog,String> {
    config.save(&state.config_dir.join("profiles.json"))?; profiles_catalog(state)
}
fn profiles_test(_state: &AppState, profile: Profile) -> Result<Availability,String> { conn_core::profiles::test(&profile) }

fn arg<T: serde::de::DeserializeOwned>(args: &Value, key: &str) -> Result<T,String> {
    serde_json::from_value(args.get(key).cloned().unwrap_or(Value::Null)).map_err(|e|format!("{key}: {e}"))
}
fn dispatch(app: &AppHandle, state: &AppState, name: &str, args: Value) -> Result<Value,String> {
    match name {
        "get_defaults" => serde_json::to_value(get_defaults(state)).map_err(|e|e.to_string()),
        "set_defaults" => serde_json::to_value(set_defaults(state, arg::<Value>(&args, "defaults")?)?).map_err(|e|e.to_string()),
        "agents" => serde_json::to_value(agents(state, arg::<String>(&args, "session")?)?).map_err(|e|e.to_string()),
        "policy_rules" => serde_json::to_value(policy_rules(state, arg::<String>(&args, "session")?)?).map_err(|e|e.to_string()),
        "diagnostics" => serde_json::to_value(diagnostics(state, arg::<String>(&args, "session")?)?).map_err(|e|e.to_string()),
        "log" => serde_json::to_value(log(arg::<String>(&args, "msg")?)).map_err(|e|e.to_string()),
        "start" => serde_json::to_value(start(app, state, arg::<u16>(&args, "rows")?, arg::<u16>(&args, "cols")?)?).map_err(|e|e.to_string()),
        "open_tab" => serde_json::to_value(open_tab(app, state, arg::<u16>(&args, "rows")?, arg::<u16>(&args, "cols")?, arg::<Option<String>>(&args, "profileId")?)?).map_err(|e|e.to_string()),
        "close_tab" => serde_json::to_value(close_tab(state, arg::<String>(&args, "session")?)?).map_err(|e|e.to_string()),
        "attend" => serde_json::to_value(attend(state, arg::<String>(&args, "session")?)?).map_err(|e|e.to_string()),
        "entrust" => serde_json::to_value(entrust(state, arg::<String>(&args, "session")?)?).map_err(|e|e.to_string()),
        "input" => serde_json::to_value(input(state, arg::<String>(&args, "session")?, arg::<String>(&args, "data")?)?).map_err(|e|e.to_string()),
        "resize" => serde_json::to_value(resize(state, arg::<String>(&args, "session")?, arg::<u16>(&args, "rows")?, arg::<u16>(&args, "cols")?)?).map_err(|e|e.to_string()),
        "status" => serde_json::to_value(status(state, arg::<String>(&args, "session")?)?).map_err(|e|e.to_string()),
        "take" => serde_json::to_value(take(state, arg::<String>(&args, "session")?)?).map_err(|e|e.to_string()),
        "approve" => serde_json::to_value(approve(state, arg::<String>(&args, "session")?, arg::<String>(&args, "approvalId")?, arg::<String>(&args, "decision")?)?).map_err(|e|e.to_string()),
        "cancel_exec" => serde_json::to_value(cancel_exec(state, arg::<String>(&args, "session")?, arg::<String>(&args, "execId")?)?).map_err(|e|e.to_string()),
        "execute_now" => serde_json::to_value(execute_now(state, arg::<String>(&args, "session")?, arg::<String>(&args, "execId")?)?).map_err(|e|e.to_string()),
        "set_mode" => serde_json::to_value(set_mode(state, arg::<String>(&args, "session")?, arg::<AgentMode>(&args, "mode")?)?).map_err(|e|e.to_string()),
        "set_control_gate" => serde_json::to_value(set_control_gate(state, arg::<String>(&args, "session")?, arg::<bool>(&args, "ask")?)?).map_err(|e|e.to_string()),
        "decide_control" => serde_json::to_value(decide_control(state, arg::<String>(&args, "session")?, arg::<String>(&args, "requestId")?, arg::<bool>(&args, "grant")?)?).map_err(|e|e.to_string()),
        "accept_proposal" => serde_json::to_value(accept_proposal(state, arg::<String>(&args, "session")?, arg::<String>(&args, "proposalId")?)?).map_err(|e|e.to_string()),
        "reject_proposal" => serde_json::to_value(reject_proposal(state, arg::<String>(&args, "session")?, arg::<String>(&args, "proposalId")?)?).map_err(|e|e.to_string()),
        "hand_back" => serde_json::to_value(hand_back(state, arg::<String>(&args, "session")?)?).map_err(|e|e.to_string()),
        "revoke_session_allow" => serde_json::to_value(revoke_session_allow(state, arg::<String>(&args, "session")?, arg::<String>(&args, "label")?)?).map_err(|e|e.to_string()),
        "set_pacing" => serde_json::to_value(set_pacing(state, arg::<String>(&args, "session")?, arg::<Value>(&args, "patch")?)?).map_err(|e|e.to_string()),
        "set_affordances" => serde_json::to_value(set_affordances(state, arg::<String>(&args, "session")?, arg::<Option<Vec<Affordance>>>(&args, "allow")?)?).map_err(|e|e.to_string()),
        "policy_read" => serde_json::to_value(policy_read(state, arg::<String>(&args, "session")?)?).map_err(|e|e.to_string()),
        "policy_write" => serde_json::to_value(policy_write(state, arg::<String>(&args, "session")?, arg::<String>(&args, "text")?)?).map_err(|e|e.to_string()),
        "policy_test" => serde_json::to_value(policy_test(state, arg::<String>(&args, "session")?, arg::<String>(&args, "cmd")?)?).map_err(|e|e.to_string()),
        "audit_tail" => serde_json::to_value(audit_tail(state, arg::<usize>(&args, "n")?)?).map_err(|e|e.to_string()),
        "cli_status" => serde_json::to_value(cli_status()).map_err(|e|e.to_string()),
        "install_cli" => serde_json::to_value(install_cli()?).map_err(|e|e.to_string()),
        "profiles_catalog" => serde_json::to_value(profiles_catalog(state)?).map_err(|e|e.to_string()),
        "profiles_discover" => serde_json::to_value(profiles_discover(state)?).map_err(|e|e.to_string()),
        "profiles_save" => serde_json::to_value(profiles_save(state, arg::<conn_core::profiles::Profiles>(&args, "config")?)?).map_err(|e|e.to_string()),
        "profiles_test" => serde_json::to_value(profiles_test(state, arg::<conn_core::backend::Profile>(&args, "profile")?)?).map_err(|e|e.to_string()),
        _ => Err(format!("Unknown frontend command: {name}")),
    }
}
