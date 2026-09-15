#![cfg(unix)]
use conn_frontend::Harness;
use serde_json::json;
use std::{sync::Arc, time::{Duration, Instant}};

fn configure_profile(dir: &std::path::Path) {
    let mut profile = conn_core::backend::Profile::local("test".into(), "/bin/bash".into());
    profile.cwd = Some(dir.to_string_lossy().into());
    profile.args = vec!["--noprofile".into(), "--norc".into()];
    std::fs::write(dir.join("profiles.json"), serde_json::to_vec(&conn_core::profiles::Profiles {
        version: 1, revision: 0, default_profile: "test".into(), profiles: vec![profile],
    }).unwrap()).unwrap();
}

fn wait_for_audit(dir: &std::path::Path, action: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let events = conn_core::audit::read_events(&dir.join("audit.jsonl")).unwrap();
        if events.iter().any(|e| e.action == action) { return; }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("missing audit action {action}");
}

#[test]
fn invalid_policy_prevents_shell_start_and_can_be_corrected() {
    let dir = tempfile::tempdir().unwrap();
    configure_profile(dir.path());
    let path = dir.path().join("policy.yaml");
    let malformed = "confirm: [\n";
    std::fs::write(&path, malformed).unwrap();
    let h = Harness::new(dir.path().into(), dir.path().join("conn.sock"), Arc::new(|_, _| {}));
    let error = h.invoke("start", json!({"rows":24,"cols":80})).unwrap_err();
    assert!(error.contains("Could not load policy") && error.contains("policy.yaml"), "{error}");
    let events = conn_core::audit::read_events(&dir.path().join("audit.jsonl")).unwrap();
    assert!(events.iter().any(|e| e.action == "policy_load_failed"));
    assert!(!events.iter().any(|e| e.action == "session_start"), "invalid policy must not create a shell");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), malformed);

    std::fs::write(&path, "default: deny\n").unwrap();
    let started = h.invoke("start", json!({"rows":24,"cols":80})).unwrap();
    let result = h.invoke("policy_test", json!({"session":started["session"],"cmd":"echo hello"})).unwrap();
    assert_eq!(result["policy"], "deny", "corrected explicit policy is used");
}

#[test]
fn invalid_policy_reload_retains_last_good_rules_and_blocks_new_tabs() {
    let dir = tempfile::tempdir().unwrap();
    configure_profile(dir.path());
    let path = dir.path().join("policy.yaml");
    std::fs::write(&path, "default: deny\n").unwrap();
    let h = Harness::new(dir.path().into(), dir.path().join("conn.sock"), Arc::new(|_, _| {}));
    let started = h.invoke("start", json!({"rows":24,"cols":80})).unwrap();
    let session = &started["session"];
    // Settings validate before overwriting a good file.
    assert!(h.invoke("policy_write", json!({"session":session,"text":"confirm: [\n"})).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "default: deny\n");

    // An external editor can still write an invalid policy; reload keeps the old one.
    std::fs::write(&path, "confirm: [\n").unwrap();
    let modified = std::time::SystemTime::now() + Duration::from_secs(2);
    std::fs::File::options().write(true).open(&path).unwrap().set_modified(modified).unwrap();
    wait_for_audit(dir.path(), "policy_reload_failed");
    assert_eq!(h.invoke("policy_test", json!({"session":session,"cmd":"echo hello"})).unwrap()["policy"], "deny");
    assert!(h.invoke("open_tab", json!({"rows":24,"cols":80,"profileId":"test"})).unwrap_err().contains("Could not load policy"));
    assert_eq!(h.invoke("diagnostics", json!({"session":session})).unwrap()["sessions"], 1);

    // Fixing the file restores normal reload behavior, including a deliberately permissive policy.
    h.invoke("policy_write", json!({"session":session,"text":"default: allow\n"})).unwrap();
    wait_for_audit(dir.path(), "policy_reloaded");
    assert_eq!(h.invoke("policy_test", json!({"session":session,"cmd":"echo hello"})).unwrap()["policy"], "allow");
}

#[test]
fn shared_commands_execute_native_shell_and_preserve_tab_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let mut profile = conn_core::backend::Profile::local("test".into(), "/bin/bash".into());
    profile.cwd = Some(dir.path().to_string_lossy().into());
    profile.args = vec!["--noprofile".into(), "--norc".into()];
    std::fs::write(dir.path().join("profiles.json"), serde_json::to_vec(&conn_core::profiles::Profiles {
        version: 1, revision: 0, default_profile: "test".into(), profiles: vec![profile],
    }).unwrap()).unwrap();
    let h = Harness::new(dir.path().into(), dir.path().join("conn.sock"), Arc::new(|_, _| {}));
    h.invoke("set_defaults", json!({"defaults":{"gate":true}})).unwrap();
    let started = h.invoke("start", json!({"rows":24,"cols":80})).unwrap();
    let id = started["session"].as_str().unwrap();
    let status = h.invoke("status", json!({"session":id})).unwrap();
    assert_eq!(status["controlGate"], true);
    h.invoke("input", json!({"session":id,"data":"printf '%s' \"$TERM\" > harness-proof\n"})).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !dir.path().join("harness-proof").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(std::fs::read_to_string(dir.path().join("harness-proof")).unwrap(), "xterm-256color");
    let second = h.invoke("open_tab", json!({"rows":30,"cols":100,"profileId":"test"})).unwrap();
    assert_ne!(second, started["session"]);
    assert_eq!(h.invoke("status", json!({"session":second})).unwrap()["controlGate"], true);
    h.invoke("close_tab", json!({"session":id})).unwrap();
    assert!(h.invoke("status", json!({"session":id})).is_err());
    h.shutdown();
}
