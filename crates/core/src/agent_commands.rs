//! Agent session operations. Owner-only operations never enter this service.
use crate::{
    affordance::Actor,
    authority::ConnId,
    protocol::RpcError,
    session::{ConnKind, Session, SessionError, SharedSession},
};
use serde_json::{json, Value};

/// Agents get one generic refusal so they cannot probe for private sessions. The
/// owner can ask why: `CONN_TRACE_REFUSALS=1` prints the cause to stderr. Reasons
/// and identifiers only, never terminal content.
pub(crate) fn trace_refusal(reason: &str, conn: ConnId, session: &str) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ENABLED
        .get_or_init(|| std::env::var_os("CONN_TRACE_REFUSALS").is_some_and(|v| !v.is_empty()))
    {
        eprintln!(
            "conn refusal {} conn={conn} session={} reason={reason}",
            chrono::Local::now().format("%H:%M:%S%.3f"),
            trace_label(session)
        );
    }
}

/// The named session or tab comes from the agent. Keep it to one bounded, escaped
/// token so a request cannot forge trace lines or drive the owner's terminal.
pub(crate) fn trace_label(requested: &str) -> String {
    let mut label: String = requested
        .chars()
        .take(96)
        .flat_map(char::escape_default)
        .collect();
    if requested.chars().nth(96).is_some() {
        label.push('…');
    }
    label
}

pub(crate) fn session_unavailable() -> RpcError {
    RpcError {
        code: "not_found".into(),
        message: "session unavailable".into(),
    }
}

pub(crate) fn str_param(params: &Value, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Synchronous dispatch. Runs under the session lock; must not block.
pub fn dispatch(
    session: &SharedSession,
    conn: ConnId,
    method: &str,
    params: &Value,
) -> Result<Value, RpcError> {
    dispatch_locked(&mut session.lock(), conn, method, params)
}

pub(crate) fn dispatch_locked(
    s: &mut Session,
    conn: ConnId,
    method: &str,
    params: &Value,
) -> Result<Value, RpcError> {
    if s.is_private() {
        trace_refusal("session is private", conn, "-");
        return Err(session_unavailable());
    }
    if !s.participant_allowed(conn) {
        trace_refusal("connection is not a selected participant", conn, "-");
        return Err(session_unavailable());
    }
    if s.conn_kind(conn) != Some(ConnKind::Agent) {
        return Err(RpcError {
            code: "owner_required".into(),
            message: "public connections must identify as agents".into(),
        });
    }
    if !matches!(
        method,
        "affordances"
            | "snapshot"
            | "request_control"
            | "release_control"
            | "type"
            | "send_key"
            | "interrupt"
            | "request_attention"
            | "check_approval"
            | "control_request_state"
            | "proposal_state"
            | "exec_state"
            | "status"
    ) {
        return Err(RpcError {
            code: "owner_required".into(),
            message: "this operation belongs to the local owner UI".into(),
        });
    }
    if method == "status" {
        return Ok(
            json!({"controller": s.status().controller, "mode": s.mode(), "effectiveMode": s.effective_mode(), "attended": s.attended(), "shared": !s.is_private(), "surfaceAvailable": s.status().surface_available }),
        );
    }
    s.note_connection_activity(conn);
    let actor = Actor::Agent { conn };
    if matches!(
        method,
        "check_approval" | "control_request_state" | "proposal_state" | "exec_state"
    ) {
        let key = match method {
            "check_approval" => "approvalId",
            "control_request_state" => "requestId",
            "proposal_state" => "proposalId",
            _ => "execId",
        };
        if !s.owns_request(conn, method, params[key].as_str().unwrap_or_default()) {
            return Err(session_unavailable());
        }
    }
    let r: Result<Value, SessionError> = match method {
        "affordances" => Ok(json!(s
            .affordances(actor)
            .iter()
            .map(|a| a.name())
            .collect::<Vec<_>>())),
        "snapshot" => s.snapshot(actor).map(|v| serde_json::to_value(v).unwrap()),
        "request_control" => s
            .agent_request_control_original(conn, params.clone())
            .map(|o| serde_json::to_value(o).unwrap()),
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
        "release_control" => s
            .agent_release_control(conn)
            .map(|_| json!({ "released": true })),
        "type" => {
            let text = str_param(params, "text").unwrap_or_default();
            s.agent_type(conn, &text)
                .map(|_| json!({ "typed": text.chars().count() }))
        }
        "send_key" => {
            let key = str_param(params, "key").unwrap_or_default();
            let intent = str_param(params, "intent");
            s.agent_send_key_with(conn, &key, intent)
                .map(|r| serde_json::to_value(r).unwrap())
        }
        "interrupt" => s
            .agent_interrupt(conn)
            .map(|_| json!({ "interrupted": true })),
        "check_approval" => {
            let id = str_param(params, "approvalId").unwrap_or_default();
            s.check_approval(&id)
                .map(|a| serde_json::to_value(a).unwrap())
        }
        "exec_state" => {
            let id = str_param(params, "execId").unwrap_or_default();
            match s.exec_state(&id) {
                Some((state, reason)) => {
                    Ok(json!({ "execId": id, "state": state, "reason": reason }))
                }
                None => Err(SessionError::NotFound(id)),
            }
        }
        "request_attention" => s
            .agent_request_attention(conn, str_param(params, "reason"))
            .map(|_| json!({ "requested": true })),
        other => Err(SessionError::InvalidInput(format!(
            "unknown method '{other}'"
        ))),
    };
    r.map_err(RpcError::from)
}
