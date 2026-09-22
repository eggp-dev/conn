#![cfg(unix)]
//! The desktop app asks before a new agent connection joins, and remembers the owner's setting.
use conn_core::ipc::{Client, ClientError};
use conn_frontend::AppRuntime;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn harness(dir: &std::path::Path, events: Arc<Mutex<Vec<Value>>>) -> AppRuntime {
    let mut profile = conn_core::backend::Profile::local("test".into(), "/bin/bash".into());
    profile.args = vec!["--noprofile".into(), "--norc".into()];
    profile.cwd = Some(dir.to_string_lossy().into());
    std::fs::write(dir.join("profiles.json"), serde_json::to_vec(&conn_core::profiles::Profiles { version: 1, revision: 0, default_profile: "test".into(), profiles: vec![profile] }).unwrap()).unwrap();
    let h = AppRuntime::new(dir.into(), dir.join("conn.sock"), Arc::new(move |name, v| { if name == "ss:admission" { events.lock().unwrap().push(v); } }));
    h.invoke("start", json!({"rows":24,"cols":80})).unwrap();
    h
}

fn wait_for(what: &str, ready: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while !ready() { assert!(Instant::now() < deadline, "timed out: {what}"); std::thread::sleep(Duration::from_millis(10)); }
}

#[test]
fn preparation_creates_a_private_live_shell_with_no_agent_participation() {
    let dir=tempfile::tempdir().unwrap();let h=harness(dir.path(),Default::default());
    let agent=Client::connect(&dir.path().join("conn.sock")).unwrap();let hello=agent.hello("agent","candidate").unwrap();
    h.invoke("decide_admission",json!({"connId":hello["conn"],"allow":true})).unwrap();
    let before=agent.call("list_tabs",json!({})).unwrap()["tabs"].clone();
    let id=h.invoke("prepare_terminal",json!({"rows":24,"cols":80})).unwrap();
    let status=h.invoke("status",json!({"session":id})).unwrap();
    assert_eq!(status["shared"],false);assert_eq!(status["processAlive"],true);assert_eq!(status["controller"]["type"],"human");
    assert_eq!(agent.call("list_tabs",json!({})).unwrap()["tabs"],before);
    assert!(agent.call("snapshot",json!({"session":id})).is_err());
    h.shutdown();
}

#[test]
fn desktop_asks_by_default_and_the_owner_command_admits() {
    let dir = tempfile::tempdir().unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let h = harness(dir.path(), events.clone());
    assert_eq!(h.invoke("admission_policy", json!({})).unwrap()["ask"], true);
    let agent = Client::connect(&dir.path().join("conn.sock")).unwrap();
    let hello = agent.hello("agent", "first-run").unwrap();
    assert_eq!(hello["admission"], "pending");
    assert!(matches!(agent.call("list_tabs", json!({})), Err(ClientError::Rpc { code, .. }) if code == "admission_pending"));
    wait_for("owner window hears the request", || events.lock().unwrap().iter().any(|e| e["state"] == "pending" && e["agentId"] == "first-run"));
    let waiting = h.invoke("pending_admissions", json!({})).unwrap();
    assert_eq!(waiting, json!([{"connId": hello["conn"], "agentId": "first-run"}]));
    h.invoke("decide_admission", json!({"connId": hello["conn"], "allow": true})).unwrap();
    assert_eq!(agent.call("list_tabs", json!({})).unwrap()["tabs"], 1, "ordinary tabs are open to an admitted connection");
    assert!(events.lock().unwrap().iter().any(|e| e["state"] == "granted"));
    h.shutdown();
}

#[test]
fn the_admission_setting_survives_saving_tab_defaults_and_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let h = harness(dir.path(), Default::default());
    h.invoke("set_admission", json!({"ask": false})).unwrap();
    // The settings sheet saves tab defaults as a whole object without this key.
    h.invoke("set_defaults", json!({"defaults": {"mode": "copilot", "gate": true}})).unwrap();
    assert_eq!(h.invoke("admission_policy", json!({})).unwrap()["ask"], false);
    let agent = Client::connect(&dir.path().join("conn.sock")).unwrap();
    assert_eq!(agent.hello("agent", "trusted-local").unwrap()["admission"], "granted");
    drop(agent);
    h.shutdown();
    drop(h);
    let again = harness(dir.path(), Default::default());
    assert_eq!(again.invoke("admission_policy", json!({})).unwrap()["ask"], false);
    let saved: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join("app.json")).unwrap()).unwrap();
    assert_eq!(saved["mode"], "copilot");
    again.shutdown();
}
