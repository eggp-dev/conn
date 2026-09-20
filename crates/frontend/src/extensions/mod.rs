//! Small extension host. API 1 registers declarative terminal themes only; executable
//! kinds stay in the manifest contract but no built-in implements them. The host never
//! writes to a PTY and has no network, credential or command execution API.
mod manifest;

use manifest::Registry;
pub use manifest::{Kind, Manifest, API_VERSION, DEFAULT_THEME};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

/// Not `deny_unknown_fields`: files written before API 1 dropped its provider still load.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: DEFAULT_THEME.into(),
        }
    }
}
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Stored {
    #[serde(default)]
    settings: Settings,
    #[serde(default)]
    themes: Vec<Manifest>,
}
struct State {
    settings: Settings,
    themes: Vec<Manifest>,
    registry: Registry,
}
pub struct Extensions {
    state: Mutex<State>,
    path: PathBuf,
}

fn persist(path: &Path, state: &State) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(&Stored {
        settings: state.settings.clone(),
        themes: state.themes.clone(),
    })
    .map_err(|_| "Could not encode extension settings")?;
    let parent = path.parent().ok_or("Invalid extension settings path")?;
    std::fs::create_dir_all(parent).map_err(|_| "Could not create extension settings directory")?;
    let mut file =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| "Could not save extension settings")?;
    file.write_all(&bytes)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|_| "Could not save extension settings")?;
    file.persist(path)
        .map_err(|_| "Could not save extension settings")?;
    Ok(())
}
impl Extensions {
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join("extensions.json");
        let stored = std::fs::metadata(&path)
            .ok()
            .filter(|m| m.len() <= 64 * 1024)
            .and_then(|_| std::fs::read(&path).ok())
            .and_then(|v| serde_json::from_slice::<Stored>(&v).ok())
            .unwrap_or_default();
        let mut registry = Registry::builtin();
        let mut themes = Vec::new();
        for manifest in stored.themes.into_iter().take(32) {
            if registry.register(manifest.clone(), false).is_ok() {
                themes.push(manifest);
            }
        }
        let settings = if registry
            .get(&stored.settings.theme)
            .is_some_and(|m| m.kind == Kind::Theme)
        {
            stored.settings
        } else {
            Settings::default()
        };
        Self {
            state: Mutex::new(State {
                settings,
                themes,
                registry,
            }),
            path,
        }
    }
    pub fn snapshot(&self) -> Value {
        let state = self.state.lock();
        json!({
            "apiVersion": API_VERSION,
            "settings": state.settings,
            "extensions": state.registry.all().map(|manifest| json!({
                "id": manifest.id, "name": manifest.name, "kind": manifest.kind, "capabilities": manifest.capabilities,
                "enabled": manifest.kind == Kind::Theme && state.settings.theme == manifest.id
            })).collect::<Vec<_>>(),
            "themes": state.registry.all().filter_map(|m| m.theme.clone()).collect::<Vec<_>>()
        })
    }
    #[cfg(test)]
    pub fn settings(&self) -> Settings {
        self.state.lock().settings.clone()
    }
    pub fn configure(&self, id: &str, config: Value) -> Result<Value, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Switch {
            enabled: bool,
        }
        let mut state = self.state.lock();
        if state.registry.get(id).ok_or("Unknown extension")?.kind != Kind::Theme {
            return Err("Only reviewed built-in executable extensions are supported".into());
        }
        let c: Switch = serde_json::from_value(config).map_err(|_| "Invalid theme settings")?;
        let previous = state.settings.clone();
        state.settings.theme = if c.enabled {
            id.into()
        } else {
            DEFAULT_THEME.into()
        };
        if let Err(error) = persist(&self.path, &state) {
            state.settings = previous;
            return Err(error);
        }
        Ok(json!({"settings":state.settings}))
    }
    pub fn install_theme(&self, manifest: Manifest) -> Result<(), String> {
        let mut state = self.state.lock();
        if state.themes.len() >= 32 {
            return Err("Theme limit reached".into());
        }
        manifest.validate(false)?;
        if state.registry.get(&manifest.id).is_some() {
            return Err("Extension ID is already registered".into());
        }
        state.themes.push(manifest.clone());
        if let Err(e) = persist(&self.path, &state) {
            state.themes.pop();
            return Err(e);
        }
        state.registry.register(manifest, false)
    }
}

#[cfg(test)]
mod tests;
