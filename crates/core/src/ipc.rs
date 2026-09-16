//! Local socket/named-pipe control plane. Newline-delimited JSON.
//!
//! Request:  `{"id":1,"method":"snapshot","params":{}}`
//! Response: `{"id":1,"result":{...}}` or `{"id":1,"error":{"code":"...","message":"..."}}`
//! Event:    `{"event":"control_revoked", ...}` (no id; pushed by the server)
//!
//! Three kinds of client, declared with `hello`:
//! * `agent`    — MCP adapter. Gets `tools_changed` and events about its own lease/approvals.
//! * `human`    — CLI. No events.
//! * `frontend` — a UI. Gets every event; with `streamOutput: true` also raw PTY output
//!   (`output` events, base64) and may send `input` / `resize`.
//!
//! See docs/protocol.md for the full method list.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::affordance::{Actor, Affordance};
use crate::approval::Decision as ApprovalDecision;
use crate::authority::{AuthorityError, ConnId};
use crate::config::Pacing;
use crate::session::{AgentMode, ConnKind, ControlRequestState, EventSink, ExecState, KeyResult, ProposalState, ServerEvent, SessionError, SharedSession};

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl From<SessionError> for RpcError {
    fn from(e: SessionError) -> Self {
        let code = match &e {
            SessionError::Authority(AuthorityError::Busy { .. }) => "busy",
            SessionError::Authority(AuthorityError::NotController) => "not_controller",
            SessionError::Authority(AuthorityError::Expired) => "lease_expired",
            SessionError::ProcessExited => "process_exited",
            SessionError::InputPending => "input_pending",
            SessionError::InvalidInput(_) => "invalid_input",
            SessionError::NotFound(_) => "not_found",
            SessionError::ApprovalPending(_) => "approval_pending",
            SessionError::ExecPending(_) => "exec_pending",
            SessionError::RateLimited { .. } => "rate_limited",
            SessionError::Masked(_) => "masked",
            SessionError::ControlDenied(_) => "control_denied",
            SessionError::WrongMode(_) => "wrong_mode",
            SessionError::ProposalPending(_) => "proposal_pending",
            SessionError::IntentRequired => "intent_required",
            SessionError::Unattended => "unattended",
            SessionError::Suspended => "suspended",
            SessionError::NotAvailable(_) => "not_available",
            SessionError::Io(_) => "io",
        };
        RpcError { code: code.into(), message: e.to_string() }
    }
}

fn str_param(params: &Value, key: &str) -> Option<String> {
    params.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn bytes_param(params: &Value, key: &str) -> Result<Vec<u8>, RpcError> {
    let s = str_param(params, key).unwrap_or_default();
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| RpcError { code: "invalid_input".into(), message: format!("{key}: not base64 ({e})") })
}

/// Synchronous dispatch. Runs under the session lock; must not block.
pub fn dispatch(session: &SharedSession, conn: ConnId, method: &str, params: &Value) -> Result<Value, RpcError> {
    let mut s = session.lock();
    s.note_connection_activity(conn);
    let kind = s.conn_kind(conn).unwrap_or(ConnKind::Human);
    let actor = match kind {
        ConnKind::Agent => Actor::Agent { conn },
        _ => Actor::Human,
    };
    let r: Result<Value, SessionError> = match method {
        "affordances" => Ok(json!(s.affordances(actor).iter().map(|a| a.name()).collect::<Vec<_>>())),
        "snapshot" => s.snapshot(actor).map(|v| serde_json::to_value(v).unwrap()),
        "request_control" => {
            s.agent_request_control_original(conn, params.clone()).map(|o| serde_json::to_value(o).unwrap())
        }
        "control_request_state" => {
            let id = str_param(params, "requestId").unwrap_or_default();
            match s.control_request_state(&id) {
                Some(state) => Ok(json!({ "requestId": id, "state": state })),
                None => Err(SessionError::NotFound(id)),
            }
        }
        "proposal_state" => {
            let id = str_param(params, "proposalId").unwrap_or_default();
            match s.proposal_state(&id) {
                Some(state) => Ok(json!({ "proposalId": id, "state": state })),
                None => Err(SessionError::NotFound(id)),
            }
        }
        "release_control" => s.agent_release_control(conn).map(|_| json!({ "released": true })),
        "type" => {
            let text = str_param(params, "text").unwrap_or_default();
            s.agent_type(conn, &text).map(|_| json!({ "typed": text.chars().count() }))
        }
        "send_key" => {
            let key = str_param(params, "key").unwrap_or_default();
            let intent = str_param(params, "intent");
            s.agent_send_key_with(conn, &key, intent).map(|r| serde_json::to_value(r).unwrap())
        }
        "analyse" => {
            let cmd = str_param(params, "cmd").unwrap_or_default();
            Ok(serde_json::to_value(s.analyse_line(&cmd)).unwrap())
        }
        "interrupt" => s.agent_interrupt(conn).map(|_| json!({ "interrupted": true })),
        "check_approval" => {
            let id = str_param(params, "approvalId").unwrap_or_default();
            s.check_approval(&id).map(|a| serde_json::to_value(a).unwrap())
        }
        "exec_state" => {
            let id = str_param(params, "execId").unwrap_or_default();
            match s.exec_state(&id) {
                Some((state, reason)) => Ok(json!({ "execId": id, "state": state, "reason": reason })),
                None => Err(SessionError::NotFound(id)),
            }
        }
        // ----- human / frontend -----
        "status" => Ok(serde_json::to_value(s.status()).unwrap()),
        "take" => Ok(json!({ "revoked": s.human_take() })),
        "approve" => {
            let decision = match str_param(params, "decision").as_deref() {
                Some("deny") => ApprovalDecision::Deny,
                Some("allow_session") => ApprovalDecision::AllowSession,
                _ => ApprovalDecision::Grant,
            };
            let by = if kind == ConnKind::Frontend { "frontend" } else { "cli" };
            // An approval must name what it approves. Approving "whatever is oldest"
            // is how an automated approver once granted someone else's rm -rf.
            match str_param(params, "approvalId") {
                Some(id) if !id.is_empty() => s.resolve_approval(&id, decision, by),
                _ => Err(SessionError::InvalidInput("approvalId is required; see `status` for pending ids".into())),
            }
            .map(|a| serde_json::to_value(a).unwrap())
        }
        "decide_control" => {
            let id = str_param(params, "requestId").unwrap_or_default();
            let grant = params.get("grant").and_then(|v| v.as_bool()).unwrap_or(false);
            s.decide_control(&id, grant).map(|r| serde_json::to_value(r).unwrap())
        }
        "accept_proposal" => {
            let id = str_param(params, "proposalId").unwrap_or_default();
            s.accept_proposal(&id).map(|r| serde_json::to_value(r).unwrap())
        }
        "reject_proposal" => {
            let id = str_param(params, "proposalId").unwrap_or_default();
            s.reject_proposal(&id).map(|_| json!({ "rejected": id }))
        }
        "hand_back" => s.hand_back().map(|l| serde_json::to_value(l).unwrap()),
        "entrust" => s.entrust().map(|a| json!({ "entrustedTo": a })),
        "request_attention" => s.agent_request_attention(conn, str_param(params, "reason")).map(|_| json!({ "requested": true })),
        "set_mode" => {
            match serde_json::from_value::<AgentMode>(params.get("mode").cloned().unwrap_or(Value::Null)) {
                Ok(m) => {
                    s.set_mode(m).map(|_| json!({ "mode": m, "effectiveMode": s.effective_mode() }))
                }
                Err(e) => Err(SessionError::InvalidInput(format!("mode: {e}; use observe | copilot | autopilot"))),
            }
        }
        "set_control_gate" => {
            let ask = params.get("ask").and_then(|v| v.as_bool()).unwrap_or(false);
            s.set_control_gate(ask);
            Ok(json!({ "ask": ask }))
        }
        "revoke_session_allow" => {
            let label = str_param(params, "label").unwrap_or_default();
            Ok(json!({ "revoked": s.revoke_session_allow(&label) }))
        }
        "execute_now" => {
            let id = str_param(params, "execId").unwrap_or_default();
            s.execute_now_scheduled(&id).map(|_| json!({ "executed": id }))
        }
        "cancel_exec" => {
            let id = str_param(params, "execId").unwrap_or_default();
            s.cancel_exec(&id).map(|_| json!({ "cancelled": id }))
        }
        "input" => {
            if kind == ConnKind::Agent {
                Err(SessionError::InvalidInput("agents must use type/send_key".into()))
            } else {
                match bytes_param(params, "data") {
                    Ok(b) => {
                        s.human_input(&b);
                        Ok(json!({ "written": b.len() }))
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        "resize" => {
            let rows = params.get("rows").and_then(|v| v.as_u64()).unwrap_or(24) as u16;
            let cols = params.get("cols").and_then(|v| v.as_u64()).unwrap_or(80) as u16;
            s.resize(rows, cols);
            Ok(json!({ "rows": rows, "cols": cols }))
        }
        "get_pacing" => Ok(serde_json::to_value(s.pacing()).unwrap()),
        "set_pacing" => {
            // partial update: merge onto the current pacing
            let mut cur = serde_json::to_value(s.pacing()).unwrap();
            if let (Some(c), Some(p)) = (cur.as_object_mut(), params.as_object()) {
                for (k, v) in p {
                    c.insert(k.clone(), v.clone());
                }
            }
            match serde_json::from_value::<Pacing>(cur) {
                Ok(p) => {
                    s.set_pacing(p.clone());
                    Ok(serde_json::to_value(p).unwrap())
                }
                Err(e) => Err(SessionError::InvalidInput(e.to_string())),
            }
        }
        "set_affordances" => {
            let allow = match params.get("allow") {
                None | Some(Value::Null) => None,
                Some(v) => match serde_json::from_value::<Vec<Affordance>>(v.clone()) {
                    Ok(list) => Some(list.into_iter().collect()),
                    Err(e) => return Err(RpcError { code: "invalid_input".into(), message: e.to_string() }),
                },
            };
            s.set_affordance_mask(allow);
            Ok(json!({ "allow": s.affordance_mask() }))
        }
        other => Err(SessionError::InvalidInput(format!("unknown method '{other}'"))),
    };
    r.map_err(RpcError::from)
}

// ---------------------------------------------------------------------------
// Hub: several sessions (tabs) behind one socket
// ---------------------------------------------------------------------------

pub type SessionId = String;

/// Sessions behind one socket. Exactly one is *attended* — the one the human is
/// looking at — and that is where agents go unless they name a session.
/// Opens a new session (tab) on behalf of an agent: `(agent_id, reason) -> session id`.
/// The host spawns the shell, registers it with the hub and tells its UI.
pub type TabOpener = Arc<dyn Fn(&str, Option<&str>) -> Result<SessionId, String> + Send + Sync>;

pub struct Hub {
    sessions: parking_lot::Mutex<Vec<(SessionId, SharedSession)>>,
    attended: parking_lot::Mutex<Option<SessionId>>,
    opener: parking_lot::Mutex<Option<TabOpener>>,
}

pub type SharedHub = Arc<Hub>;

impl Hub {
    pub fn new() -> SharedHub {
        Arc::new(Self { sessions: parking_lot::Mutex::new(Vec::new()), attended: parking_lot::Mutex::new(None), opener: parking_lot::Mutex::new(None) })
    }

    pub fn single(id: &str, session: SharedSession) -> SharedHub {
        let hub = Self::new();
        hub.add(id, session);
        hub
    }

    /// Let agents open tabs. Without an opener `open_tab` is unsupported and the
    /// tab affordances are not offered.
    pub fn set_opener(&self, opener: TabOpener) {
        *self.opener.lock() = Some(opener);
        for (_, s) in self.sessions.lock().iter() {
            s.lock().set_tabs_supported(true);
        }
    }

    pub fn tabs_supported(&self) -> bool {
        self.opener.lock().is_some()
    }

    /// Open a tab for `agent_id`. Returns the new session's id.
    pub fn open_tab(&self, agent_id: &str, reason: Option<&str>) -> Result<SessionId, RpcError> {
        let opener = self.opener.lock().clone().ok_or_else(|| RpcError { code: "unsupported".into(), message: "this host cannot open tabs".into() })?;
        let id = opener(agent_id, reason).map_err(|m| RpcError { code: "open_failed".into(), message: m })?;
        if self.get(&id).is_none() {
            return Err(RpcError { code: "open_failed".into(), message: "opener did not register the session".into() });
        }
        Ok(id)
    }

    /// 1-based position of a session in the tab order.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.sessions.lock().iter().position(|(i, _)| i == id).map(|p| p + 1)
    }

    /// Find a session by id or by 1-based index.
    pub fn find_tab(&self, tab: &Value) -> Option<(SessionId, SharedSession)> {
        let list = self.sessions.lock();
        let hit = match tab {
            Value::Number(n) => n.as_u64().and_then(|n| list.get(n.checked_sub(1)? as usize)),
            Value::String(s) => list.iter().find(|(i, _)| i == s).or_else(|| s.parse::<usize>().ok().and_then(|n| list.get(n.checked_sub(1)?))),
            _ => None,
        };
        hit.map(|(i, s)| (i.clone(), s.clone()))
    }

    pub fn add(&self, id: &str, session: SharedSession) {
        let first = self.sessions.lock().is_empty();
        session.lock().set_tabs_supported(self.tabs_supported());
        self.sessions.lock().push((id.to_string(), session.clone()));
        if first {
            *self.attended.lock() = Some(id.to_string());
            session.lock().set_attended(true);
        } else {
            session.lock().set_attended(false);
        }
    }

    pub fn remove(&self, id: &str) -> Option<SharedSession> {
        let mut list = self.sessions.lock();
        let pos = list.iter().position(|(i, _)| i == id)?;
        let (_, s) = list.remove(pos);
        drop(list);
        if self.attended.lock().as_deref() == Some(id) {
            let next = self.sessions.lock().first().map(|(i, _)| i.clone());
            drop(self.attended.lock());
            if let Some(n) = next {
                self.set_attended(&n);
            } else {
                *self.attended.lock() = None;
            }
        }
        Some(s)
    }

    pub fn ids(&self) -> Vec<SessionId> {
        self.sessions.lock().iter().map(|(i, _)| i.clone()).collect()
    }

    pub fn get(&self, id: &str) -> Option<SharedSession> {
        self.sessions.lock().iter().find(|(i, _)| i == id).map(|(_, s)| s.clone())
    }

    pub fn attended_id(&self) -> Option<SessionId> {
        self.attended.lock().clone()
    }

    /// The human now looks at `id`. Every other session becomes unattended.
    pub fn set_attended(&self, id: &str) -> bool {
        let list: Vec<(SessionId, SharedSession)> = self.sessions.lock().clone();
        if !list.iter().any(|(i, _)| i == id) {
            return false;
        }
        *self.attended.lock() = Some(id.to_string());
        for (i, s) in list {
            s.lock().set_attended(i == id);
        }
        true
    }

    /// Resolve a request's target: the named session, else the attended one.
    pub fn resolve(&self, session: Option<&str>) -> Option<(SessionId, SharedSession)> {
        match session {
            Some(id) => self.get(id).map(|s| (id.to_string(), s)),
            None => {
                let id = self.attended_id().or_else(|| self.ids().first().cloned())?;
                self.get(&id).map(|s| (id, s))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Server (tokio)
// ---------------------------------------------------------------------------

/// Events leave a session tagged with its id.
struct TokioSink {
    session: SessionId,
    tx: tokio::sync::mpsc::UnboundedSender<(SessionId, ServerEvent)>,
}

impl EventSink for TokioSink {
    fn send(&self, event: ServerEvent) {
        let _ = self.tx.send((self.session.clone(), event));
    }
}

/// Serve the sessions on `path` until the future is dropped.
pub async fn serve(path: PathBuf, hub: SharedHub) -> std::io::Result<()> {
    let listener = crate::transport::Listener::bind(&path)?;
    serve_listener(listener, hub).await
}

async fn serve_listener(mut listener: crate::transport::Listener, hub: SharedHub) -> std::io::Result<()> {
    let mut next_conn: ConnId = 1;
    loop {
        let stream = listener.accept().await?;
        let conn = next_conn;
        next_conn += 1;
        let hub = hub.clone();
        tokio::spawn(async move {
            handle_conn(stream, conn, hub).await;
        });
    }
}

/// Run `serve` on a dedicated thread with its own runtime. For embedders that do
/// not want to manage tokio. Returns a guard; dropping it stops the server and
/// removes the socket file.
pub fn serve_in_background(path: PathBuf, hub: SharedHub) -> std::io::Result<ServerGuard> {
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?;
    let listener = { let _entered = rt.enter(); crate::transport::Listener::bind(&path)? };
    let handle = rt.spawn(serve_listener(listener, hub));
    Ok(ServerGuard { rt: Some(rt), handle: Some(handle), path })
}

pub struct ServerGuard {
    rt: Option<tokio::runtime::Runtime>,
    handle: Option<tokio::task::JoinHandle<std::io::Result<()>>>,
    path: PathBuf,
}

impl ServerGuard {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.abort();
        }
        if let Some(rt) = self.rt.take() {
            rt.shutdown_timeout(Duration::from_millis(200));
        }
        #[cfg(unix)]
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Per-connection identity, so a connection can be registered lazily in every
/// session it touches.
#[derive(Clone)]
struct ConnIdentity {
    kind: ConnKind,
    name: String,
    stream_output: bool,
}

async fn handle_conn(stream: crate::transport::Stream, conn: ConnId, hub: SharedHub) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let (rd, mut wr) = tokio::io::split(stream);
    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        while let Some(line) = out_rx.recv().await {
            if wr.write_all(line.as_bytes()).await.is_err() || wr.write_all(b"\n").await.is_err() {
                break;
            }
        }
    });
    let (ev_tx, mut ev_rx) = tokio::sync::mpsc::unbounded_channel::<(SessionId, ServerEvent)>();
    let out_ev = out_tx.clone();
    let forwarder = tokio::spawn(async move {
        while let Some((sid, ev)) = ev_rx.recv().await {
            if let Ok(mut v) = serde_json::to_value(&ev) {
                if let Some(o) = v.as_object_mut() {
                    o.insert("session".into(), Value::String(sid));
                }
                let _ = out_ev.send(v.to_string());
            }
        }
    });

    let mut identity = ConnIdentity { kind: ConnKind::Human, name: "cli".into(), stream_output: false };
    let mut registered: Vec<SessionId> = Vec::new();
    // A connection is bound to the session it first lands in (the attended one at
    // hello time). It does not drift when the human changes tabs — an agent whose
    // human walked away must see `unattended`, not another tab's screen. Requests may
    // still name a `session` explicitly.
    let mut bound: Option<SessionId> = None;
    let register = |sid: &SessionId, s: &SharedSession, id: &ConnIdentity, registered: &mut Vec<SessionId>| {
        if registered.iter().any(|r| r == sid) {
            return;
        }
        let sink = Box::new(TokioSink { session: sid.clone(), tx: ev_tx.clone() });
        let mut g = s.lock();
        if id.kind == ConnKind::Frontend {
            g.register_frontend(conn, &id.name, sink, id.stream_output);
        } else {
            g.register_conn(conn, id.kind, &id.name, sink);
        }
        registered.push(sid.clone());
    };

    let mut lines = BufReader::new(rd).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let _ = out_tx.send(
                    serde_json::to_string(&Response { id: 0, result: None, error: Some(RpcError { code: "parse".into(), message: e.to_string() }) }).unwrap(),
                );
                continue;
            }
        };
        for sid in &registered {
            if let Some(s) = hub.get(sid) { s.lock().note_connection_activity(conn); }
        }
        let result = if req.method == "hello" {
            let name = str_param(&req.params, "agentId").or_else(|| str_param(&req.params, "name")).unwrap_or_else(|| "client".into());
            let kind = match str_param(&req.params, "kind").as_deref() {
                Some("agent") => ConnKind::Agent,
                Some("frontend") => ConnKind::Frontend,
                _ => ConnKind::Human,
            };
            identity = ConnIdentity { kind, name, stream_output: req.params.get("streamOutput").and_then(|v| v.as_bool()).unwrap_or(false) };
            // Frontends hear every session; agents and humans attach lazily where they act.
            if kind == ConnKind::Frontend {
                for sid in hub.ids() {
                    if let Some(s) = hub.get(&sid) {
                        register(&sid, &s, &identity, &mut registered);
                    }
                }
            } else if let Some((sid, s)) = hub.resolve(None) {
                register(&sid, &s, &identity, &mut registered);
                bound = Some(sid);
            }
            let target = bound.clone().or_else(|| hub.attended_id());
            let modes = target.as_deref().and_then(|id| hub.get(id)).map(|s| { let s = s.lock(); (s.mode(), s.effective_mode()) });
            Ok(json!({ "conn": conn, "kind": match kind { ConnKind::Agent => "agent", ConnKind::Frontend => "frontend", ConnKind::Human => "human" }, "attended": hub.attended_id(), "session": target, "mode": modes.map(|m| m.0), "effectiveMode": modes.map(|m| m.1) }))
        } else if req.method == "sessions" || req.method == "list_tabs" {
            let list: Vec<Value> = hub.ids().into_iter().enumerate().filter_map(|(i, sid)| {
                let s = hub.get(&sid)?;
                let g = s.lock();
                let st = g.status();
                Some(json!({ "tab": i + 1, "id": sid, "current": bound.as_deref() == Some(sid.as_str()), "attended": st.attended, "controller": st.controller, "pending": st.pending.len(), "entrustedTo": st.entrusted_to, "attentionRequest": st.attention_request, "openedBy": st.opened_by, "processAlive": st.process_alive, "mode": st.mode, "effectiveMode": st.effective_mode }))
            }).collect();
            Ok(json!({ "sessions": list, "tabs": list.len(), "attended": hub.attended_id(), "current": bound }))
        } else if req.method == "open_tab" {
            // Agent: open a tab. It starts unattended; the human sees it knock.
            let reason = str_param(&req.params, "reason");
            let cur = bound.clone().and_then(|b| hub.get(&b).map(|s| (b, s))).or_else(|| hub.resolve(None));
            match cur {
                _ if !hub.tabs_supported() => Err(RpcError { code: "unsupported".into(), message: "this host cannot open tabs".into() }),
                Some((from, s)) => {
                    let allowed = { register(&from, &s, &identity, &mut registered); s.lock().agent_can(conn, Affordance::OpenTab) };
                    match allowed {
                        Err(e) => Err(e.into()),
                        Ok(()) => match hub.open_tab(&identity.name, reason.as_deref()) {
                            Err(e) => Err(e),
                            Ok(sid) => {
                                let ns = hub.get(&sid).expect("checked");
                                register(&sid, &ns, &identity, &mut registered);
                                ns.lock().agent_opened_tab(conn, reason);
                                bound = Some(sid.clone());
                                Ok(json!({ "session": sid, "tab": hub.index_of(&sid), "attended": false, "note": "the human is not looking at this tab yet; request_attention is queued — wait for attention or entrust" }))
                            }
                        },
                    }
                }
                None => Err(RpcError { code: "not_found".into(), message: "no session".into() }),
            }
        } else if req.method == "switch_tab" {
            let want = req.params.get("tab").cloned().or_else(|| req.params.get("session").cloned()).unwrap_or(Value::Null);
            let cur = bound.clone().and_then(|b| hub.get(&b).map(|s| (b, s))).or_else(|| hub.resolve(None));
            match (cur, hub.find_tab(&want)) {
                (None, _) => Err(RpcError { code: "not_found".into(), message: "no session".into() }),
                (_, None) => Err(RpcError { code: "not_found".into(), message: format!("no tab {want}") }),
                (Some((from, s)), Some((to, ts))) => {
                    let allowed = { register(&from, &s, &identity, &mut registered); s.lock().agent_can(conn, Affordance::SwitchTab) };
                    match allowed {
                        Err(e) => Err(e.into()),
                        Ok(()) => {
                            if from != to {
                                register(&to, &ts, &identity, &mut registered);
                                s.lock().agent_switched_tab(conn, &from, &to);
                                ts.lock().agent_switched_tab(conn, &from, &to);
                                bound = Some(to.clone());
                            }
                            let attended = ts.lock().status().attended;
                            Ok(json!({ "session": to, "tab": hub.index_of(&to), "attended": attended }))
                        }
                    }
                }
            }
        } else if req.method == "set_attended" {
            let sid = str_param(&req.params, "session").unwrap_or_default();
            if hub.set_attended(&sid) { Ok(json!({ "attended": sid })) } else { Err(RpcError { code: "not_found".into(), message: format!("session {sid}") }) }
        } else {
            let target = str_param(&req.params, "session").or_else(|| bound.clone());
            match hub.resolve(target.as_deref()) {
                Some((sid, s)) => {
                    register(&sid, &s, &identity, &mut registered);
                    if bound.is_none() {
                        bound = Some(sid);
                    }
                    call_with_pacing(&s, conn, &req.method, &req.params).await
                }
                None => Err(RpcError { code: "not_found".into(), message: "no such session".into() }),
            }
        };
        let resp = match result {
            Ok(v) => Response { id: req.id, result: Some(v), error: None },
            Err(e) => Response { id: req.id, result: None, error: Some(e) },
        };
        if out_tx.send(serde_json::to_string(&resp).unwrap()).is_err() {
            break;
        }
    }
    for sid in registered {
        if let Some(s) = hub.get(&sid) {
            s.lock().connection_closed(conn);
        }
    }
    forwarder.abort();
    drop(out_tx);
    let _ = writer.await;
}

/// Dispatch, absorbing pacing on behalf of the agent: rate limits become waits, and a
/// scheduled ENTER is awaited until it executes or is cancelled.
async fn call_with_pacing(session: &SharedSession, conn: ConnId, method: &str, params: &Value) -> Result<Value, RpcError> {
    if method == "send_key" && str_param(params, "key").map(|k| k.eq_ignore_ascii_case("ENTER")).unwrap_or(false) {
        // Let the shell echo settle so the VT cursor line reflects the typed text.
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
    let mut result = dispatch(session, conn, method, params);
    for _ in 0..200 {
        match &result {
            Err(e) if e.code == "rate_limited" => {
                let ms = e.message.split_whitespace().rev().nth(1).and_then(|s| s.parse::<u64>().ok()).unwrap_or(50);
                tokio::time::sleep(Duration::from_millis(ms.clamp(5, 5000))).await;
                result = dispatch(session, conn, method, params);
            }
            _ => break,
        }
    }
    if let Ok(v) = &result {
        // Gated request_control: wait for the human (up to the approval TTL).
        if method == "request_control" && v.get("status").and_then(|s| s.as_str()) == Some("pending") {
            let id = v["requestId"].as_str().unwrap_or_default().to_string();
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                let st = session.lock().control_request_state(&id);
                match st {
                    Some(ControlRequestState::Pending) => continue,
                    Some(ControlRequestState::Granted) => {
                        let s = session.lock();
                        return Ok(serde_json::to_value(s.status().controller).unwrap());
                    }
                    Some(other) => return Err(RpcError { code: "control_denied".into(), message: format!("control request {id} {other:?}") }),
                    None => break,
                }
            }
        }
        // Copilot proposal: wait for the human to commit or reject.
        if v.get("status").and_then(|s| s.as_str()) == Some("proposed") {
            let id = v["proposalId"].as_str().unwrap_or_default().to_string();
            let cmd = v["cmd"].as_str().unwrap_or_default().to_string();
            loop {
                tokio::time::sleep(Duration::from_millis(50)).await;
                let st = session.lock().proposal_state(&id);
                match st {
                    Some(ProposalState::Ready) | Some(ProposalState::Drafting) => continue,
                    Some(ProposalState::Executed) => return Ok(serde_json::to_value(KeyResult::Executed { cmd }).unwrap()),
                    Some(ProposalState::Rejected) => return Ok(serde_json::to_value(KeyResult::Rejected { cmd }).unwrap()),
                    Some(ProposalState::Denied) => return Ok(serde_json::to_value(KeyResult::Denied { cmd, label: "policy".into() }).unwrap()),
                    None => break,
                }
            }
        }
        if v.get("status").and_then(|s| s.as_str()) == Some("scheduled") {
            let exec_id = v["execId"].as_str().unwrap_or_default().to_string();
            let cmd = v["cmd"].as_str().unwrap_or_default().to_string();
            loop {
                tokio::time::sleep(Duration::from_millis(20)).await;
                let state = session.lock().exec_state(&exec_id);
                match state {
                    Some((ExecState::Scheduled, _)) => continue,
                    Some((ExecState::Executed, _)) => return Ok(serde_json::to_value(KeyResult::Executed { cmd }).unwrap()),
                    Some((ExecState::Cancelled, reason)) => {
                        return Ok(serde_json::to_value(KeyResult::Cancelled { cmd, reason: reason.unwrap_or_default() }).unwrap())
                    }
                    None => break,
                }
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Client (blocking, std)
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("conn proxy is not running ({0})")]
    NotRunning(String),
    #[error("connection lost")]
    Disconnected,
    #[error("{code}: {message}")]
    Rpc { code: String, message: String },
}

impl From<RpcError> for ClientError {
    fn from(e: RpcError) -> Self {
        ClientError::Rpc { code: e.code, message: e.message }
    }
}

type Pending = Arc<Mutex<HashMap<u64, mpsc::Sender<Response>>>>;

pub struct Client {
    writer: tokio::sync::mpsc::UnboundedSender<String>,
    shutdown: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    pending: Pending,
    next_id: Mutex<u64>,
    events: Mutex<Option<mpsc::Receiver<Value>>>,
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Ok(mut shutdown) = self.shutdown.lock() {
            if let Some(tx) = shutdown.take() { let _ = tx.send(()); }
        }
    }
}

impl Client {
    pub fn connect(path: &Path) -> Result<Self, ClientError> {
        let path = path.to_path_buf();
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let (ev_tx, ev_rx) = mpsc::channel::<Value>();
        let (writer, mut writes) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (shutdown, stopped) = tokio::sync::oneshot::channel();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(),String>>();
        let p2 = pending.clone();
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(rt)=>rt, Err(e)=>{let _=ready_tx.send(Err(e.to_string())); return;}
            };
            rt.block_on(async move {
                use tokio::io::{AsyncBufReadExt,AsyncWriteExt,BufReader};
                let stream = match crate::transport::connect(&path).await {
                    Ok(s)=>s, Err(e)=>{let _=ready_tx.send(Err(format!("{}: {e}",path.display()))); return;}
                };
                let (reader,mut wr)=tokio::io::split(stream);
                let mut lines=BufReader::new(reader).lines();
                let _=ready_tx.send(Ok(()));
                let read=async {
                    while let Ok(Some(line))=lines.next_line().await {
                        let Ok(v)=serde_json::from_str::<Value>(&line) else {continue};
                        if v.get("id").is_some() {
                            if let Ok(resp)=serde_json::from_value::<Response>(v) {
                                if let Some(tx)=p2.lock().unwrap().remove(&resp.id) {let _=tx.send(resp);}
                            }
                        } else {let _=ev_tx.send(v);}
                    }
                };
                let write=async {
                    while let Some(line)=writes.recv().await {
                        if wr.write_all(line.as_bytes()).await.is_err() || wr.write_all(b"\n").await.is_err() {break;}
                    }
                };
                tokio::select! {_=read=>{},_=write=>{},_=stopped=>{}}
                p2.lock().unwrap().clear();
            });
        });
        ready_rx.recv_timeout(Duration::from_secs(5)).map_err(|e|ClientError::NotRunning(e.to_string()))?.map_err(ClientError::NotRunning)?;
        Ok(Self {writer,shutdown:Mutex::new(Some(shutdown)),pending,next_id:Mutex::new(1),events:Mutex::new(Some(ev_rx))})
    }

    /// Take the event receiver (once).
    pub fn take_events(&self) -> Option<mpsc::Receiver<Value>> {
        self.events.lock().unwrap().take()
    }

    pub fn call(&self, method: &str, params: Value) -> Result<Value, ClientError> {
        let id = {
            let mut n = self.next_id.lock().unwrap();
            let id = *n;
            *n += 1;
            id
        };
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let line = serde_json::to_string(&Request { id, method: method.into(), params }).unwrap();
        if self.writer.send(line).is_err() {
            self.pending.lock().unwrap().remove(&id);
            return Err(ClientError::Disconnected);
        }
        let resp = rx.recv().map_err(|_| ClientError::Disconnected)?;
        match (resp.result, resp.error) {
            (_, Some(e)) => Err(e.into()),
            (Some(v), None) => Ok(v),
            (None, None) => Ok(Value::Null),
        }
    }

    pub fn hello(&self, kind: &str, name: &str) -> Result<Value, ClientError> {
        self.call("hello", json!({ "kind": kind, "agentId": name, "name": name }))
    }

    pub fn hello_frontend(&self, name: &str, stream_output: bool) -> Result<Value, ClientError> {
        self.call("hello", json!({ "kind": "frontend", "name": name, "streamOutput": stream_output }))
    }
}
