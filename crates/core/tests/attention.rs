//! Attention: the agent sees only what the human sees.
mod common;

use std::time::Duration;

use common::Harness;
use conn_core::affordance::{Actor, Affordance};
use conn_core::ipc::Hub;
use conn_core::session::{AgentMode, KeyResult, ServerEvent, SessionError};

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
    assert!(h.session.current_lease().is_some(), "the lease itself survives the absence");
    assert!(names(&agent.lock().unwrap()).contains(&"control_suspended".to_string()));
    assert!(names(&fe.lock().unwrap()).contains(&"attention_changed".to_string()));
    assert_eq!(h.pty_str(), "ls", "nothing reached the shell while away");

    // the agent may knock
    h.session.agent_request_attention(1, Some("need you to check the result".into())).unwrap();
    assert_eq!(h.session.status().attention_request.unwrap().reason.as_deref(), Some("need you to check the result"));

    h.session.set_attended(true);
    assert!(h.session.snapshot(Actor::Agent { conn: 1 }).is_ok());
    assert!(h.session.agent_type(1, "x").is_ok());
    assert!(h.session.status().attention_request.is_none());
    assert!(names(&agent.lock().unwrap()).contains(&"control_resumed".to_string()));
}

#[test]
fn entrusting_keeps_the_agent_going_under_the_cap() {
    // example policy: unattended = copilot
    let mut h = Harness::new();
    h.agent(1, "claude");
    h.session.agent_request_control(1).unwrap();
    assert_eq!(h.session.mode(), AgentMode::Autopilot);
    let who = h.session.entrust().unwrap();
    assert_eq!(who, "claude");
    h.session.set_attended(false);
    // still allowed to see and act — but as copilot, not autopilot
    assert!(h.session.snapshot(Actor::Agent { conn: 1 }).is_ok());
    assert_eq!(h.session.effective_mode(), AgentMode::Copilot);
    h.session.agent_type(1, "echo hi").unwrap();
    assert_eq!(h.pty_str(), "", "copilot cap: typing becomes a proposal, not shell input");
    let r = h.session.agent_send_key_with(1, "ENTER", Some("greet".into())).unwrap();
    assert!(matches!(r, KeyResult::Proposed { .. }), "{r:?}");
    // another agent is not entrusted
    h.agent(2, "copilot");
    assert!(matches!(h.session.snapshot(Actor::Agent { conn: 2 }), Err(SessionError::Unattended)));
    // returning clears the entrustment and the cap
    h.session.set_attended(true);
    assert!(h.session.entrusted_agent().is_none());
    assert_eq!(h.session.effective_mode(), AgentMode::Autopilot);
}

#[test]
fn entrust_requires_a_connected_agent() {
    let mut h = Harness::new();
    assert!(h.session.entrust().is_err());
    h.agent(1, "claude");
    h.session.agent_request_control(1).unwrap();
    h.session.human_take();
    assert_eq!(h.session.entrust().unwrap(), "claude", "falls back to the last agent and re-grants");
    assert!(h.session.current_lease().is_some());
    h.session.connection_closed(1);
    assert!(h.session.entrusted_agent().is_none());
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
