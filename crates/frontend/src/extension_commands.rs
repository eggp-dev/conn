//! Native mediation for the extension registry: status, theme selection and theme install.
use crate::{arg, AppState};
use serde_json::Value;

pub(crate) fn dispatch(state: &AppState, name: &str, args: &Value) -> Option<Result<Value, String>> {
    Some(match name {
        "extensions_status" => Ok(state.extensions.snapshot()),
        "extension_configure" => arg::<String>(args, "id").and_then(|id| state.extensions.configure(&id, arg(args, "config")?)),
        "extension_install_theme" => arg(args, "manifest").and_then(|m| state.extensions.install_theme(m)).map(|_| state.extensions.snapshot()),
        _ => return None,
    })
}
