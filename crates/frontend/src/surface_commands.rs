//! Owner-window surface publication and participation transitions.
use crate::{arg, engine, AppState};
use conn_core::screen::SurfaceFrame;
use serde_json::{json, Value};

pub(crate) fn invalidate(state: &AppState, id: &str) -> Option<u64> {
    let engine = engine(state, id).ok()?;
    let session = engine.session();
    let mut session = session.lock();
    state.extensions.cancel(id);
    Some(session.invalidate_surface())
}

pub(crate) fn dispatch(state: &AppState, name: &str, args: &Value) -> Option<Result<Value, String>> {
    if !matches!(name, "publish_surface" | "invalidate_surface" | "set_sharing" | "sharing_participants") { return None; }
    Some((|| {
        let id: String = arg(args, "session")?;
        if name == "sharing_participants" {
            let e = engine(state, &id)?;
            let session = e.session();
            let candidates = state.hub.agent_connections();
            let s = session.lock();
            return Ok(json!(candidates.into_iter().map(|a| json!({"connId":a.conn_id,"agentId":a.agent_id,"idleSecs":a.idle_secs,"selected":s.participant_allowed(a.conn_id)})).collect::<Vec<_>>()));
        }
        if name == "invalidate_surface" {
            return Ok(json!({"generation":invalidate(state, &id).ok_or("Session unavailable")?}));
        }
        let e = engine(state, &id)?;
        if name == "publish_surface" {
            let frame: SurfaceFrame = arg(args, "frame")?;
            let session = e.session();
            let mut s = session.lock();
            let changed = !frame.visible || s.authoritative_surface().map_or(true, |old| old.surface_id != frame.surface_id || old.generation != frame.generation || old.revision != frame.revision || old.output_seq != frame.output_seq);
            if changed { state.extensions.cancel(&id); }
            s.publish_surface(frame).map_err(|e| e.to_string())?;
            return Ok(json!({"generation":s.surface_generation(),"outputSeq":s.output_seq()}));
        }
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
