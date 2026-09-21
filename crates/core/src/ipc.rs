//! Local socket/named-pipe control plane. Newline-delimited JSON.
//!
//! Request:  `{"id":1,"method":"snapshot","params":{}}`
//! Response: `{"id":1,"result":{...}}` or `{"id":1,"error":{"code":"...","message":"..."}}`
//! Event:    `{"event":"control_revoked", ...}` (no id; pushed by the server)
//!
//! Public connections identify as agents. Owner UI operations and raw output
//! are in-process native capabilities; a socket client cannot claim either role.
//! Responses use protocol version 2 and session terminal grids only.
//!
//! See docs/protocol.md for the full method list.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::affordance::Affordance;
use crate::authority::ConnId;
use crate::session::{SessionError, ConnKind, ControlRequestState, EventSink, ExecState, KeyResult, ProposalState, ServerEvent, SharedSession};

pub use crate::protocol::{Request, Response, RpcError};
pub use crate::agent_commands::dispatch;
use crate::agent_commands::{trace_refusal, session_unavailable, str_param};
#[cfg(test)] use crate::agent_commands::trace_label;
fn connection_closing() -> RpcError {
    RpcError { code: "connection_closing".into(), message: "the connection closed before this request could run".into() }
}

// ---------------------------------------------------------------------------
// Hub: several sessions (tabs) behind one socket
// ---------------------------------------------------------------------------

pub use crate::collaboration::{Hub, SharedHub, SessionId, TabOpener};
pub use crate::connections::{Admission, AdmissionPolicy, AdmissionChange, AdmissionListener};
const ADMISSION_WAIT: Duration = Duration::from_secs(25);
fn admission_error(state: Admission) -> RpcError {
    if state == Admission::Denied { RpcError { code: "admission_denied".into(), message: "the human declined this connection".into() } }
    else { RpcError { code: "admission_pending".into(), message: "waiting for the human to allow this connection in the Conn window".into() } }
}

// ---------------------------------------------------------------------------
// Server (tokio)
// ---------------------------------------------------------------------------

/// Events leave a session tagged with its id.
struct TokioSink {
    session: SessionId,
    tx: tokio::sync::mpsc::UnboundedSender<(SessionId, u64, ServerEvent)>,
}

impl EventSink for TokioSink {
    fn send(&self, event: ServerEvent) {
        self.send_scoped(event, 0);
    }
    fn send_scoped(&self, event: ServerEvent, generation: u64) {
        let _ = self.tx.send((self.session.clone(), generation, event));
    }
}

/// Serve the sessions behind `listener` until the future is dropped.
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

/// Serve the sessions on `path` from a dedicated runtime. For embedders that do
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
struct ConnIdentity { name: String }

async fn handle_conn(stream: crate::transport::Stream, conn: ConnId, hub: SharedHub) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let (rd, mut wr) = tokio::io::split(stream);
    // Read transport liveness independently of approval/proposal/grace waits.
    // A bounded queue rejects flooding rather than hiding EOF behind queued work.
    let (input_tx, mut input_rx) = tokio::sync::mpsc::channel(64);
    let (closed_tx, mut closed_rx) = tokio::sync::watch::channel(false);
    let reader_closed = closed_tx.clone();
    let reader = tokio::spawn(async move {
        let mut lines = BufReader::new(rd).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if input_tx.try_send(line).is_err() { break; }
        }
        let _ = reader_closed.send(true);
    });
    struct Outbound { line: String, guard: Option<(SessionId, u64, Option<u64>)> }
    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<Outbound>();
    let writer_hub = hub.clone();
    let writer = tokio::spawn(async move {
        while let Some(mut message) = out_rx.recv().await {
            if let Some((sid, generation, frame_generation)) = &message.guard {
                let refused = match writer_hub.get(sid) {
                    None => Some("session closed before the reply was sent"),
                    Some(s) => {
                        let s = s.lock();
                        if !s.participant_allowed(conn) { Some("participation ended before the reply was sent") }
                        else if s.participation_generation() != *generation { Some("sharing changed before the reply was sent") }
                        else if frame_generation.is_some_and(|g| s.surface_generation() != g) { Some("snapshot belongs to an earlier sharing boundary") }
                        else { None }
                    }
                };
                if let Some(reason) = refused {
                    trace_refusal(reason, conn, sid);
                    let id = serde_json::from_str::<Value>(&message.line).ok().and_then(|v| v["id"].as_u64());
                    let Some(id) = id else { continue; };
                    message.line = serde_json::to_string(&Response { id, result: None, error: Some(session_unavailable()) }).unwrap();
                }
            }
            if wr.write_all(message.line.as_bytes()).await.is_err() || wr.write_all(b"\n").await.is_err() {
                let _ = closed_tx.send(true);
                break;
            }
        }
    });
    let (ev_tx, mut ev_rx) = tokio::sync::mpsc::unbounded_channel::<(SessionId, u64, ServerEvent)>();
    let (catalog_tx, mut catalog_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let out_ev = out_tx.clone();
    let forwarder = tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(()) = catalog_rx.recv() => {
                    // Connection-level invalidation carries no session identity or
                    // content and must also reach agents whose access was removed.
                    let _ = out_ev.send(Outbound { line: json!({"event":"tools_changed"}).to_string(), guard: None });
                }
                Some((sid, generation, ev)) = ev_rx.recv() => {
                    if let Ok(mut v) = serde_json::to_value(&ev) {
                        if let Some(o) = v.as_object_mut() {
                            o.insert("session".into(), Value::String(sid.clone()));
                        }
                        let _ = out_ev.send(Outbound { line: v.to_string(), guard: Some((sid, generation, None)) });
                    }
                }
                else => break,
            }
        }
    });

    let mut identity = ConnIdentity { name: "client".into() };
    let mut registered: Vec<SessionId> = Vec::new();
    // A connection is bound to the session it first lands in (the attended one at
    // hello time). It does not drift when the human changes tabs — an agent whose
    // human changed tabs must remain bound to its original session. Requests may
    // still name a `session` explicitly.
    let mut bound: Option<SessionId> = None;
    let mut introduced = false;
    let mut admission_rx: Option<tokio::sync::watch::Receiver<Admission>> = None;
    let register = |sid: &SessionId, s: &SharedSession, id: &ConnIdentity, registered: &mut Vec<SessionId>| {
        if registered.iter().any(|r| r == sid) {
            return;
        }
        let sink = Box::new(TokioSink { session: sid.clone(), tx: ev_tx.clone() });
        let mut g = s.lock();
        g.register_conn(conn, ConnKind::Agent, &id.name, sink);
        registered.push(sid.clone());
    };

    // After transport loss, requests queued before EOF are drained: a one-shot
    // client may pipeline and half-close. Only reads run then; nothing that
    // writes, waits on the human or moves the binding outlives its sender.
    let mut draining = false;
    loop {
        let line = if draining {
            match input_rx.try_recv() { Ok(line) => line, Err(_) => break }
        } else {
            tokio::select! {
                biased;
                _ = closed_rx.changed() => { draining = true; continue; }
                line = input_rx.recv() => match line { Some(line) => line, None => break },
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let _ = out_tx.send(Outbound {
                    line: serde_json::to_string(&Response { id: 0, result: None, error: Some(RpcError { code: "parse".into(), message: e.to_string() }) }).unwrap(), guard: None,
                });
                continue;
            }
        };
        for sid in &registered {
            if let Some(s) = hub.get_public(sid) { s.lock().note_connection_activity(conn); }
        }
        hub.connections.touch(conn);
        let mut disclosure = str_param(&req.params, "session").or_else(|| bound.clone()).or_else(|| hub.public_attended_for(conn))
            .and_then(|id| hub.get(&id).map(|s| { let generation = s.lock().participation_generation(); (id, generation, None::<u64>) }));
        let read_only = matches!(req.method.as_str(), "hello" | "sessions" | "list_tabs" | "affordances" | "navigation_affordances"
            | "snapshot" | "status" | "check_approval" | "control_request_state" | "proposal_state" | "exec_state");
        if draining && !read_only {
            let _ = out_tx.send(Outbound { line: serde_json::to_string(&Response { id: req.id, result: None, error: Some(connection_closing()) }).unwrap(), guard: None });
            continue;
        }
        // Until the owner admits this connection it learns nothing. Discovery
        // answers at once so clients can start; real work waits briefly for the
        // decision, which makes the first call succeed right after "Allow".
        if let Some(rx) = admission_rx.as_mut().filter(|_| req.method != "hello") {
            let mut state = *rx.borrow();
            let discovery = matches!(req.method.as_str(), "sessions" | "list_tabs" | "affordances" | "navigation_affordances" | "status");
            if state == Admission::Pending && !discovery && !draining {
                state = tokio::select! {
                    biased;
                    _ = closed_rx.changed() => { draining = true; Admission::Pending }
                    state = async { let _ = tokio::time::timeout(ADMISSION_WAIT, rx.wait_for(|a| *a != Admission::Pending)).await; *rx.borrow() } => state,
                };
            }
            if state != Admission::Granted {
                let _ = out_tx.send(Outbound { line: serde_json::to_string(&Response { id: req.id, result: None, error: Some(admission_error(state)) }).unwrap(), guard: None });
                continue;
            }
        }
        let result = tokio::select! {
            biased;
            // Reads are short and side-effect free; only cancellable work races EOF.
            _ = closed_rx.changed(), if !read_only && !draining => { draining = true; Err(connection_closing()) }
            result = async { if !introduced && req.method != "hello" {
            Err(RpcError { code: "hello_required".into(), message: "identify as an agent with hello first".into() })
        } else if req.method == "hello" && introduced {
            Err(RpcError { code: "invalid_input".into(), message: "identity is immutable for this connection".into() })
        } else if req.method == "hello" && str_param(&req.params, "kind").as_deref() != Some("agent") {
            Err(RpcError { code: "owner_required".into(), message: "public transport accepts agents only; use the native owner UI".into() })
        } else if req.method == "hello" && str_param(&req.params, "session").is_some_and(|id| hub.get_public(&id).is_none()) {
            Err(session_unavailable())
        } else if req.method == "hello" {
            let name = str_param(&req.params, "agentId").or_else(|| str_param(&req.params, "name")).unwrap_or_else(|| "client".into());
            let admitted = hub.admit(conn, &name, catalog_tx.clone());
            let admission = *admitted.borrow();
            admission_rx = Some(admitted);
            introduced = true;
            identity = ConnIdentity { name };
            if admission != Admission::Granted {
                // Nothing about any session is disclosed or bound before the owner answers.
                disclosure = None;
                Ok(json!({ "protocolVersion": 2, "conn": conn, "kind": "agent", "admission": admission, "attended": null, "session": null, "mode": null, "effectiveMode": null }))
            } else {
            if let Some((sid, s)) = hub.resolve_public(str_param(&req.params, "session").as_deref()).filter(|(_, s)| s.lock().participant_allowed(conn)) {
                register(&sid, &s, &identity, &mut registered);
                bound = Some(sid.clone());
            }
            let target = bound.clone().or_else(|| hub.public_attended_for(conn));
            let modes = target.as_deref().and_then(|id| hub.get_public(id)).map(|s| { let s = s.lock(); (s.mode(), s.effective_mode()) });
            Ok(json!({ "protocolVersion": 2, "conn": conn, "kind": "agent", "admission": admission, "attended": hub.public_attended_for(conn), "session": target, "mode": modes.map(|m| m.0), "effectiveMode": modes.map(|m| m.1) }))
            }
        } else if req.method == "sessions" || req.method == "list_tabs" {
            let list: Vec<Value> = hub.public_ids().into_iter().enumerate().filter_map(|(i, sid)| {
                let s = hub.get_public(&sid)?;
                let g = s.lock();
                if !g.participant_allowed(conn) { return None; }
                let st = g.status();
                Some(json!({ "tab": i + 1, "id": sid, "current": bound.as_deref() == Some(sid.as_str()), "attended": st.attended, "controller": st.controller, "pending": st.pending.len(), "processAlive": st.process_alive, "mode": st.mode, "effectiveMode": st.effective_mode }))
            }).enumerate().map(|(i, mut row)| { row["tab"] = json!(i + 1); row }).collect();
            Ok(json!({ "sessions": list, "tabs": list.len(), "attended": hub.public_attended_for(conn), "current": bound }))
        } else if req.method == "open_tab" {
            // Agent: open a tab. It starts unattended; the human sees it knock.
            let reason = str_param(&req.params, "reason");
            let cur = bound.clone().and_then(|b| hub.get_public(&b).map(|s| (b, s))).or_else(|| hub.resolve_public(None));
            match cur {
                _ if !hub.tabs_supported() => Err(RpcError { code: "unsupported".into(), message: "this host cannot open tabs".into() }),
                Some((from, s)) => {
                    let allowed = { register(&from, &s, &identity, &mut registered); s.lock().agent_can(conn, Affordance::OpenTab) };
                    match allowed {
                        Err(e) => Err(e.into()),
                        Ok(()) => match hub.open_tab(&identity.name, reason.as_deref()) {
                            Err(e) => Err(e),
                            Ok(sid) => {
                                let ns = hub.get_public(&sid).expect("checked");
                                register(&sid, &ns, &identity, &mut registered);
                                hub.leave_other_tabs(conn, &sid);
                                ns.lock().agent_opened_tab(conn, reason);
                                bound = Some(sid.clone());
                                Ok(json!({ "session": sid, "tab": hub.public_index_of(&sid, conn), "attended": false, "note": "the human is not looking at this tab yet; request_attention is queued — wait for the human to show this tab" }))
                            }
                        },
                    }
                }
                None => Err(RpcError { code: "not_found".into(), message: "no session".into() }),
            }
        } else if req.method == "navigation_affordances" {
            // Navigation is connection-scoped; a vanished source must not hide
            // recovery. Only destinations explicitly available to this agent count.
            disclosure = None;
            let pinned = bound.as_ref().and_then(|b| hub.get_public(b)).is_some_and(|s| s.lock().blocks_navigation_from(conn));
            let can_switch = !pinned && hub.public_ids().iter().any(|id| hub.get_public(id)
                .is_some_and(|s| s.lock().agent_can_navigate(conn).is_ok()));
            Ok(json!(if can_switch { vec!["switch_tab"] } else { vec![] }))
        } else if req.method == "switch_tab" {
            let want = req.params.get("tab").cloned().or_else(|| req.params.get("session").cloned()).unwrap_or(Value::Null);
            match hub.find_public_tab(&want, conn) {
                None => { trace_refusal("no such shared tab for this connection (private, not selected, or closed)", conn, &want.as_str().map(str::to_owned).unwrap_or_else(|| want.to_string())); Err(session_unavailable()) }
                Some((to, ts)) => {
                    let pinned = bound.as_ref().filter(|from| **from != to).and_then(|b| hub.get_public(b)).is_some_and(|s| s.lock().blocks_navigation_from(conn));
                    let allowed = if pinned { Err(SessionError::Masked("switch_tab".into())) } else { ts.lock().agent_can_navigate(conn) };
                    match allowed {
                        Err(e) => Err(e.into()),
                        Ok(()) => {
                            register(&to, &ts, &identity, &mut registered);
                            hub.leave_other_tabs(conn, &to);
                            if let Some(from) = bound.as_ref().filter(|from| **from != to) {
                                if let Some(source) = hub.get_public(from) {
                                    let mut source = source.lock();
                                    if source.participant_allowed(conn) {
                                        source.agent_switched_tab(conn, from, &to);
                                        drop(source);
                                        ts.lock().agent_switched_tab(conn, from, &to);
                                    }
                                }
                                // Do not carry an inaccessible source identity into
                                // the destination's session-scoped event stream.
                            }
                            bound = Some(to.clone());
                            let (attended, generation) = {
                                let target = ts.lock();
                                (target.status().attended, target.participation_generation())
                            };
                            disclosure = Some((to.clone(), generation, None));
                            Ok(json!({ "session": to, "tab": hub.public_index_of(&to, conn), "attended": attended }))
                        }
                    }
                }
            }
        } else if req.method == "set_attended" {
            Err(RpcError { code: "owner_required".into(), message: "only the native owner can select the presented tab".into() })
        } else {
            let target = str_param(&req.params, "session").or_else(|| bound.clone());
            match hub.resolve_public(target.as_deref()) {
                Some((sid, s)) => {
                    register(&sid, &s, &identity, &mut registered);
                    if bound.is_none() {
                        bound = Some(sid.clone());
                    }
                    call_with_pacing(&hub, &sid, &s, conn, &req.method, &req.params).await
                }
                None => { trace_refusal("no such shared session (closed, private, or never bound)", conn, target.as_deref().unwrap_or("-")); Err(session_unavailable()) }
            }
        } } => result,
        };
        let resp = match result {
            Ok(v) => Response { id: req.id, result: Some(v), error: None },
            Err(e) => Response { id: req.id, result: None, error: Some(e) },
        };
        let guard = if resp.error.is_none() && !matches!(req.method.as_str(), "hello" | "sessions" | "list_tabs") {
            disclosure.map(|(sid, epoch, _)| (sid, epoch, if req.method == "snapshot" { resp.result.as_ref().and_then(|r| r["generation"].as_u64()) } else { None }))
        } else { None };
        if out_tx.send(Outbound { line: serde_json::to_string(&resp).unwrap(), guard }).is_err() {
            break;
        }
    }
    hub.forget_agent(conn);
    for sid in registered {
        if let Some(s) = hub.get(&sid) {
            s.lock().connection_closed(conn);
        }
    }
    reader.abort();
    forwarder.abort();
    let _ = reader.await;
    let _ = forwarder.await;
    // Every sender is gone now; let queued responses reach a half-closed peer,
    // but never wait on one that stopped reading.
    drop(out_tx);
    let mut writer = writer;
    if tokio::time::timeout(Duration::from_secs(1), &mut writer).await.is_err() {
        writer.abort();
        let _ = writer.await;
    }
}

/// Dispatch, absorbing pacing on behalf of the agent: rate limits become waits, and a
/// scheduled ENTER is awaited until it executes or is cancelled.
async fn call_with_pacing(hub: &Hub, id: &str, session: &SharedSession, conn: ConnId, method: &str, params: &Value) -> Result<Value, RpcError> {
    if method == "send_key" && str_param(params, "key").map(|k| k.eq_ignore_ascii_case("ENTER")).unwrap_or(false) {
        // Let the shell echo settle so the VT cursor line reflects the typed text.
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
    let initial_generation = session.lock().participation_generation();
    let dispatch_request = || {
        if method == "request_control" { hub.request_control(id, conn, params) }
        else { dispatch(session, conn, method, params) }
    };
    let mut result = dispatch_request();
    for _ in 0..200 {
        match &result {
            Err(e) if e.code == "rate_limited" => {
                let ms = e.message.split_whitespace().rev().nth(1).and_then(|s| s.parse::<u64>().ok()).unwrap_or(50);
                tokio::time::sleep(Duration::from_millis(ms.clamp(5, 5000))).await;
                result = dispatch_request();
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
                if session.lock().participation_generation() != initial_generation { trace_refusal("sharing changed while the request was waiting", conn, "-"); return Err(session_unavailable()); }
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
                if session.lock().participation_generation() != initial_generation { trace_refusal("sharing changed while the request was waiting", conn, "-"); return Err(session_unavailable()); }
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
                if session.lock().participation_generation() != initial_generation { trace_refusal("sharing changed while the request was waiting", conn, "-"); return Err(session_unavailable()); }
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
    #[error("Conn app is not running ({0})")]
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


}

#[cfg(test)]
mod tests {
    use super::trace_label;

    #[test]
    fn trace_labels_cannot_forge_lines_or_drive_a_terminal() {
        assert_eq!(trace_label("t1-64099c19"), "t1-64099c19");
        let hostile = trace_label("x\nconn refusal 00:00:00 conn=9 reason=forged\x1b[2J\r");
        assert!(!hostile.contains('\n') && !hostile.contains('\r') && !hostile.contains('\x1b'), "{hostile}");
        assert!(hostile.starts_with("x\\nconn refusal"));
        let long = trace_label(&"a".repeat(5000));
        assert_eq!(long.chars().count(), 97);
        assert!(long.ends_with('…'));
    }
}
