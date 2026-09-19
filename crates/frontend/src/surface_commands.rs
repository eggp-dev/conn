//! Owner-authorized participation transitions.
use crate::{arg, engine, AppState};
use serde_json::{json, Value};

pub(crate) fn dispatch(state: &AppState, name: &str, args: &Value) -> Option<Result<Value, String>> {
    if !matches!(name, "set_sharing" | "sharing_participants") { return None; }
    Some((|| {
        let id: String = arg(args, "session")?;
        if name == "sharing_participants" {
            let e = engine(state, &id)?;
            let session = e.session();
            let candidates = state.hub.agent_connections();
            let s = session.lock();
            return Ok(json!(candidates.into_iter().map(|a| json!({"connId":a.conn_id,"agentId":a.agent_id,"idleSecs":a.idle_secs,"selected":s.participant_allowed(a.conn_id)})).collect::<Vec<_>>()));
        }
        let e = engine(state, &id)?;
        let shared: bool = arg(args, "shared")?;
        let selected: Vec<u64> = if shared { arg(args, "connectionIds")? } else { Vec::new() };
        let candidates = state.hub.agent_connections();
        if selected.iter().any(|id| !candidates.iter().any(|a| a.conn_id == *id)) {
            return Err("A selected agent disconnected; choose participants again".into());
        }
        // Stop the owner-bound writer and drain its queue BEFORE acquiring Session.
        // Its worker takes operation -> Session locks in the same order.
        state.automation.stop_session(&id);
        let session = e.session();
        let mut s = session.lock();
        state.extensions.cancel(&id);
        if shared && s.external_origin() && s.is_private() {
            let audit = conn_core::audit::Audit::open(&state.config_dir.join("audit.jsonl"))
                .map_err(|_| "Could not open collaboration history")?.for_session(id);
            s.activate_shared_audit(audit);
        }
        s.set_shared_with_agents(shared, selected).map_err(|e| e.to_string())?;
        serde_json::to_value(s.status()).map_err(|e| e.to_string())
    })())
}
