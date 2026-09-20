//! Attention is UI metadata, not an authorization boundary.
mod common;

use std::time::Duration;

use common::{Harness, SessionExt};
use conn_core::affordance::{Actor, Affordance};
use conn_core::ipc::Hub;
use conn_core::session::{ServerEvent};

fn names(evs: &[ServerEvent]) -> Vec<String> {
    evs.iter().map(|e| serde_json::to_value(e).unwrap()["event"].as_str().unwrap().to_string()).collect()
}

#[test]
fn switching_tabs_preserves_shared_session_and_human_input_preempts() {
    let mut h=Harness::new();let fe=h.frontend("ui");h.agent(1,"claude");
    h.session.agent_request_control(1).unwrap();h.session.agent_type(1,"ls").unwrap();
    let generation=h.session.participation_generation();
    h.session.set_attended(false);
    assert_eq!(generation,h.session.participation_generation());
    assert!(h.session.snapshot(Actor::Agent{conn:1}).is_ok());
    h.session.agent_type(1," -l").unwrap();
    assert!(h.session.current_lease().is_some());
    assert!(names(&fe.lock().unwrap()).contains(&"attention_changed".to_string()));
    h.session.agent_request_attention(1,Some("check result".into())).unwrap();
    h.session.human_input(b"\x03");
    assert!(h.session.current_lease().is_none());assert!(h.session.agent_type(1,"late").is_err());
    assert_eq!(h.pty_str(),"ls -l\x03");
    h.session.set_attended(true);assert!(h.session.status().attention_request.is_none());
}

#[test]
fn hub_routes_attention_across_sessions() {
    let a = Harness::new();
    let b = Harness::new();
    let sa = std::sync::Arc::new(parking_lot::Mutex::new(a.session));
    let sb = std::sync::Arc::new(parking_lot::Mutex::new(b.session));
    let hub = Hub::new();
    hub.add("t1", sa.clone());
    hub.add("t2", sb.clone());
    assert_eq!(hub.attended_id().as_deref(), Some("t1"));
    assert!(sa.lock().attended() && !sb.lock().attended());
    assert!(hub.set_attended("t2"));
    assert!(!sa.lock().attended() && sb.lock().attended());
    assert_eq!(hub.attended_id().as_deref(), Some("t2"));
    assert!(hub.get("t1").is_some());
    assert!(!hub.set_attended("nope"));
    hub.remove("t2");
    assert_eq!(hub.attended_id().as_deref(), Some("t1"));
    assert!(sa.lock().attended());
}

#[test]
fn unattended_default_is_safe_even_for_the_human_cli_path() {
    let mut h = Harness::with(&Harness::test_policy(), Duration::from_secs(60), Duration::from_secs(300));
    h.session.set_attended(false);
    // human actions are never blocked by attention
    h.session.human_input(b"echo human\r");
    assert_eq!(h.pty_str(), "echo human\r");
    assert!(h.session.affordances(Actor::Human).contains(&Affordance::Snapshot));
}

#[test]
fn grace_in_a_tab_nobody_watches_knocks_for_attention() {
    use conn_core::session::KeyResult;
    let mut h = Harness::new();
    let events = h.frontend("ui");
    h.agent(1, "a");
    h.session.set_pacing(conn_core::Pacing { enter_grace_ms: 3000, ..Default::default() });
    h.session.agent_request_control(1).unwrap();
    // Watched tab: the grace countdown is already in front of the human.
    h.session.agent_type(1, "echo watched").unwrap();
    let KeyResult::Scheduled { exec_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    assert!(!names(&events.lock().unwrap()).contains(&"attention_requested".to_string()));
    h.session.cancel_exec(&exec_id).unwrap();
    // Unwatched tab: grace is the human's chance to object, so the tab must knock.
    h.session.set_attended(false);
    h.session.pty_output(b"\r\x1b[2K$ ");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "echo background").unwrap();
    assert!(matches!(h.session.agent_send_key(1, "ENTER").unwrap(), KeyResult::Scheduled { .. }));
    assert!(names(&events.lock().unwrap()).contains(&"attention_requested".to_string()));
    assert!(h.session.status().attention_request.is_some());
}
