#![cfg(unix)]
//! Test the default engine's home-directory policy in an isolated subprocess.
use conn_core::{audit::Audit, Engine, EngineConfig};

#[test]
fn default_engine_refuses_invalid_policy_before_spawning_a_shell() {
    const CHILD: &str = "CONN_POLICY_STARTUP_FIXTURE";
    if std::env::var_os(CHILD).is_some() {
        let (audit, records) = Audit::memory();
        let result = Engine::spawn(EngineConfig {
            shell: Some("/bin/sh".into()),
            audit: Some(audit),
            ..EngineConfig::default()
        });
        let error = match result {
            Err(error) => error,
            Ok(engine) => {
                engine.terminate().unwrap();
                panic!("default engine started with an invalid policy");
            }
        };
        assert!(error.to_string().contains("Could not load policy"), "{error}");
        let records = records.lock().unwrap();
        assert!(records.iter().any(|e| e.action == "policy_load_failed"));
        assert!(!records.iter().any(|e| e.action == "session_start"));
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".conn")).unwrap();
    std::fs::write(dir.path().join(".conn/policy.yaml"), "confirm: [\n").unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "default_engine_refuses_invalid_policy_before_spawning_a_shell", "--nocapture"])
        .env(CHILD, "1").env("HOME", dir.path()).output().unwrap();
    assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
}
