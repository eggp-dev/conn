//! Owner-authorized participation transitions.
use crate::{arg, engine, AppState};
use serde_json::{json, Value};

pub(crate) fn dispatch(
    state: &AppState,
    name: &str,
    args: &Value,
) -> Option<Result<Value, String>> {
    if !matches!(
        name,
        "set_sharing" | "sharing_participants" | "sharing_state"
    ) {
        return None;
    }
    Some((|| {
        let id: String = arg(args, "session")?;
        if matches!(name, "sharing_participants" | "sharing_state") {
            let e = engine(state, &id)?;
            let session = e.session();
            let candidates = state.hub.agent_connections();
            let s = session.lock();
            let participants = candidates.into_iter().map(|a| json!({"connId":a.conn_id,"agentId":a.agent_id,"idleSecs":a.idle_secs,"selected":s.participant_allowed(a.conn_id)})).collect::<Vec<_>>();
            return Ok(if name == "sharing_state" {
                json!({"revision":s.participation_generation(),"participants":participants})
            } else {
                json!(participants)
            });
        }
        let e = engine(state, &id)?;
        let shared: bool = arg(args, "shared")?;
        let selected: Vec<u64> = if shared {
            arg(args, "connectionIds")?
        } else {
            Vec::new()
        };
        let expected: Option<u64> = if args
            .get("expectedRevision")
            .is_some_and(|value| !value.is_null())
        {
            Some(arg(args, "expectedRevision")?)
        } else {
            None
        };
        let candidates = state.hub.agent_connections();
        if selected
            .iter()
            .any(|id| !candidates.iter().any(|a| a.conn_id == *id))
        {
            return Err("connection_gone".into());
        }
        // Prepare fallible resources before changing authority. A disabled audit
        // can be prepared here without recording private input or starting replay.
        let session = e.session();
        let needs_audit = {
            let s = session.lock();
            shared && s.external_origin() && s.is_private()
        };
        let audit = if needs_audit {
            Some(
                conn_core::audit::Audit::open(&state.config_dir.join("audit.jsonl"))
                    .map_err(|_| "history_unavailable")?
                    .for_session(id.clone()),
            )
        } else {
            None
        };
        let status = state.automation.transition_session(&id, || {
            let mut s = session.lock();
            if expected.is_some_and(|revision| revision != s.participation_generation()) {
                return Err("sharing_changed".into());
            }
            let live = state.hub.agent_connections();
            if selected
                .iter()
                .any(|id| !live.iter().any(|a| a.conn_id == *id))
            {
                return Err("connection_gone".into());
            }
            s.validate_sharing(shared, &selected)
                .map_err(|e| conn_core::ipc::RpcError::from(e).code)?;
            if let Some(audit) = audit {
                s.activate_shared_audit(audit);
            }
            s.set_shared_with_agents(shared, selected)
                .map_err(|e| conn_core::ipc::RpcError::from(e).code)?;
            Ok(json!(s.status()))
        })?;
        // Resolve fulfilled preparation requests after releasing the session
        // lock, so a newly shared shell does not leave a second stale card.
        state.hub.activity_snapshot();
        Ok(status)
    })())
}
