//! Attention: the agent sees only what the human sees.
mod common;

use std::time::Duration;

use common::Harness;
use conn_core::affordance::{Actor, Affordance};
use conn_core::ipc::Hub;
use conn_core::session::{ServerEvent, SessionError};

fn names(evs: &[ServerEvent]) -> Vec<String> {
    evs.iter().map(|e| serde_json::to_value(e).unwrap()["event"].as_str().unwrap().to_string()).collect()
}

#[test]
fn leaving_hides_the_screen_and_suspends_writes() {
    let mut h = Harness::new();
    let fe = h.frontend("ui");
    let agent = h.agent(1, "claude");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "ls").unwrap();

    h.session.set_attended(false);
    assert!(matches!(h.session.snapshot(Actor::Agent { conn: 1 }), Err(SessionError::Unattended)), "nobody looking → nothing to see");
    assert!(matches!(h.session.agent_type(1, "x"), Err(SessionError::Suspended)));
    assert!(matches!(h.session.agent_send_key_with(1, "ENTER", Some("t".into())), Err(SessionError::Suspended)));
    assert_eq!(h.session.affordances(Actor::Agent { conn: 1 }), vec![Affordance::RequestAttention]);
    assert!(h.session.current_lease().is_none(), "leaving revokes authority");
    assert!(names(&agent.lock().unwrap()).contains(&"control_revoked".to_string()));
    assert!(names(&fe.lock().unwrap()).contains(&"attention_changed".to_string()));
    assert_eq!(h.pty_str(), "ls", "nothing reached the shell while away");

    // the agent may knock
    h.session.agent_request_attention(1, Some("need you to check the result".into())).unwrap();
    assert_eq!(h.session.status().attention_request.unwrap().reason.as_deref(), Some("need you to check the result"));

    h.session.set_attended(true);
    common::present(&mut h.session, vec!["ls".into()]);
    assert!(h.session.snapshot(Actor::Agent { conn: 1 }).is_ok());
    h.session.human_input(b"\x15");
    h.session.agent_request_control(1).unwrap();
    assert!(h.session.agent_type(1, "x").is_ok());
    assert!(h.session.status().attention_request.is_none());
}

#[test]
fn leaving_cannot_restore_hidden_observation_or_writes() {
    let mut h = Harness::new(); h.agent(1,"claude");
    h.session.agent_request_control(1).unwrap();
    h.session.set_attended(false);
    assert!(h.session.current_lease().is_none());
    assert!(h.session.snapshot(Actor::Agent{conn:1}).is_err());
    assert!(h.session.agent_type(1,"hidden").is_err());
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
    assert_eq!(hub.resolve(None).unwrap().0, "t1");
    assert!(hub.set_attended("t2"));
    assert!(!sa.lock().attended() && sb.lock().attended());
    assert_eq!(hub.resolve(None).unwrap().0, "t2");
    assert_eq!(hub.resolve(Some("t1")).unwrap().0, "t1");
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
