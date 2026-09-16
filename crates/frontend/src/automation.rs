//! Platform-neutral, session-scoped external automation. This is intentionally
//! narrower than Harness::invoke and always uses the agent policy path.
use crate::{AppHandle, AppState};
use conn_core::authority::ConnId;
use conn_core::session::{ControlRequestState, ExecState, ProposalState, SharedSession};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Weak,
};
use std::time::{Duration, Instant};

const MAX_REQUESTS: usize = 256;
const MAX_QUEUE: usize = 16;
const TIMEOUT: Duration = Duration::from_secs(120);

/// Set by the native adapter from OS sender metadata, never from script arguments.
#[derive(Clone, Debug)]
pub struct Caller {
    pub identity: String,
    pub name: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub enabled: bool,
    pub profiles: Vec<String>,
}

struct Binding {
    owner: String,
    name: String,
    window: String,
    session_id: String,
    handle: String,
    conn: ConnId,
    session: SharedSession,
    stopped: AtomicBool,
    operation: Mutex<()>,
    queue: Mutex<VecDeque<Arc<Job>>>,
    wake: parking_lot::Condvar,
}
impl Binding {
    fn stop(&self) {
        let _operation = self.operation.lock();
        if !self.stopped.swap(true, Ordering::AcqRel) {
            self.session.lock().connection_closed(self.conn);
        }
        self.wake.notify_all();
    }
}
struct Job {
    id: String,
    owner: String,
    binding: Arc<Binding>,
    text: String,
    newline: bool,
    release_after_delivery: bool,
    intent: String,
    created: Instant,
    cancelled: AtomicBool,
    result: Mutex<Value>,
}
impl Job {
    fn update(&self, state: &str, detail: Value) {
        *self.result.lock() = json!({"requestId": self.id, "session": self.binding.handle, "tab": self.binding.session_id, "state": state, "detail": detail});
    }
    fn done(&self) -> bool {
        matches!(
            self.result.lock()["state"].as_str(),
            Some("delivered" | "denied" | "cancelled" | "failed")
        )
    }
}

pub(crate) struct Automation {
    config: Mutex<Config>,
    bindings: Mutex<HashMap<String, Arc<Binding>>>,
    requests: Mutex<HashMap<String, Arc<Job>>>,
}
impl Automation {
    pub(crate) fn load(dir: &std::path::Path) -> Self {
        let config = std::fs::read(dir.join("automation.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            config: Mutex::new(config),
            bindings: Default::default(),
            requests: Default::default(),
        }
    }
    pub(crate) fn settings(&self) -> Value {
        let mut recent: Vec<_> = self
            .requests
            .lock()
            .values()
            .map(|j| {
                (
                    j.created,
                    json!({"caller": j.binding.name, "result": j.result.lock().clone()}),
                )
            })
            .collect();
        recent.sort_by_key(|(time, _)| std::cmp::Reverse(*time));
        json!({"config": self.config.lock().clone(), "nativeSupported": cfg!(target_os="macos"), "recent": recent.into_iter().take(20).map(|(_,v)|v).collect::<Vec<_>>()})
    }
    pub(crate) fn stop_all(&self) {
        for b in self.bindings.lock().values() {
            b.stop();
        }
    }
    pub(crate) fn stop_session(&self, id: &str) {
        for b in self.bindings.lock().values().filter(|b| b.session_id == id) {
            b.stop();
        }
    }
}

pub(crate) fn save(state: &AppState, config: Config) -> Result<Value, String> {
    let mut current = state.automation.config.lock();
    let profiles = conn_core::profiles::Profiles::load(&state.config_dir.join("profiles.json"))?;
    if config
        .profiles
        .iter()
        .any(|id| !profiles.profiles.iter().any(|p| &p.id == id))
    {
        return Err("Unknown automation profile".into());
    }
    std::fs::create_dir_all(&state.config_dir).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(&state.config_dir).map_err(|e| e.to_string())?;
    use std::io::Write;
    file.write_all(&serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.persist(state.config_dir.join("automation.json"))
        .map_err(|e| e.to_string())?;
    // Changing scope revokes existing sessions; existing shells remain available to the human.
    state.automation.stop_all();
    *current = config;
    drop(current);
    Ok(state.automation.settings())
}

fn validate_text(text: &str) -> Result<(), String> {
    if text.len() > 16_384 || text.chars().any(char::is_control) {
        return Err("Text must be one line without control characters (maximum 16 KiB); use the newline option".into());
    }
    Ok(())
}

fn field(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("Missing {key}"))
}
fn binding(state: &AppState, caller: &Caller, id: &str) -> Result<Arc<Binding>, String> {
    state
        .automation
        .bindings
        .lock()
        .get(id)
        .filter(|b| b.owner == caller.identity && !b.stopped.load(Ordering::Acquire))
        .cloned()
        .ok_or_else(|| "Session unavailable for this caller; create a new session".into())
}
fn request(state: &AppState, caller: &Caller, id: &str) -> Result<Arc<Job>, String> {
    state
        .automation
        .requests
        .lock()
        .get(id)
        .filter(|j| j.owner == caller.identity)
        .cloned()
        .ok_or_else(|| "Unknown request for this caller".into())
}

pub(crate) fn dispatch(
    app: &AppHandle,
    state: &Arc<AppState>,
    window: &str,
    caller: Caller,
    operation: &str,
    args: Value,
) -> Result<Value, String> {
    if caller.identity.is_empty() || caller.name.is_empty() || caller.name.len() > 256 {
        return Err("Invalid native caller".into());
    }
    match operation {
        "window.create" => {
            if args.get("sensitive").and_then(Value::as_bool) == Some(true) {
                return Err(
                    "Sensitive input is not supported; use SSH-managed authentication".into(),
                );
            }
            // Validate before opening a shell. The native adapter supplies the new
            // window label and creates its webview after this scoped session exists.
            let command = match args.get("command") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) if s.is_empty() => None,
                Some(Value::String(s)) => {
                    validate_text(s)?;
                    Some(s.clone())
                }
                _ => return Err("Command must be text".into()),
            };
            let handle = dispatch(
                app,
                state,
                window,
                caller.clone(),
                "session.create",
                json!({}),
            )?;
            let request = if let Some(command) = command {
                match dispatch(
                    app,
                    state,
                    window,
                    caller.clone(),
                    "session.write",
                    json!({
                        "session":handle, "text":command, "newline":true,
                        "releaseAfterDelivery":true,
                        "intent":"Run the startup command requested by the external launcher"
                    }),
                ) {
                    Ok(id) => id,
                    Err(error) => {
                        if let Ok(b) = binding(state, &caller, handle.as_str().unwrap_or_default())
                        {
                            let _startup = state.startup.lock();
                            let _ = crate::close_tab(state, b.session_id.clone());
                        }
                        return Err(error);
                    }
                }
            } else {
                Value::Null
            };
            Ok(json!({"session":handle,"requestId":request}))
        }
        "session.create" => {
            let config = state.automation.config.lock();
            if !config.enabled {
                return Err("Enable AppleScript in Settings → Automation first".into());
            }
            let profiles =
                conn_core::profiles::Profiles::load(&state.config_dir.join("profiles.json"))?;
            let profile = args
                .get("profile")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or(&profiles.default_profile);
            if !config.profiles.iter().any(|p| p == profile) {
                return Err("This profile is not enabled for automation".into());
            }
            if state
                .automation
                .bindings
                .lock()
                .values()
                .filter(|b| !b.stopped.load(Ordering::Acquire))
                .count()
                >= 32
            {
                return Err("Too many automation sessions; release a session first".into());
            }
            let _startup = state.startup.lock();
            crate::ensure_runtime(app, state)?;
            let id = crate::spawn_tab(app, state, window, 24, 80, Some(profile))?;
            state.windows.lock().select(window, &id);
            state.hub.set_attended(&id);
            let session = state.hub.get(&id).ok_or("Session closed")?;
            let name = format!("AppleScript · {}", caller.name);
            let conn = {
                let mut s = session.lock();
                // Creating an external session never silently disables its control gate.
                s.set_control_gate(true);
                let conn = s.subscribe_agent(&name, Box::new(|_| {}));
                s.audit().record(
                    &name,
                    "automation_opened",
                    json!({"session":id,"profile":profile,"origin":"external_automation"}),
                );
                conn
            };
            let handle = uuid::Uuid::new_v4().to_string();
            let b = Arc::new(Binding {
                owner: caller.identity,
                name: name.clone(),
                window: window.into(),
                session_id: id.clone(),
                handle: handle.clone(),
                conn,
                session,
                stopped: AtomicBool::new(false),
                operation: Default::default(),
                queue: Default::default(),
                wake: Default::default(),
            });
            state
                .automation
                .bindings
                .lock()
                .insert(handle.clone(), b.clone());
            let weak = Arc::downgrade(state);
            std::thread::spawn(move || worker(weak, b));
            app.emit(
                "ss:tab_opened",
                json!({"session":id,"agentId":name,"focus":true,"origin":"external_automation"}),
            )?;
            Ok(json!(handle))
        }
        "session.write" => {
            let id = field(&args, "session")?;
            let b = binding(state, &caller, &id)?;
            let text = field(&args, "text")?;
            // No blind password logging and no embedded Return/control-key bypass.
            if args.get("sensitive").and_then(Value::as_bool) == Some(true) {
                return Err(
                    "Sensitive input is not supported; use SSH-managed authentication".into(),
                );
            }
            validate_text(&text)?;
            let newline = args.get("newline").and_then(Value::as_bool).unwrap_or(true);
            let release_after_delivery = args
                .get("releaseAfterDelivery")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let intent = args
                .get("intent")
                .and_then(Value::as_str)
                .filter(|v| !v.trim().is_empty())
                .unwrap_or("External application requested terminal input")
                .to_owned();
            if intent.len() > 4096 {
                return Err("Intent too long".into());
            }
            let request_id = args
                .get("requestId")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            if request_id.is_empty()
                || request_id.len() > 128
                || request_id.chars().any(char::is_control)
            {
                return Err("Invalid request ID".into());
            }
            // A stop must not let the worker exit between validation and enqueue.
            let _operation = b.operation.lock();
            if b.stopped.load(Ordering::Acquire) {
                return Err("Automation session was released".into());
            }
            let mut requests = state.automation.requests.lock();
            if let Some(j) = requests.get(&request_id) {
                if j.owner == caller.identity
                    && j.binding.handle == id
                    && j.text == text
                    && j.newline == newline
                    && j.release_after_delivery == release_after_delivery
                    && j.intent == intent
                {
                    return Ok(json!(request_id));
                }
                return Err("Request ID conflicts with another request".into());
            }
            if requests.len() >= MAX_REQUESTS {
                // Keep deduplication exact for the lifetime of the instance. Never evict IDs.
                return Err(
                    "Request capacity reached; restart Conn before submitting more automation"
                        .into(),
                );
            }
            let mut queue = b.queue.lock();
            if queue.len() >= MAX_QUEUE {
                return Err("Session request queue is full".into());
            }
            let job = Arc::new(Job {
                id: request_id.clone(),
                owner: caller.identity,
                binding: b.clone(),
                text,
                newline,
                release_after_delivery,
                intent,
                created: Instant::now(),
                cancelled: AtomicBool::new(false),
                result: Mutex::new(Value::Null),
            });
            job.update("queued", Value::Null);
            requests.insert(request_id.clone(), job.clone());
            queue.push_back(job);
            b.wake.notify_one();
            Ok(json!(request_id))
        }
        "request.status" => Ok(request(state, &caller, &field(&args, "requestId")?)?
            .result
            .lock()
            .clone()),
        "request.cancel" => {
            let j = request(state, &caller, &field(&args, "requestId")?)?;
            if j.done() {
                return Ok(json!(false));
            }
            j.cancelled.store(true, Ordering::Release);
            // Revoke this automation session so queued writes cannot reacquire after cancellation.
            j.binding.stop();
            Ok(json!(true))
        }
        "session.release" => {
            binding(state, &caller, &field(&args, "session")?)?.stop();
            Ok(json!(true))
        }
        "session.status" => {
            let b = binding(state, &caller, &field(&args, "session")?)?;
            let s = b.session.lock();
            Ok(
                json!({"session":b.handle,"tab":b.session_id,"mode":s.mode(),"effectiveMode":s.effective_mode(),"processAlive":s.process_alive(),"attended":s.attended()}),
            )
        }
        _ => Err("Unsupported automation operation".into()),
    }
}

fn worker(state: Weak<AppState>, b: Arc<Binding>) {
    let mut has_had_control = false;
    loop {
        let job = {
            let mut queue = b.queue.lock();
            if queue.is_empty() && !b.stopped.load(Ordering::Acquire) {
                b.wake.wait_for(&mut queue, Duration::from_secs(1));
            }
            queue.pop_front()
        };
        let Some(job) = job else {
            if b.stopped.load(Ordering::Acquire) || state.strong_count() == 0 {
                b.stop();
                break;
            }
            continue;
        };
        let outcome = run_job(&state, &job, &mut has_had_control);
        match outcome {
            Ok(detail) => {
                if job.release_after_delivery {
                    b.stop();
                }
                job.update("delivered", detail);
            }
            Err((status, detail)) => {
                job.update(status, json!(detail));
                b.stop();
            }
        }
    }
}
type Failure = (&'static str, String);
fn check(state: &Weak<AppState>, job: &Job) -> Result<(), Failure> {
    if job.cancelled.load(Ordering::Acquire) || job.binding.stopped.load(Ordering::Acquire) {
        return Err(("cancelled", "Automation stopped".into()));
    }
    if job.created.elapsed() >= TIMEOUT {
        return Err((
            "cancelled",
            "Request timed out; inspect the terminal before retrying".into(),
        ));
    }
    if state.strong_count() == 0 || !job.binding.session.lock().process_alive() {
        return Err(("failed", "Session closed".into()));
    }
    Ok(())
}
fn pause() {
    std::thread::sleep(Duration::from_millis(25));
}
fn call(state: &Weak<AppState>, job: &Job, method: &str, params: Value) -> Result<Value, Failure> {
    loop {
        check(state, job)?;
        let response = {
            let _operation = job.binding.operation.lock();
            check(state, job)?;
            conn_core::ipc::dispatch(&job.binding.session, job.binding.conn, method, &params)
        };
        match response {
            Ok(v) => return Ok(v),
            Err(e) if e.code == "rate_limited" => pause(),
            Err(e) => return Err(("failed", format!("{}: {}", e.code, e.message))),
        }
    }
}
fn run_job(
    state: &Weak<AppState>,
    job: &Job,
    has_had_control: &mut bool,
) -> Result<Value, Failure> {
    check(state, job)?;
    while !state
        .upgrade()
        .is_some_and(|s| s.windows.lock().ready(&job.binding.window))
    {
        check(state, job)?;
        pause();
    }
    let b = &job.binding;
    if *has_had_control {
        if b.session
            .lock()
            .current_lease()
            .is_none_or(|l| l.conn != b.conn || l.expires_at <= Instant::now())
        {
            return Err((
                "cancelled",
                "Control was returned to the human; create a new automation session".into(),
            ));
        }
    } else {
        job.update("awaiting_permission", Value::Null);
        let ctl = call(
            state,
            job,
            "request_control",
            json!({"reason":job.intent,"command":job.text,"origin":"external_automation","requestId":job.id}),
        )?;
        if ctl["status"] == "pending" {
            let id = ctl["requestId"].as_str().unwrap_or_default();
            loop {
                check(state, job)?;
                match b.session.lock().control_request_state(id) {
                    Some(ControlRequestState::Pending) => {}
                    Some(ControlRequestState::Granted) => break,
                    _ => return Err(("denied", "Control request was not granted".into())),
                }
                pause();
            }
        }
        *has_had_control = true;
    }
    job.update("delivering", Value::Null);
    call(state, job, "type", json!({"text":job.text}))?;
    if !job.newline {
        let proposed = b.session.lock().effective_mode() == conn_core::session::AgentMode::Copilot;
        return Ok(json!({"inputDelivered":!proposed,"proposed":proposed,"executed":false}));
    }
    let sent = call(
        state,
        job,
        "send_key",
        json!({"key":"ENTER","intent":job.intent}),
    )?;
    match sent["status"].as_str() {
        Some("executed") => Ok(json!({"inputDelivered":true})),
        Some("denied" | "rejected" | "cancelled") => {
            Err(("denied", "Command was not executed".into()))
        }
        Some("scheduled") => {
            job.update("grace", Value::Null);
            loop {
                check(state, job)?;
                match b
                    .session
                    .lock()
                    .exec_state(sent["execId"].as_str().unwrap_or_default())
                {
                    Some((ExecState::Scheduled, _)) => {}
                    Some((ExecState::Executed, _)) => return Ok(json!({"inputDelivered":true})),
                    _ => return Err(("cancelled", "Scheduled execution cancelled".into())),
                }
                pause();
            }
        }
        Some("proposed") => {
            job.update("awaiting_acceptance", Value::Null);
            loop {
                check(state, job)?;
                match b
                    .session
                    .lock()
                    .proposal_state(sent["proposalId"].as_str().unwrap_or_default())
                {
                    Some(ProposalState::Ready | ProposalState::Drafting) => {}
                    Some(ProposalState::Executed) => return Ok(json!({"inputDelivered":true})),
                    _ => return Err(("denied", "Proposal was not accepted".into())),
                }
                pause();
            }
        }
        Some("pending") => {
            job.update("awaiting_approval", Value::Null);
            loop {
                check(state, job)?;
                let approval = call(
                    state,
                    job,
                    "check_approval",
                    json!({"approvalId":sent["approvalId"]}),
                )?;
                match approval["state"].as_str() {
                    Some("pending") => {}
                    Some("granted") => return Ok(json!({"inputDelivered":true})),
                    _ => return Err(("denied", "Command approval was not granted".into())),
                }
                pause();
            }
        }
        _ => Err(("failed", "Unexpected execution result".into())),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::Harness;
    use conn_core::backend::Profile;
    fn caller() -> Caller {
        Caller {
            identity: "test-sender:1".into(),
            name: "PAM test".into(),
        }
    }
    fn harness() -> (tempfile::TempDir, Harness) {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("config");
        std::fs::create_dir_all(&config).unwrap();
        let mut p = Profile::local("test-shell".into(), "/bin/sh".into());
        p.args = vec!["-i".into()];
        p.env.insert("ENV".into(), "/dev/null".into());
        p.env.insert("PS1".into(), "automation-test$ ".into());
        let profiles = conn_core::profiles::Profiles {
            version: 1,
            revision: 0,
            default_profile: p.id.clone(),
            profiles: vec![p],
        };
        std::fs::write(
            config.join("profiles.json"),
            serde_json::to_vec(&profiles).unwrap(),
        )
        .unwrap();
        let h = Harness::new(config, tmp.path().join("conn.sock"), Arc::new(|_, _| {}));
        h.invoke("set_defaults", json!({"defaults":{"mode":"autopilot","pacing":{"enterGraceMs":0,"minWriteIntervalMs":0}}})).unwrap();
        (tmp, h)
    }
    fn enable(h: &Harness) {
        h.invoke(
            "automation_save",
            json!({"config":{"enabled":true,"profiles":["test-shell"]}}),
        )
        .unwrap();
    }
    fn open(h: &Harness) -> String {
        h.automate(caller(), "session.create", json!({}))
            .unwrap()
            .as_str()
            .unwrap()
            .into()
    }
    fn wait_for(mut condition: impl FnMut() -> bool) {
        let end = Instant::now() + Duration::from_secs(5);
        while !condition() {
            assert!(Instant::now() < end, "timed out");
            pause();
        }
    }
    fn physical(h: &Harness, handle: &str) -> String {
        h.state
            .automation
            .bindings
            .lock()
            .get(handle)
            .unwrap()
            .session_id
            .clone()
    }
    fn grant(h: &Harness, handle: &str) {
        let id = physical(h, handle);
        let window = h.state.windows.lock().owner(&id).unwrap();
        let mut request_id = String::new();
        wait_for(|| {
            let status = h
                .invoke_in_window(&window, "status", json!({"session":id}))
                .unwrap();
            if let Some(id) = status["controlRequests"][0]["requestId"].as_str() {
                request_id = id.into();
                true
            } else {
                false
            }
        });
        h.invoke_in_window(
            &window,
            "decide_control",
            json!({"session":id,"requestId":request_id,"grant":true}),
        )
        .unwrap();
    }
    fn state(h: &Harness, req: &str) -> String {
        h.automate(caller(), "request.status", json!({"requestId":req}))
            .unwrap()["state"]
            .as_str()
            .unwrap()
            .into()
    }

    #[test]
    fn disabled_scope_and_unknown_operations_are_closed() {
        let (_tmp, h) = harness();
        assert!(h.automate(caller(), "session.create", json!({})).is_err());
        h.invoke(
            "automation_save",
            json!({"config":{"enabled":true,"profiles":[]}}),
        )
        .unwrap();
        assert!(h.automate(caller(), "session.create", json!({})).is_err());
        assert!(h
            .automate(caller(), "input", json!({"data":"echo bypass\r"}))
            .is_err());
        assert!(h
            .invoke(
                "automation_save",
                json!({"config":{"enabled":true,"profiles":["missing"]}})
            )
            .is_err());
    }

    #[test]
    fn cold_start_gate_delivery_and_idempotency_use_one_session() {
        let (_tmp, h) = harness();
        enable(&h);
        let id = open(&h);
        let started = h.invoke("start", json!({"rows":24,"cols":80})).unwrap();
        assert_eq!(started["sessions"], json!([physical(&h, &id)]));
        let args = json!({"session":id,"text":"printf 'automation-proof\\n'","requestId":"once"});
        assert_eq!(
            h.automate(caller(), "session.write", args.clone()).unwrap(),
            json!("once")
        );
        assert_eq!(state(&h, "once"), "queued");
        h.invoke("ui_ready", json!({})).unwrap();
        grant(&h, &id);
        wait_for(|| state(&h, "once") == "delivered");
        assert_eq!(
            h.automate(caller(), "session.write", args).unwrap(),
            json!("once")
        );
        assert_eq!(h.state.automation.requests.lock().len(), 1);
        assert!(h
            .automate(
                caller(),
                "session.write",
                json!({"session":id,"text":"pwd","requestId":"once"})
            )
            .is_err());
        let s = h.state.hub.get(&physical(&h, &id)).unwrap();
        wait_for(|| {
            serde_json::to_string(
                &s.lock()
                    .snapshot(conn_core::affordance::Actor::Human)
                    .unwrap(),
            )
            .unwrap()
            .contains("automation-proof")
        });
        assert!(s.lock().status().control_gate);
    }

    #[test]
    fn cancellation_revokes_queue_without_reacquiring_and_other_callers_cannot_use_it() {
        let (_tmp, h) = harness();
        enable(&h);
        let id = open(&h);
        let a = json!({"session":id,"text":"echo queued","requestId":"cancel-me"});
        h.automate(caller(), "session.write", a.clone()).unwrap();
        let other = Caller {
            identity: "other-process".into(),
            name: "PAM test".into(),
        };
        assert!(h.automate(other.clone(), "session.write", a).is_err());
        assert!(h
            .automate(other, "request.status", json!({"requestId":"cancel-me"}))
            .is_err());
        h.automate(caller(), "request.cancel", json!({"requestId":"cancel-me"}))
            .unwrap();
        h.invoke("ui_ready", json!({})).unwrap();
        wait_for(|| state(&h, "cancel-me") == "cancelled");
        assert!(h
            .state
            .hub
            .get(&physical(&h, &id))
            .unwrap()
            .lock()
            .status()
            .control_requests
            .is_empty());
        assert!(h
            .automate(
                caller(),
                "session.write",
                json!({"session":id,"text":"echo later"})
            )
            .is_err());
    }

    #[test]
    fn takeover_during_grace_cancels_following_input() {
        let (_tmp, h) = harness();
        enable(&h);
        let id = open(&h);
        h.invoke(
            "set_pacing",
            json!({"session":physical(&h,&id),"patch":{"enterGraceMs":3000}}),
        )
        .unwrap();
        h.invoke("ui_ready", json!({})).unwrap();
        h.automate(
            caller(),
            "session.write",
            json!({"session":id,"text":"echo first","requestId":"first"}),
        )
        .unwrap();
        h.automate(
            caller(),
            "session.write",
            json!({"session":id,"text":"echo second","requestId":"second"}),
        )
        .unwrap();
        grant(&h, &id);
        wait_for(|| state(&h, "first") == "grace");
        h.invoke("take", json!({"session":physical(&h,&id)}))
            .unwrap();
        wait_for(|| state(&h, "first") == "cancelled" && state(&h, "second") == "cancelled");
        assert!(!h
            .state
            .hub
            .get(&physical(&h, &id))
            .unwrap()
            .lock()
            .input_line()
            .contains("second"));
    }

    #[test]
    fn policy_denial_stops_following_write() {
        let (_tmp, h) = harness();
        std::fs::write(h.state.config_dir.join("policy.yaml"), "default: deny\n").unwrap();
        enable(&h);
        let id = open(&h);
        h.invoke("ui_ready", json!({})).unwrap();
        h.automate(
            caller(),
            "session.write",
            json!({"session":id,"text":"echo blocked","requestId":"blocked"}),
        )
        .unwrap();
        h.automate(
            caller(),
            "session.write",
            json!({"session":id,"text":"echo queued","requestId":"queued"}),
        )
        .unwrap();
        grant(&h, &id);
        wait_for(|| state(&h, "blocked") == "denied" && state(&h, "queued") == "cancelled");
        let events =
            conn_core::audit::read_events(&h.state.config_dir.join("audit.jsonl")).unwrap();
        assert!(events
            .iter()
            .any(|e| e.action == "exec" && e.fields["policy"] == "deny"));
        assert!(!events
            .iter()
            .any(|e| e.action == "exec" && e.fields["cmd"] == "echo queued"));
    }

    #[test]
    fn separate_automation_sessions_have_distinct_live_connections() {
        let (_tmp, h) = harness();
        enable(&h);
        let first = open(&h);
        let second = open(&h);
        let rows = crate::diagnostics::connections(&h.state.hub);
        assert_eq!(rows.len(), 2);
        assert_ne!(rows[0]["connId"], rows[1]["connId"]);
        assert_ne!(first, second);
        assert!(rows
            .iter()
            .all(|r| r["sessions"].as_array().unwrap().len() == 1));
    }

    #[test]
    fn immediate_shell_output_is_replayed_once_before_live_output() {
        use base64::Engine as _;
        let (_tmp, original) = harness();
        let config = original.state.config_dir.clone();
        let socket = original.state.socket.clone();
        drop(original);
        let profile_path = config.join("profiles.json");
        let mut profiles = conn_core::profiles::Profiles::load(&profile_path).unwrap();
        profiles.profiles[0].args = vec![
            "-c".into(),
            "printf 'startup-output-proof\\n'; exec /bin/sh -i".into(),
        ];
        std::fs::write(profile_path, serde_json::to_vec(&profiles).unwrap()).unwrap();
        let received = Arc::new(Mutex::new(Vec::<u8>::new()));
        let capture = received.clone();
        let h = Harness::new(
            config,
            socket,
            Arc::new(move |event, value| {
                if event == "ss:output" {
                    capture.lock().extend(
                        base64::engine::general_purpose::STANDARD
                            .decode(value["data"].as_str().unwrap())
                            .unwrap(),
                    );
                }
            }),
        );
        enable(&h);
        let handle = open(&h);
        let tab = physical(&h, &handle);
        let session = h.state.hub.get(&tab).unwrap();
        wait_for(|| {
            serde_json::to_string(
                &session
                    .lock()
                    .snapshot(conn_core::affordance::Actor::Human)
                    .unwrap(),
            )
            .unwrap()
            .contains("startup-output-proof")
        });
        assert!(received.lock().is_empty(), "xterm has not attached yet");
        h.invoke("attach_output", json!({"session":tab})).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&received.lock())
                .matches("startup-output-proof")
                .count(),
            1
        );
        h.invoke("attach_output", json!({"session":tab})).unwrap();
        h.invoke(
            "input",
            json!({"session":tab,
            "data":"printf '%s%s\\n' live-output- proof\n"}),
        )
        .unwrap();
        wait_for(|| String::from_utf8_lossy(&received.lock()).contains("live-output-proof"));
        let bytes = received.lock();
        let output = String::from_utf8_lossy(&bytes);
        assert_eq!(output.matches("startup-output-proof").count(), 1);
        assert_eq!(output.matches("live-output-proof").count(), 1);
    }

    #[test]
    fn concurrent_release_and_writes_leave_no_stranded_requests() {
        let (_tmp, h) = harness();
        enable(&h);
        let h = Arc::new(h);
        for round in 0..8 {
            let handle = open(&h);
            let barrier = Arc::new(std::sync::Barrier::new(5));
            let mut threads = Vec::new();
            for writer in 0..4 {
                let (h, handle, barrier) = (h.clone(), handle.clone(), barrier.clone());
                threads.push(std::thread::spawn(move || {
                    barrier.wait();
                    h.automate(caller(), "session.write", json!({"session":handle,
                        "text":"echo should-not-run", "requestId":format!("race-{round}-{writer}")}))
                }));
            }
            barrier.wait();
            h.automate(caller(), "session.release", json!({"session":handle}))
                .unwrap();
            for thread in threads {
                if let Ok(request) = thread.join().unwrap() {
                    wait_for(|| state(&h, request.as_str().unwrap()) == "cancelled");
                }
            }
            assert!(h
                .state
                .hub
                .get(&physical(&h, &handle))
                .unwrap()
                .lock()
                .status()
                .control_requests
                .is_empty());
        }
    }

    #[test]
    fn window_launch_waits_for_its_ui_runs_the_exact_wrapper_and_returns_control() {
        let (tmp, h) = harness();
        std::fs::write(h.state.config_dir.join("policy.yaml"), "default: allow\n").unwrap();
        enable(&h);
        let main = h.invoke("start", json!({"rows":24,"cols":80})).unwrap();
        h.invoke("ui_ready", json!({})).unwrap();
        let started = tmp.path().join("started");
        let finished = tmp.path().join("finished");
        let command = format!("/bin/sh -c 'printf started > \"{}\"; echo \"Press [Enter] key to exit.\"; read ANSWER; printf done > \"{}\"'", started.display(), finished.display());
        let launch = h
            .automate_in_window(
                "automation-1",
                caller(),
                "window.create",
                json!({"command":command}),
            )
            .unwrap();
        let handle = launch["session"].as_str().unwrap();
        let request = launch["requestId"].as_str().unwrap();
        let tab = physical(&h, handle);
        assert_eq!(state(&h, request), "queued");
        assert!(!started.exists());
        let boot = h
            .invoke_in_window("automation-1", "start", json!({"rows":24,"cols":80}))
            .unwrap();
        assert_eq!(boot["sessions"], json!([tab]));
        assert_eq!(
            state(&h, request),
            "queued",
            "another window being ready must not start this input"
        );
        h.invoke_in_window("automation-1", "ui_ready", json!({}))
            .unwrap();
        grant(&h, handle);
        // Opaque shell wrappers may require per-command review as well as control.
        wait_for(|| {
            let status = h
                .invoke_in_window("automation-1", "status", json!({"session":tab}))
                .unwrap();
            if let Some(id) = status["pending"][0]["id"].as_str() {
                h.invoke_in_window(
                    "automation-1",
                    "approve",
                    json!({"session":tab,"approvalId":id,"decision":"grant"}),
                )
                .unwrap();
            }
            state(&h, request) == "delivered"
        });
        wait_for(|| std::fs::read_to_string(&started).ok().as_deref() == Some("started"));
        assert!(
            !finished.exists(),
            "the wrapper must still be waiting in read"
        );
        assert_eq!(
            h.invoke_in_window("automation-1", "status", json!({"session":tab}))
                .unwrap()["controller"]["type"],
            "human"
        );
        h.invoke_in_window("automation-1", "input", json!({"session":tab,"data":"\n"}))
            .unwrap();
        wait_for(|| std::fs::read_to_string(&finished).ok().as_deref() == Some("done"));
        let events =
            conn_core::audit::read_events(&h.state.config_dir.join("audit.jsonl")).unwrap();
        assert!(events
            .iter()
            .any(|e| e.action == "exec" && e.fields["cmd"] == command));
        h.close_window("automation-1");
        assert_eq!(
            h.invoke("status", json!({"session":main["session"]}))
                .unwrap()["processAlive"],
            true
        );
    }

    #[test]
    fn window_launch_preserves_copilot_acceptance_before_delivery() {
        let (_tmp, h) = harness();
        enable(&h);
        h.invoke("set_defaults", json!({"defaults":{"mode":"copilot","pacing":{"enterGraceMs":0,"minWriteIntervalMs":0}}})).unwrap();
        let launch = h
            .automate_in_window(
                "automation-1",
                caller(),
                "window.create",
                json!({"command":"echo copilot-launch"}),
            )
            .unwrap();
        let handle = launch["session"].as_str().unwrap();
        let request = launch["requestId"].as_str().unwrap();
        let tab = physical(&h, handle);
        h.invoke_in_window("automation-1", "ui_ready", json!({}))
            .unwrap();
        grant(&h, handle);
        wait_for(|| state(&h, request) == "awaiting_acceptance");
        let s = h.state.hub.get(&tab).unwrap();
        assert!(s.lock().input_line().is_empty());
        let status = h
            .invoke_in_window("automation-1", "status", json!({"session":tab}))
            .unwrap();
        assert_eq!(status["proposal"]["text"], "echo copilot-launch");
        h.invoke_in_window(
            "automation-1",
            "accept_proposal",
            json!({"session":tab,"proposalId":status["proposal"]["proposalId"]}),
        )
        .unwrap();
        wait_for(|| state(&h, request) == "delivered");
        assert!(s.lock().current_lease().is_none());
    }

    #[test]
    fn rejected_window_commands_do_not_create_orphan_shells() {
        let (_tmp, h) = harness();
        enable(&h);
        for args in [
            json!({"command":"pwd\necho bypass"}),
            json!({"command":7}),
            json!({"command":"secret","sensitive":true}),
        ] {
            assert!(h
                .automate_in_window("automation-1", caller(), "window.create", args)
                .is_err());
            assert!(h.state.hub.ids().is_empty());
        }
        h.invoke(
            "automation_save",
            json!({"config":{"enabled":true,"profiles":[]}}),
        )
        .unwrap();
        assert!(h
            .automate_in_window(
                "automation-1",
                caller(),
                "window.create",
                json!({"command":"pwd"})
            )
            .is_err());
        assert!(h.state.hub.ids().is_empty());
    }

    #[test]
    fn closing_a_window_cancels_its_pending_launch() {
        let (_tmp, h) = harness();
        enable(&h);
        let launch = h
            .automate_in_window(
                "automation-1",
                caller(),
                "window.create",
                json!({"command":"echo never"}),
            )
            .unwrap();
        let request = launch["requestId"].as_str().unwrap();
        h.close_window("automation-1");
        wait_for(|| state(&h, request) == "cancelled");
        assert!(h.state.hub.ids().is_empty());
        assert!(h
            .invoke_in_window("automation-1", "ui_ready", json!({}))
            .is_err());
    }

    #[test]
    fn multiline_control_characters_and_sensitive_input_never_enter_queue() {
        let (_tmp, h) = harness();
        enable(&h);
        let id = open(&h);
        for text in ["echo a\necho b", "echo a\recho b", "echo a\u{1b}[D", "\t"] {
            assert!(h
                .automate(caller(), "session.write", json!({"session":id,"text":text}))
                .is_err());
        }
        assert!(h
            .automate(
                caller(),
                "session.write",
                json!({"session":id,"text":"secret","sensitive":true})
            )
            .is_err());
        assert!(h.state.automation.requests.lock().is_empty());
        h.invoke("automation_revoke", json!({})).unwrap();
        assert!(h
            .automate(caller(), "session.status", json!({"session":id}))
            .is_err());
    }
}
