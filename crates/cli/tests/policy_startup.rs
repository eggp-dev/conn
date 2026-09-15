//! Malformed policy must never become an implicit allow-all session.
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn serve_refuses_invalid_policy_before_starting_the_backend() {
    for policy in ["confirm: [\n", "confirm:\n  - pattern: '('\n    label: broken\n"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("policy.yaml");
        let audit = dir.path().join("audit.jsonl");
        std::fs::write(&path, policy).unwrap();
        let shell = if cfg!(windows) { "cmd.exe" } else { "/bin/sh" };
        let profiles = conn_core::profiles::Profiles {
            version: 1, revision: 0, default_profile: "test".into(),
            profiles: vec![conn_core::backend::Profile::local("test".into(), shell.into())],
        };
        std::fs::write(dir.path().join("profiles.json"), serde_json::to_vec(&profiles).unwrap()).unwrap();
        let stderr = std::fs::File::create(dir.path().join("stderr.txt")).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_conn"))
            .arg("--policy").arg(&path)
            .arg("--audit").arg(&audit)
            .arg("--socket").arg(dir.path().join("conn.sock"))
            .arg("--profiles-file").arg(dir.path().join("profiles.json"))
            .arg("serve")
            .env_remove("CONN")
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(stderr)
            .spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() { break status; }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("conn serve kept running with an invalid policy");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(!status.success());
        let stderr = std::fs::read_to_string(dir.path().join("stderr.txt")).unwrap();
        assert!(stderr.contains("Could not load policy") && stderr.contains("policy.yaml"), "{stderr}");
        let events = conn_core::audit::read_events(&audit).unwrap();
        assert!(events.iter().any(|e| e.action == "policy_load_failed"));
        assert!(!events.iter().any(|e| e.action == "session_start"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), policy);
    }
}
