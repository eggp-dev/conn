//! Native mediation: extension context is built from a current session terminal grid,
//! never from model/context strings supplied by the webview or a public socket.
use crate::{arg, engine, AppState};
use crate::extensions::{FrameToken, VisibleContext};
use serde_json::{json, Value};

fn token(id: &str, frame: &conn_core::screen::SurfaceFrame) -> FrameToken {
    FrameToken { session:id.into(),surface_id:frame.surface_id.clone(),generation:frame.generation,frame_id:frame.revision }
}

pub(crate) fn dispatch(state: &AppState, name: &str, args: &Value) -> Option<Result<Value, String>> {
    if !matches!(name, "extensions_status" | "extension_configure" | "extension_install_theme" | "extension_set_key" | "extension_delete_key" | "completion_request" | "completion_status" | "completion_accept" | "completion_cancel") { return None; }
    Some((|| {
        match name {
            "extensions_status" => return Ok(state.extensions.snapshot()),
            "extension_configure" => return state.extensions.configure(&arg::<String>(args, "id")?, arg(args, "config")?),
            "extension_install_theme" => { state.extensions.install_theme(arg(args, "manifest")?)?; return Ok(state.extensions.snapshot()); }
            "extension_set_key" => { state.extensions.set_api_key(arg(args, "key")?)?; return Ok(Value::Null); }
            "extension_delete_key" => { state.extensions.delete_api_key()?; return Ok(Value::Null); }
            _ => {}
        }
        let id: String = arg(args, "session")?;
        if name == "completion_cancel" { state.extensions.cancel(&id); return Ok(Value::Null); }
        let e = engine(state, &id)?;
        let session = e.session();
        let mut s = session.lock();
        let frame = s.authoritative_surface().map_err(|e| { state.extensions.cancel(&id); e.to_string() })?;
        let current = token(&id, &frame);
        match name {
            "completion_request" => {
                let context = VisibleContext { token: current, lines: frame.screen, shared: !s.is_private(), prompt_ready: s.completion_prompt_ready(), explicit: args.get("explicit").and_then(Value::as_bool).unwrap_or(false) };
                serde_json::to_value(state.extensions.start_completion(context)?).map_err(|e|e.to_string())
            }
            "completion_status" => {
                let job = state.extensions.completion(&id, &arg::<String>(args,"id")?)?;
                if job.token != current { state.extensions.cancel(&id); return Err("Suggestion expired; request a new one".into()); }
                serde_json::to_value(job).map_err(|e|e.to_string())
            }
            "completion_accept" => {
                let text = state.extensions.take_proposal(&id, &arg::<String>(args,"id")?, &current)?;
                s.accept_completion(&current.surface_id,current.generation,current.frame_id,&text).map_err(|e|e.to_string())?;
                Ok(json!({"inserted":true}))
            }
            _ => unreachable!(),
        }
    })())
}

#[cfg(test)]
mod tests;
