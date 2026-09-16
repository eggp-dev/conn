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

#[test]
fn native_windows_keep_tabs_output_and_close_lifecycle_separate() {
    let dir = tempfile::tempdir().unwrap();
    configure_profile(dir.path());
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorded = events.clone();
    let h = Harness::new(dir.path().into(), dir.path().join("conn.sock"), Arc::new(move |name, value| {
        recorded.lock().unwrap().push((name.to_owned(), value));
    }));
    let main = h.invoke("start", json!({"rows":24,"cols":80})).unwrap();
    let other = h.invoke_in_window("automation-1", "start", json!({"rows":24,"cols":80})).unwrap();
    let a = main["session"].as_str().unwrap();
    let b = other["session"].as_str().unwrap();
    assert_ne!(a,b);
    assert_eq!(main["sessions"], json!([a]));
    assert_eq!(other["sessions"], json!([b]));
    assert!(h.invoke("input", json!({"session":b,"data":"echo wrong-window\n"})).is_err());
    let extra = h.invoke_in_window("automation-1", "open_tab", json!({"rows":24,"cols":80,"profileId":"test"})).unwrap();
    assert_eq!(h.invoke("start", json!({"rows":24,"cols":80})).unwrap()["sessions"], json!([a]));
    assert_eq!(h.invoke_in_window("automation-1", "start", json!({"rows":24,"cols":80})).unwrap()["sessions"], json!([b,extra]));
    h.focus_window("main");
    assert_eq!(h.invoke("status", json!({"session":a})).unwrap()["attended"], true);
    h.focus_window("automation-1");
    assert_eq!(h.invoke_in_window("automation-1", "status", json!({"session":extra})).unwrap()["attended"], true);
    assert_eq!(h.invoke("status", json!({"session":a})).unwrap()["attended"], false);
    h.close_window("automation-1");
    assert!(h.invoke_in_window("automation-1", "start", json!({"rows":24,"cols":80})).is_err());
    assert_eq!(h.invoke("status", json!({"session":a})).unwrap()["processAlive"], true);
    h.focus_window("main");
    h.invoke("attach_output", json!({"session":a})).unwrap();
    h.invoke("input", json!({"session":a,"data":"printf alive > window-close-proof\n"})).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !dir.path().join("window-close-proof").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(std::fs::read_to_string(dir.path().join("window-close-proof")).unwrap(), "alive");
    let events = events.lock().unwrap();
    assert!(events.iter().any(|(name,v)| name == "ss:output" && v["session"] == a));
    assert!(events.iter().filter(|(_,v)| v["session"] == a).all(|(_,v)| v["window"] == "main"));
    assert!(events.iter().filter(|(_,v)| v["session"] == b || v["session"] == extra).all(|(_,v)| v["window"] == "automation-1"));
}

#[test]
fn private_output_and_terminal_responses_are_confined_to_the_owning_native_window() {
    use base64::Engine as _;
    let dir = tempfile::tempdir().unwrap();
    configure_profile(dir.path());
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorded = events.clone();
    let target: Arc<std::sync::Mutex<std::sync::Weak<Harness>>> = Default::default();
    let receiver = target.clone();
    let h = Arc::new(Harness::new(dir.path().into(), dir.path().join("conn.sock"), Arc::new(move |name, value| {
        recorded.lock().unwrap().push((name.to_owned(), value.clone()));
        if name == "ss:tab_opened" && value["externalStarting"] == true {
            if let Some(h) = receiver.lock().unwrap().upgrade() {
                h.invoke_in_window(value["window"].as_str().unwrap(), "attach_output", json!({"session":value["session"]})).unwrap();
            }
        }
    })));
    *target.lock().unwrap() = Arc::downgrade(&h);
    h.invoke_in_window("main", "automation_save", json!({"config":{"enabled":true,"profiles":["test"]}})).unwrap();
    h.prepare_automation_window("private-window").unwrap();
    h.invoke_in_window("private-window", "ui_ready", json!({})).unwrap();
    h.automate_in_window("private-window", conn_frontend::automation::Caller {
        identity: "synthetic-caller:1".into(), name: "Synthetic launcher".into(), still_alive: Arc::new(|| true),
    }, "window.create", json!({"command":"/bin/sh -c 'printf SYNTHETIC_WINDOW_OUTPUT; sleep 60'"})).unwrap();
    let started = h.invoke_in_window("private-window", "start", json!({"rows":24,"cols":80})).unwrap();
    let id = started["session"].as_str().unwrap();
    assert!(h.invoke("attach_output", json!({"session":id})).is_err());
    assert!(h.invoke_in_window("main", "terminal_response", json!({"session":id,"data":"\u{1b}[0n"})).is_err());
    h.invoke_in_window("private-window", "attach_output", json!({"session":id})).unwrap();
    h.invoke_in_window("private-window", "terminal_response", json!({"session":id,"data":"\u{1b}[0n"})).unwrap();
    assert_eq!(h.invoke_in_window("private-window", "status", json!({"session":id})).unwrap()["externalInputAvailable"], true);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let output: Vec<u8> = events.lock().unwrap().iter().filter(|(name,v)| name == "ss:output" && v["session"] == id)
            .flat_map(|(_,v)| base64::engine::general_purpose::STANDARD.decode(v["data"].as_str().unwrap()).unwrap()).collect();
        if String::from_utf8_lossy(&output).contains("SYNTHETIC_WINDOW_OUTPUT") { break; }
        assert!(Instant::now() < deadline, "private output was not rendered");
        std::thread::sleep(Duration::from_millis(10));
    }
    let captured = events.lock().unwrap();
    assert!(captured.iter().filter(|(_,v)| v["session"] == id).all(|(_,v)| v["window"] == "private-window"));
    assert!(captured.iter().filter(|(name,_)| name != "ss:output").all(|(_,v)| !v.to_string().contains("SYNTHETIC_WINDOW_OUTPUT")));
    drop(captured);
    assert!(!std::fs::read_to_string(dir.path().join("audit.jsonl")).unwrap_or_default().contains("SYNTHETIC_WINDOW_OUTPUT"));
    h.close_window("private-window");
}
