#![cfg(unix)]
//! Connection admission and tab-scoped leases over the real transport.
//! Ordinary tabs are open to every admitted connection, so admission is the consent point.
mod common;
use common::{present, Harness};
use conn_core::affordance::Affordance;
use conn_core::ipc::{self, AdmissionPolicy, Client, ClientError, Hub, SharedHub};
use conn_core::session::{ControllerInfo, SharedSession};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn session() -> SharedSession {
    let s: SharedSession = Arc::new(Harness::new().session.into());
    present(&mut s.lock(), vec!["$".into()]);
    s
}

fn serve(hub: &SharedHub) -> (tempfile::TempDir, std::path::PathBuf, ipc::ServerGuard) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("admission.sock");
    let guard = ipc::serve_in_background(path.clone(), hub.clone()).unwrap();
    (dir, path, guard)
}

fn code(result: Result<serde_json::Value, ClientError>) -> String {
    match result { Err(ClientError::Rpc { code, .. }) => code, other => panic!("expected an RPC error, got {other:?}") }
}

fn wait_until(what: &str, ready: impl Fn() -> bool) {
    for _ in 0..200 { if ready() { return; } std::thread::sleep(Duration::from_millis(10)); }
    panic!("timed out: {what}");
}

#[test]
fn pending_connection_learns_nothing_until_the_owner_allows_it() {
    let hub = Hub::single("tab", session());
    hub.set_admission_policy(AdmissionPolicy::Ask);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    hub.set_admission_listener(Arc::new(move |c| log.lock().unwrap().push((c.conn_id, c.agent_id, c.state))));
    let (_dir, path, _guard) = serve(&hub);
    let client = Arc::new(Client::connect(&path).unwrap());
    let hello = client.hello("agent", "newcomer").unwrap();
    assert_eq!(hello["admission"], "pending");
    assert!(hello["session"].is_null() && hello["mode"].is_null(), "{hello}");
    let conn = hello["conn"].as_u64().unwrap();
    for method in ["list_tabs", "sessions", "affordances", "navigation_affordances", "status"] {
        assert_eq!(code(client.call(method, json!({}))), "admission_pending", "{method}");
    }
    assert!(hub.agent_connections().is_empty(), "a pending connection is not a sharing candidate");
    assert_eq!(hub.pending_admissions().len(), 1);
    assert_eq!(seen.lock().unwrap().as_slice(), &[(conn, "newcomer".to_string(), "pending")]);

    // Real work waits for the decision, so the first call succeeds right after "Allow".
    let waiting = { let client = client.clone(); std::thread::spawn(move || client.call("snapshot", json!({}))) };
    std::thread::sleep(Duration::from_millis(150));
    assert!(!waiting.is_finished());
    assert!(hub.decide_admission(conn, true));
    let snapshot = waiting.join().unwrap().unwrap();
    assert_eq!(snapshot["screen"][0], "$");
    assert!(!hub.decide_admission(conn, false), "a decision is final for that connection");
    assert_eq!(hub.agent_connections().len(), 1);
    assert_eq!(seen.lock().unwrap().last().unwrap().2, "granted");
    assert_eq!(client.call("list_tabs", json!({})).unwrap()["tabs"], 1);
}

#[test]
fn denied_and_departed_connections_stay_out() {
    let tab = session();
    let hub = Hub::single("tab", tab.clone());
    hub.set_admission_policy(AdmissionPolicy::Ask);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    hub.set_admission_listener(Arc::new(move |c| log.lock().unwrap().push(c.state)));
    let (_dir, path, _guard) = serve(&hub);

    let denied = Client::connect(&path).unwrap();
    let conn = denied.hello("agent", "stranger").unwrap()["conn"].as_u64().unwrap();
    assert!(hub.decide_admission(conn, false));
    for method in ["snapshot", "request_control", "list_tabs"] {
        assert_eq!(code(denied.call(method, json!({}))), "admission_denied", "{method}");
    }
    assert!(matches!(tab.lock().status().controller, ControllerInfo::Human));

    // Same display name, new connection: nothing is inherited.
    let again = Client::connect(&path).unwrap();
    assert_eq!(again.hello("agent", "stranger").unwrap()["admission"], "pending");
    drop(again);
    wait_until("pending connection closed", || seen.lock().unwrap().last() == Some(&"closed"));
    assert!(hub.pending_admissions().is_empty());
}

#[test]
fn embedders_admit_by_default_and_relaxing_the_policy_admits_waiters() {
    let hub = Hub::single("tab", session());
    let (_dir, path, _guard) = serve(&hub);
    let open = Client::connect(&path).unwrap();
    assert_eq!(open.hello("agent", "embedded").unwrap()["admission"], "granted");

    hub.set_admission_policy(AdmissionPolicy::Ask);
    let waiter = Client::connect(&path).unwrap();
    assert_eq!(waiter.hello("agent", "waiter").unwrap()["admission"], "pending");
    assert!(open.call("snapshot", json!({})).is_ok(), "tightening never evicts an admitted connection");
    hub.set_admission_policy(AdmissionPolicy::Allow);
    assert!(waiter.call("snapshot", json!({})).is_ok());
}

#[test]
fn moving_to_another_tab_releases_everything_held_in_the_source() {
    let (old, target) = (session(), session());
    let hub = Hub::single("old", old.clone());
    hub.set_opener(Arc::new(|_, _| Err("unused".into())));
    hub.add("target", target.clone());
    let (_dir, path, _guard) = serve(&hub);
    let agent = Client::connect(&path).unwrap();
    agent.hello("agent", "mover").unwrap();
    agent.call("request_control", json!({"reason":"work"})).unwrap();
    assert!(matches!(old.lock().status().controller, ControllerInfo::Agent { .. }));
    agent.call("switch_tab", json!({"tab":"target"})).unwrap();
    assert!(matches!(old.lock().status().controller, ControllerInfo::Human), "a lease belongs to its tab");
    // The freed tab is immediately available to someone else.
    let other = Client::connect(&path).unwrap();
    other.hello("agent", "other").unwrap();
    other.call("request_control", json!({"session":"old","reason":"next"})).unwrap();
}

#[test]
fn an_owner_mask_pins_an_agent_only_while_its_source_is_usable() {
    let (old, target) = (session(), session());
    let hub = Hub::single("old", old.clone());
    hub.set_opener(Arc::new(|_, _| Err("unused".into())));
    hub.add("target", target.clone());
    let (_dir, path, _guard) = serve(&hub);
    let agent = Client::connect(&path).unwrap();
    agent.hello("agent", "pinned").unwrap();
    old.lock().set_affordance_mask(Some([Affordance::Snapshot, Affordance::RequestControl].into_iter().collect()));
    assert_eq!(agent.call("navigation_affordances", json!({})).unwrap(), json!([]));
    assert_eq!(code(agent.call("switch_tab", json!({"tab":"target"}))), "masked");
    // Recovery is never blocked by a source that can no longer be used.
    old.lock().process_exited(Some(0));
    assert_eq!(agent.call("navigation_affordances", json!({})).unwrap(), json!(["switch_tab"]));
    agent.call("switch_tab", json!({"tab":"target"})).unwrap();
}

#[test]
fn naming_a_session_explicitly_cannot_collect_leases_across_tabs() {
    // Found on macOS: the binding was not enough, because any request may name a session.
    let (first, second, third) = (session(), session(), session());
    let hub = Hub::single("first", first.clone());
    hub.set_opener(Arc::new(|_, _| Err("unused".into())));
    hub.add("second", second.clone());
    hub.add("third", third.clone());
    let (_dir, path, _guard) = serve(&hub);
    let agent = Client::connect(&path).unwrap();
    agent.hello("agent", "collector").unwrap();
    let held = |s: &SharedSession| matches!(s.lock().status().controller, ControllerInfo::Agent { .. });
    agent.call("request_control", json!({"reason":"bound tab"})).unwrap();
    agent.call("request_control", json!({"session":"second","reason":"named tab"})).unwrap();
    assert!(!held(&first) && held(&second), "asking for control elsewhere gives up the previous lease");
    // Reads elsewhere are free and cost nothing.
    agent.call("snapshot", json!({"session":"first"})).unwrap();
    assert!(held(&second));
    // Renewing in place keeps the lease.
    agent.call("request_control", json!({"session":"second","reason":"renew"})).unwrap();
    assert!(held(&second));
    // Moving releases a lease taken by name, not only the one in the bound tab.
    agent.call("switch_tab", json!({"tab":"third"})).unwrap();
    assert!(!held(&first) && !held(&second) && !held(&third));
}
