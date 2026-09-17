//! Small extension host. Reviewed providers receive only a host-authorized visible frame.
//! The host never writes to a PTY. A one-shot proposal must be accepted through Conn's
//! normal actor/input path, with a fresh authoritative frame and sharing check.
mod manifest;
mod provider;
mod secrets;

use manifest::Registry;
pub use manifest::{
    Capability, Kind, Manifest, API_VERSION, COMPLETION_ID, DEFAULT_THEME, PROVIDER_ID,
};
use parking_lot::Mutex;
use provider::{CompletionProvider, OpenAi};
use secrets::{OsSecretStore, SecretStore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

const MAX_CONTEXT: usize = 16 * 1024;
const MAX_INFLIGHT: usize = 2;
const REQUESTS_PER_MINUTE: usize = 20;
const JOB_TTL: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FrameToken {
    pub session: String,
    pub surface_id: String,
    pub generation: u64,
    pub frame_id: u64,
}
/// Deliberately not Deserialize: never construct from an untrusted webview request.
pub struct VisibleContext {
    pub token: FrameToken,
    pub lines: Vec<String>,
    pub shared: bool,
    pub prompt_ready: bool,
    /// Trusted user invocation can request a suggestion when shell integration is unavailable.
    pub explicit: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub theme: String,
    pub model: String,
    pub provider_enabled: bool,
    pub completion_enabled: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: DEFAULT_THEME.into(),
            model: String::new(),
            provider_enabled: false,
            completion_enabled: false,
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
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Ready,
    Failed,
    Cancelled,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionJob {
    pub id: String,
    #[serde(flatten)]
    pub token: FrameToken,
    pub status: JobStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
#[derive(Clone)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
}
struct Job {
    view: CompletionJob,
    cancellation: Cancellation,
    created: Instant,
    revision: u64,
}
struct State {
    settings: Settings,
    themes: Vec<Manifest>,
    registry: Registry,
    revision: u64,
    jobs: HashMap<String, Job>,
    inflight: usize,
    requests: VecDeque<Instant>,
}
struct Inner {
    state: Mutex<State>,
    secrets: Arc<dyn SecretStore>,
    provider: Arc<dyn CompletionProvider>,
    path: PathBuf,
}
pub struct Extensions {
    inner: Arc<Inner>,
}

fn valid_model(model: &str) -> bool {
    model.len() <= 100
        && model
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.' | b':'))
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
fn cancel_locked(state: &mut State, session: Option<&str>) {
    for job in state
        .jobs
        .values_mut()
        .filter(|j| session.is_none_or(|s| j.view.token.session == s))
    {
        job.cancellation.cancel();
        job.view.status = JobStatus::Cancelled;
        job.view.text = None;
        job.view.error = None;
    }
}

impl Extensions {
    pub fn load(config_dir: &Path) -> Self {
        Self::with_adapters(
            config_dir,
            Arc::new(OsSecretStore::new(config_dir)),
            Arc::new(OpenAi),
        )
    }
    fn with_adapters(
        config_dir: &Path,
        secrets: Arc<dyn SecretStore>,
        provider: Arc<dyn CompletionProvider>,
    ) -> Self {
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
        let settings = if valid_model(&stored.settings.model)
            && registry
                .get(&stored.settings.theme)
                .is_some_and(|m| m.kind == Kind::Theme)
        {
            stored.settings
        } else {
            Settings::default()
        };
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State {
                    settings,
                    themes,
                    registry,
                    revision: 0,
                    jobs: HashMap::new(),
                    inflight: 0,
                    requests: VecDeque::new(),
                }),
                secrets,
                provider,
                path,
            }),
        }
    }
    pub fn snapshot(&self) -> Value {
        let key_status = match self.inner.secrets.get() {
            Ok(Some(_)) => "stored",
            Ok(None) => "missing",
            Err(_) => "unavailable",
        };
        let state = self.inner.state.lock();
        json!({
            "apiVersion": API_VERSION,
            "settings": state.settings,
            "keyStatus": key_status,
            "extensions": state.registry.all().map(|manifest| json!({
                "id": manifest.id, "name": manifest.name, "kind": manifest.kind, "capabilities": manifest.capabilities,
                "enabled": match manifest.kind { Kind::Theme => state.settings.theme == manifest.id, Kind::Provider => state.settings.provider_enabled, Kind::Completion => state.settings.completion_enabled }
            })).collect::<Vec<_>>(),
            "themes": state.registry.all().filter_map(|m| m.theme.clone()).collect::<Vec<_>>()
        })
    }
    #[cfg(test)]
    pub fn settings(&self) -> Settings {
        self.inner.state.lock().settings.clone()
    }
    /// Bridge regression fixture: never reads the OS store or starts a provider worker.
    /// Production builds do not expose a way to inject proposals.
    #[cfg(test)]
    pub(crate) fn seed_ready_for_test(&self, token: FrameToken) -> CompletionJob {
        let mut state = self.inner.state.lock();
        state.settings.provider_enabled = true;
        state.settings.completion_enabled = true;
        let view = CompletionJob {
            id: uuid::Uuid::new_v4().to_string(),
            token,
            status: JobStatus::Ready,
            text: Some(" synthetic-command".into()),
            error: None,
        };
        let revision = state.revision;
        state.jobs.insert(
            view.id.clone(),
            Job {
                view: view.clone(),
                cancellation: Cancellation::new(),
                created: Instant::now(),
                revision,
            },
        );
        view
    }
    pub fn configure(&self, id: &str, config: Value) -> Result<Value, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Switch {
            enabled: bool,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ProviderConfig {
            enabled: bool,
            model: String,
        }
        let mut state = self.inner.state.lock();
        let kind = state
            .registry
            .get(id)
            .ok_or("Unknown extension")?
            .kind
            .clone();
        let previous = state.settings.clone();
        match kind {
            Kind::Theme => {
                let c: Switch =
                    serde_json::from_value(config).map_err(|_| "Invalid theme settings")?;
                state.settings.theme = if c.enabled {
                    id.into()
                } else {
                    DEFAULT_THEME.into()
                };
            }
            Kind::Provider => {
                let c: ProviderConfig =
                    serde_json::from_value(config).map_err(|_| "Invalid provider settings")?;
                if !valid_model(&c.model) || (c.enabled && c.model.is_empty()) {
                    return Err("Choose a valid model ID".into());
                }
                state.settings.provider_enabled = c.enabled;
                state.settings.model = c.model;
            }
            Kind::Completion => {
                let c: Switch =
                    serde_json::from_value(config).map_err(|_| "Invalid suggestion settings")?;
                state.settings.completion_enabled = c.enabled;
            }
        }
        if let Err(error) = persist(&self.inner.path, &state) {
            state.settings = previous;
            return Err(error);
        }
        state.revision += 1;
        cancel_locked(&mut state, None);
        Ok(json!({"settings":state.settings}))
    }
    pub fn install_theme(&self, manifest: Manifest) -> Result<(), String> {
        let mut state = self.inner.state.lock();
        if state.themes.len() >= 32 {
            return Err("Theme limit reached".into());
        }
        manifest.validate(false)?;
        if state.registry.get(&manifest.id).is_some() {
            return Err("Extension ID is already registered".into());
        }
        state.themes.push(manifest.clone());
        if let Err(e) = persist(&self.inner.path, &state) {
            state.themes.pop();
            return Err(e);
        }
        state.registry.register(manifest, false)
    }
    pub fn set_api_key(&self, key: String) -> Result<(), String> {
        let key = Zeroizing::new(key);
        if key.len() < 16 || key.len() > 1024 || !key.bytes().all(|c| c.is_ascii_graphic()) {
            return Err("Invalid API key format".into());
        }
        self.inner.secrets.set(&key)?;
        let mut state = self.inner.state.lock();
        state.revision += 1;
        cancel_locked(&mut state, None);
        Ok(())
    }
    pub fn delete_api_key(&self) -> Result<(), String> {
        // Stop future result delivery even when deletion fails because the keychain is locked.
        {
            let mut state = self.inner.state.lock();
            state.revision += 1;
            cancel_locked(&mut state, None);
        }
        self.inner.secrets.delete()
    }
    pub fn cancel(&self, session: &str) {
        cancel_locked(&mut self.inner.state.lock(), Some(session));
    }
    pub fn cancel_all(&self) {
        cancel_locked(&mut self.inner.state.lock(), None);
    }

    pub fn start_completion(&self, context: VisibleContext) -> Result<CompletionJob, String> {
        if !context.shared {
            return Err("Share this terminal before requesting suggestions".into());
        }
        if !context.prompt_ready && !context.explicit {
            return Err("Suggestions require a confirmed shell prompt".into());
        }
        if context.token.session.is_empty()
            || context.token.surface_id.is_empty()
            || context.token.frame_id == 0
            || context.lines.is_empty()
            || context.lines.len() > 500
        {
            return Err("Visible terminal frame is unavailable".into());
        }
        if context.lines.iter().map(String::len).sum::<usize>()
            + context.lines.len().saturating_sub(1)
            > MAX_CONTEXT
            || context
                .lines
                .iter()
                .any(|line| line.chars().any(char::is_control))
        {
            return Err("Visible terminal context is too large or invalid".into());
        }
        let mut state = self.inner.state.lock();
        if !state.settings.provider_enabled || !state.settings.completion_enabled {
            return Err("Enable a model provider and command suggestions first".into());
        }
        for (id, capability) in [
            (PROVIDER_ID, Capability::ModelRequest),
            (COMPLETION_ID, Capability::VisibleFrame),
            (COMPLETION_ID, Capability::ModelRequest),
            (COMPLETION_ID, Capability::Proposal),
        ] {
            if !state.registry.allows(id, capability) {
                return Err("Extension permission unavailable".into());
            }
        }
        if state.inflight >= MAX_INFLIGHT {
            return Err("A suggestion is already in progress".into());
        }
        let now = Instant::now();
        state
            .requests
            .retain(|at| now.duration_since(*at) < Duration::from_secs(60));
        if state.requests.len() >= REQUESTS_PER_MINUTE {
            return Err("Suggestion request limit reached; try again shortly".into());
        }
        cancel_locked(&mut state, Some(&context.token.session));
        state.jobs.retain(|_, j| {
            now.duration_since(j.created) < JOB_TTL && j.view.token.session != context.token.session
        });
        let cancellation = Cancellation::new();
        let view = CompletionJob {
            id: uuid::Uuid::new_v4().to_string(),
            token: context.token.clone(),
            status: JobStatus::Pending,
            text: None,
            error: None,
        };
        let id = view.id.clone();
        let revision = state.revision;
        state.jobs.insert(
            id.clone(),
            Job {
                view: view.clone(),
                cancellation: cancellation.clone(),
                created: now,
                revision,
            },
        );
        state.requests.push_back(now);
        state.inflight += 1;
        let model = state.settings.model.clone();
        let inner = self.inner.clone();
        let spawn = std::thread::Builder::new()
            .name("conn-suggestion".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if cancellation.is_cancelled() {
                        return Err("Suggestion cancelled".into());
                    }
                    let key = inner
                        .secrets
                        .get()?
                        .ok_or("Save an API key before requesting suggestions")?;
                    if cancellation.is_cancelled() {
                        return Err("Suggestion cancelled".into());
                    }
                    if now.elapsed() >= JOB_TTL {
                        return Err("Suggestion expired; request a new one".into());
                    }
                    inner
                        .provider
                        .complete(&model, key, &context, &cancellation)
                }))
                .unwrap_or_else(|_| Err("Model provider stopped unexpectedly".into()));
                let mut state = inner.state.lock();
                state.inflight = state.inflight.saturating_sub(1);
                let current_revision = state.revision;
                if let Some(job) = state.jobs.get_mut(&id) {
                    if job.cancellation.is_cancelled()
                        || revision != current_revision
                        || job.created.elapsed() >= JOB_TTL
                    {
                        job.view.status = JobStatus::Cancelled;
                    } else {
                        match result.and_then(|text| {
                            provider::validate_suggestion(&text)?;
                            Ok(text)
                        }) {
                            Ok(text) => {
                                job.view.status = JobStatus::Ready;
                                job.view.text = Some(text);
                            }
                            Err(error) => {
                                job.view.status = JobStatus::Failed;
                                job.view.error = Some(error);
                            }
                        }
                    }
                }
            });
        if spawn.is_err() {
            state.inflight -= 1;
            state.jobs.remove(&view.id);
            return Err("Could not start suggestion".into());
        }
        Ok(view)
    }
    pub fn completion(&self, session: &str, id: &str) -> Result<CompletionJob, String> {
        let mut state = self.inner.state.lock();
        let job = state
            .jobs
            .get_mut(id)
            .filter(|j| j.view.token.session == session)
            .ok_or("Suggestion unavailable")?;
        if job.created.elapsed() >= JOB_TTL {
            job.cancellation.cancel();
            job.view.status = JobStatus::Cancelled;
            job.view.text = None;
            job.view.error = None;
        }
        Ok(job.view.clone())
    }
    /// Called only after root verifies current sharing, visible surface, prompt and control.
    /// Removes the proposal on every attempt so a rejected acceptance cannot be replayed.
    pub fn take_proposal(
        &self,
        session: &str,
        id: &str,
        current: &FrameToken,
    ) -> Result<String, String> {
        let mut state = self.inner.state.lock();
        if !state
            .jobs
            .get(id)
            .is_some_and(|j| j.view.token.session == session)
        {
            return Err("Suggestion unavailable".into());
        }
        let job = state.jobs.remove(id).ok_or("Suggestion unavailable")?;
        if job.view.status != JobStatus::Ready
            || job.cancellation.is_cancelled()
            || job.created.elapsed() >= JOB_TTL
            || job.revision != state.revision
            || &job.view.token != current
            || !state.settings.completion_enabled
            || !state.settings.provider_enabled
        {
            return Err("Suggestion expired; request a new one".into());
        }
        job.view.text.ok_or_else(|| "Suggestion unavailable".into())
    }
}
impl Drop for Extensions {
    fn drop(&mut self) {
        self.cancel_all();
    }
}

#[cfg(test)]
mod tests;
