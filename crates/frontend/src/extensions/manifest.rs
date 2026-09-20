//! Versioned, declarative extension contributions. No arbitrary executable loader.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const API_VERSION: u32 = 1;
pub const DEFAULT_THEME: &str = "conn.theme.midnight";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Theme,
    VisibleFrame,
    ModelRequest,
    Proposal,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Theme,
    Provider,
    Completion,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub api_version: u32,
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub capabilities: BTreeSet<Capability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
}

/// Only terminal color tokens are configurable. Control/approval UI is not themeable.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub background: String,
    pub foreground: String,
    pub cursor: String,
    pub selection: String,
    pub ansi: [String; 16],
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'-'))
}
fn valid_name(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= 80 && !name.chars().any(char::is_control)
}
fn color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|c| c.is_ascii_hexdigit())
}

impl Theme {
    pub fn validate(&self) -> Result<(), String> {
        if !valid_id(&self.id) || !valid_name(&self.name) {
            return Err("Invalid theme identity".into());
        }
        if [
            &self.background,
            &self.foreground,
            &self.cursor,
            &self.selection,
        ]
        .into_iter()
        .chain(self.ansi.iter())
        .any(|v| !color(v))
        {
            return Err("Themes accept six-digit hex colors only".into());
        }
        if self.foreground.eq_ignore_ascii_case(&self.background)
            || self.cursor.eq_ignore_ascii_case(&self.background)
        {
            return Err("Theme text and cursor must remain visible".into());
        }
        Ok(())
    }
}
impl Manifest {
    pub fn validate(&self, reviewed_builtin: bool) -> Result<(), String> {
        if self.api_version != API_VERSION {
            return Err("Unsupported extension API version".into());
        }
        if !valid_id(&self.id) || !valid_name(&self.name) {
            return Err("Invalid extension identity".into());
        }
        // No executable built-in ships today. One is admitted here by exact reviewed ID and capability set.
        let expected: BTreeSet<_> = match (&self.kind, reviewed_builtin) {
            (Kind::Theme, _) => [Capability::Theme].into_iter().collect(),
            _ => return Err("Only reviewed built-in executable extensions are supported".into()),
        };
        if self.capabilities != expected {
            return Err("Extension capabilities do not match its contribution".into());
        }
        match (&self.kind, &self.theme) {
            (Kind::Theme, Some(theme)) if theme.id == self.id => theme.validate(),
            (Kind::Theme, _) => Err("Theme contribution is required".into()),
            (_, None) => Ok(()),
            _ => Err("Unexpected theme contribution".into()),
        }
    }
}

pub struct Registry {
    manifests: BTreeMap<String, Manifest>,
}
impl Registry {
    pub fn new() -> Self {
        Self {
            manifests: BTreeMap::new(),
        }
    }
    pub fn register(&mut self, manifest: Manifest, reviewed_builtin: bool) -> Result<(), String> {
        manifest.validate(reviewed_builtin)?;
        if self.manifests.contains_key(&manifest.id) {
            return Err("Extension ID is already registered".into());
        }
        self.manifests.insert(manifest.id.clone(), manifest);
        Ok(())
    }
    pub fn get(&self, id: &str) -> Option<&Manifest> {
        self.manifests.get(id)
    }
    pub fn all(&self) -> impl Iterator<Item = &Manifest> {
        self.manifests.values()
    }
    pub fn builtin() -> Self {
        let mut registry = Self::new();
        let themes: Vec<Theme> = serde_json::from_str(include_str!("builtin-themes.json"))
            .expect("valid bundled theme data");
        for theme in themes {
            registry
                .register(
                    Manifest {
                        api_version: API_VERSION,
                        id: theme.id.clone(),
                        name: theme.name.clone(),
                        kind: Kind::Theme,
                        capabilities: [Capability::Theme].into_iter().collect(),
                        theme: Some(theme),
                    },
                    true,
                )
                .expect("valid built-in theme");
        }
        registry
    }
}
