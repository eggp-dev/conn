use super::*;
use secrets::Secret;
use std::sync::atomic::AtomicUsize;

#[derive(Default)]
struct MemorySecrets {
    key: Mutex<Option<String>>,
    reads: AtomicUsize,
}
impl SecretStore for MemorySecrets {
    fn get(&self) -> Result<Option<Secret>, String> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.key.lock().clone().map(Zeroizing::new))
    }
    fn set(&self, key: &str) -> Result<(), String> {
        *self.key.lock() = Some(key.to_owned());
        Ok(())
    }
    fn delete(&self) -> Result<(), String> {
        self.key.lock().take();
        Ok(())
    }
}
struct FakeProvider {
    calls: AtomicUsize,
    wait: bool,
    answer: String,
}
impl CompletionProvider for FakeProvider {
    fn complete(
        &self,
        _: &str,
        _: Secret,
        context: &VisibleContext,
        cancel: &Cancellation,
    ) -> Result<String, String> {
        assert!(context.shared && (context.prompt_ready || context.explicit));
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.wait {
            while !cancel.is_cancelled() {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        Ok(self.answer.clone())
    }
}
fn fixture(
    wait: bool,
    answer: &str,
) -> (
    tempfile::TempDir,
    Extensions,
    Arc<MemorySecrets>,
    Arc<FakeProvider>,
) {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::default());
    let provider = Arc::new(FakeProvider {
        calls: AtomicUsize::new(0),
        wait,
        answer: answer.into(),
    });
    let host = Extensions::with_adapters(dir.path(), secrets.clone(), provider.clone());
    (dir, host, secrets, provider)
}
fn enable(host: &Extensions) {
    host.set_api_key("sk-local-test-only-not-a-real-key".into())
        .unwrap();
    host.configure(PROVIDER_ID, json!({"enabled":true,"model":"test-model"}))
        .unwrap();
    host.configure(COMPLETION_ID, json!({"enabled":true}))
        .unwrap();
}
fn context() -> VisibleContext {
    VisibleContext {
        token: FrameToken {
            session: "s1".into(),
            surface_id: "surface1".into(),
            generation: 4,
            frame_id: 18,
        },
        lines: vec!["user$ git".into()],
        shared: true,
        prompt_ready: true,
        explicit: false,
    }
}
fn wait_job(host: &Extensions, job: &CompletionJob) -> CompletionJob {
    for _ in 0..200 {
        let j = host.completion(&job.token.session, &job.id).unwrap();
        if j.status != JobStatus::Pending {
            return j;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("test provider did not finish")
}
#[test]
fn private_or_unconfirmed_prompt_never_reads_credentials_or_calls_provider() {
    let (_dir, host, secrets, provider) = fixture(false, " status");
    enable(&host);
    let reads = secrets.reads.load(Ordering::SeqCst);
    let mut private = context();
    private.shared = false;
    assert!(host.start_completion(private).is_err());
    let mut unknown = context();
    unknown.prompt_ready = false;
    assert!(host.start_completion(unknown).is_err());
    assert_eq!(secrets.reads.load(Ordering::SeqCst), reads);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
}
#[test]
fn default_is_off_and_key_never_serializes_or_enters_settings_file() {
    let (dir, host, _secrets, provider) = fixture(false, " status");
    assert!(host.start_completion(context()).is_err());
    enable(&host);
    let serialized = serde_json::to_string(&host.snapshot()).unwrap();
    let disk = std::fs::read_to_string(dir.path().join("extensions.json")).unwrap();
    assert!(!serialized.contains("sk-local") && !disk.contains("sk-local"));
    assert_eq!(host.snapshot()["keyStatus"], "stored");
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    host.delete_api_key().unwrap();
    assert_eq!(host.snapshot()["keyStatus"], "missing");
    let missing = host.start_completion(context()).unwrap();
    assert_eq!(wait_job(&host, &missing).status, JobStatus::Failed);
}
#[test]
fn proposal_requires_exact_frame_and_is_one_shot() {
    let (_dir, host, _, _) = fixture(false, " status");
    enable(&host);
    let job = host.start_completion(context()).unwrap();
    assert_eq!(wait_job(&host, &job).status, JobStatus::Ready);
    let mut stale = job.token.clone();
    stale.frame_id += 1;
    assert!(host.take_proposal("s1", &job.id, &stale).is_err());
    assert!(host.take_proposal("s1", &job.id, &job.token).is_err());
    let job = host.start_completion(context()).unwrap();
    wait_job(&host, &job);
    assert!(host
        .take_proposal("wrong-session", &job.id, &job.token)
        .is_err());
    assert_eq!(
        host.take_proposal("s1", &job.id, &job.token).unwrap(),
        " status"
    );
    assert!(host.take_proposal("s1", &job.id, &job.token).is_err());
}
#[test]
fn disable_or_cancel_drops_inflight_results() {
    let (_dir, host, _, provider) = fixture(true, " status");
    enable(&host);
    let job = host.start_completion(context()).unwrap();
    for _ in 0..100 {
        if provider.calls.load(Ordering::SeqCst) > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    host.configure(COMPLETION_ID, json!({"enabled":false}))
        .unwrap();
    assert_eq!(wait_job(&host, &job).status, JobStatus::Cancelled);
    assert!(host.take_proposal("s1", &job.id, &job.token).is_err());
    assert!(host.start_completion(context()).is_err());
}
#[test]
fn configuration_model_or_key_change_invalidates_ready_proposals() {
    let (_dir, host, _, _) = fixture(false, " status");
    enable(&host);
    let job = host.start_completion(context()).unwrap();
    wait_job(&host, &job);
    host.configure(PROVIDER_ID, json!({"enabled":true,"model":"another-model"}))
        .unwrap();
    assert!(host.take_proposal("s1", &job.id, &job.token).is_err());
    let job = host.start_completion(context()).unwrap();
    wait_job(&host, &job);
    host.set_api_key("sk-different-test-key-value".into())
        .unwrap();
    assert!(host.take_proposal("s1", &job.id, &job.token).is_err());
}
#[test]
fn unknown_executable_manifest_or_excess_capability_is_rejected() {
    let registry = Registry::builtin();
    let mut manifest = registry.get(PROVIDER_ID).unwrap().clone();
    assert!(manifest.validate(false).is_err());
    manifest.id = "thirdparty.provider".into();
    assert!(manifest.validate(true).is_err());
    let mut theme = registry.get(DEFAULT_THEME).unwrap().clone();
    theme.capabilities.insert(Capability::VisibleFrame);
    assert!(theme.validate(false).is_err());
    theme.capabilities.remove(&Capability::VisibleFrame);
    theme.api_version = 999;
    assert!(theme.validate(false).is_err());
    let value = json!({"apiVersion":1,"id":"evil.theme","name":"Evil","kind":"theme","capabilities":["theme"],"entrypoint":"/bin/sh"});
    assert!(serde_json::from_value::<Manifest>(value).is_err());
}
#[test]
fn declarative_theme_persists_without_executable_or_ui_injection() {
    let (dir, host, secrets, provider) = fixture(false, " status");
    let mut manifest = Registry::builtin().get(DEFAULT_THEME).unwrap().clone();
    manifest.id = "user.theme".into();
    manifest.name = "User".into();
    let theme = manifest.theme.as_mut().unwrap();
    theme.id = "user.theme".into();
    theme.name = "User".into();
    host.install_theme(manifest.clone()).unwrap();
    host.configure("user.theme", json!({"enabled":true}))
        .unwrap();
    let restored = Extensions::with_adapters(dir.path(), secrets, provider);
    assert_eq!(restored.settings().theme, "user.theme");
    assert!(host.install_theme(manifest.clone()).is_err());
    manifest.theme.as_mut().unwrap().foreground = "url(https://outside.invalid)".into();
    assert!(manifest.validate(false).is_err());
}
#[test]
fn unknown_configuration_fields_and_endpoint_changes_are_rejected() {
    let (_dir, host, _, _) = fixture(false, " status");
    assert!(host
        .configure(
            PROVIDER_ID,
            json!({"enabled":true,"model":"x","endpoint":"https://elsewhere.invalid"})
        )
        .is_err());
    assert!(host
        .configure(
            PROVIDER_ID,
            json!({"enabled":true,"model":"x\nInjected: yes"})
        )
        .is_err());
    assert!(host
        .configure(COMPLETION_ID, json!({"enabled":true,"apiKey":"secret"}))
        .is_err());
    assert!(host
        .set_api_key("secret with spaces or newline\n".into())
        .is_err());
}
#[test]
fn oversize_frames_or_control_character_suggestions_are_rejected() {
    let (_dir, host, _, provider) = fixture(false, "ls\nwhoami");
    enable(&host);
    let mut huge = context();
    huge.lines = vec!["x".repeat(MAX_CONTEXT + 1)];
    assert!(host.start_completion(huge).is_err());
    let mut wrapped = context();
    wrapped.lines = vec!["x".repeat(MAX_CONTEXT / 2); 2];
    assert!(host.start_completion(wrapped).is_err());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    let job = host.start_completion(context()).unwrap();
    assert_eq!(wait_job(&host, &job).status, JobStatus::Failed);
    assert!(host.take_proposal("s1", &job.id, &job.token).is_err());
}
#[test]
fn inflight_cap_prevents_unbounded_model_requests() {
    let (_dir, host, _, _) = fixture(true, " status");
    enable(&host);
    host.start_completion(context()).unwrap();
    let mut second = context();
    second.token.session = "s2".into();
    host.start_completion(second).unwrap();
    let mut third = context();
    third.token.session = "s3".into();
    assert!(host.start_completion(third).is_err());
    host.cancel_all();
}
struct UnavailableStore;
impl SecretStore for UnavailableStore {
    fn get(&self) -> Result<Option<Secret>, String> {
        Err("OS credential store is unavailable".into())
    }
    fn set(&self, _: &str) -> Result<(), String> {
        Err("OS credential store is unavailable".into())
    }
    fn delete(&self) -> Result<(), String> {
        Err("OS credential store is unavailable".into())
    }
}
#[test]
fn unavailable_keychain_has_no_plaintext_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let host = Extensions::with_adapters(
        dir.path(),
        Arc::new(UnavailableStore),
        Arc::new(FakeProvider {
            calls: AtomicUsize::new(0),
            wait: false,
            answer: " status".into(),
        }),
    );
    assert!(host
        .set_api_key("sk-never-store-this-secret".into())
        .is_err());
    assert_eq!(host.snapshot()["keyStatus"], "unavailable");
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
}

struct BlockingSecrets {
    entered: AtomicBool,
    release: AtomicBool,
}
impl SecretStore for BlockingSecrets {
    fn get(&self) -> Result<Option<Secret>, String> {
        self.entered.store(true, Ordering::Release);
        while !self.release.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(2));
        }
        Ok(Some(Zeroizing::new("sk-blocking-test-credential".into())))
    }
    fn set(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn delete(&self) -> Result<(), String> {
        Ok(())
    }
}
#[test]
fn revocation_while_keychain_is_blocked_prevents_provider_call() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(BlockingSecrets {
        entered: AtomicBool::new(false),
        release: AtomicBool::new(false),
    });
    let provider = Arc::new(FakeProvider {
        calls: AtomicUsize::new(0),
        wait: false,
        answer: " status".into(),
    });
    let host = Extensions::with_adapters(dir.path(), secrets.clone(), provider.clone());
    host.configure(PROVIDER_ID, json!({"enabled":true,"model":"test-model"}))
        .unwrap();
    host.configure(COMPLETION_ID, json!({"enabled":true}))
        .unwrap();
    let job = host.start_completion(context()).unwrap();
    for _ in 0..100 {
        if secrets.entered.load(Ordering::Acquire) {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(secrets.entered.load(Ordering::Acquire));
    host.cancel("s1");
    secrets.release.store(true, Ordering::Release);
    for _ in 0..100 {
        if host.inner.state.lock().inflight == 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        host.completion("s1", &job.id).unwrap().status,
        JobStatus::Cancelled
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
}
#[test]
fn explicit_request_allows_unknown_prompt_but_never_private_screen() {
    let (_dir, host, _, provider) = fixture(false, " status");
    enable(&host);
    let mut unknown = context();
    unknown.prompt_ready = false;
    unknown.explicit = true;
    let job = host.start_completion(unknown).unwrap();
    assert_eq!(wait_job(&host, &job).status, JobStatus::Ready);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    let mut private = context();
    private.prompt_ready = false;
    private.explicit = true;
    private.shared = false;
    assert!(host.start_completion(private).is_err());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
}
