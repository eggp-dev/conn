//! Frontend-facing behaviour: events, headless approvals, pacing, affordance masks.
mod common;

use std::collections::HashSet;
use std::time::{Duration, Instant};

use common::Harness;
use conn_core::affordance::{Actor, Affordance};
use conn_core::approval::{ApprovalState, Decision};
use conn_core::policy::EXAMPLE_POLICY;
use conn_core::session::{ExecState, KeyResult, ServerEvent, SessionError};
use conn_core::Pacing;

fn names(evs: &[ServerEvent]) -> Vec<String> {
    evs.iter().map(|e| serde_json::to_value(e).unwrap()["event"].as_str().unwrap().to_string()).collect()
}

#[test]
fn frontend_receives_lifecycle_events() {
    let mut h = Harness::headless();
    let fe = h.frontend("ui");
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "kubectl delete pod x").unwrap();
    let KeyResult::Pending { approval_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    h.session.resolve_approval(&approval_id, Decision::Grant, "frontend").unwrap();
    h.session.human_input(b"x");
    h.session.pty_output(b"out");
    common::present(&mut h.session, vec!["out".into()]);
    h.session.tick(Instant::now());
    let evs = fe.lock().unwrap().clone();
    let n = names(&evs);
    for expected in ["control_granted", "agent_input", "approval_requested", "approval_resolved", "agent_exec", "control_revoked", "screen_changed"] {
        assert!(n.contains(&expected.to_string()), "missing {expected} in {n:?}");
    }
    assert!(!n.contains(&"output".to_string()), "no output streaming unless requested");
    let req = evs.iter().find_map(|e| match e { ServerEvent::ApprovalRequested { request } => Some(request.clone()), _ => None }).unwrap();
    assert_eq!(req.cmd, "kubectl delete pod x");
    assert_eq!(req.label, "delete resource");
}

#[test]
fn headless_mode_does_not_draw_the_prompt() {
    let mut h = Harness::headless();
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "sudo ls").unwrap();
    assert!(matches!(h.session.agent_send_key(1, "ENTER").unwrap(), KeyResult::Pending { .. }));
    assert!(!h.session.prompt_active());
    assert!(!h.stdout_str().contains("\x1b[?1049h"));
    // human keystrokes are a takeover, not a prompt answer
    h.session.human_input(b"d");
    assert!(h.session.controller().is_human());
    assert_eq!(h.session.status().pending.len(), 1, "approval still pending; frontend decides");
    h.session.resolve_first_pending(Decision::Deny, "frontend").unwrap();
    assert!(h.pty_str().ends_with("\x15"));
}

#[test]
fn output_streaming_to_subscribers() {
    let mut h = Harness::new();
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    h.session.subscribe("ui", Box::new(common::VecSink(events.clone())), true);
    h.session.pty_output(b"hello");
    let evs = events.lock().unwrap();
    let data = evs.iter().find_map(|e| match e { ServerEvent::Output { data, .. } => Some(data.clone()), _ => None }).unwrap();
    use base64::Engine as _;
    assert_eq!(base64::engine::general_purpose::STANDARD.decode(data).unwrap(), b"hello");
}

#[test]
fn enter_grace_schedules_then_executes() {
    let mut h = Harness::new();
    h.session.set_pacing(Pacing { enter_grace_ms: 30, ..Pacing::default() });
    let fe = h.frontend("ui");
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "echo hi").unwrap();
    let r = h.session.agent_send_key(1, "ENTER").unwrap();
    let KeyResult::Scheduled { exec_id, grace_ms, .. } = r else { panic!("{r:?}") };
    assert_eq!(grace_ms, 30);
    assert_eq!(h.pty_str(), "echo hi", "ENTER not sent yet");
    assert_eq!(h.session.exec_state(&exec_id).unwrap().0, ExecState::Scheduled);
    assert!(matches!(h.session.agent_type(1, "x"), Err(SessionError::ExecPending(_))));
    h.session.tick(Instant::now());
    assert_eq!(h.session.exec_state(&exec_id).unwrap().0, ExecState::Scheduled, "not due yet");
    h.session.tick(Instant::now() + Duration::from_millis(50));
    assert_eq!(h.session.exec_state(&exec_id).unwrap().0, ExecState::Executed);
    assert_eq!(h.pty_str(), "echo hi\r");
    assert!(names(&fe.lock().unwrap()).contains(&"exec_scheduled".to_string()));
}

#[test]
fn human_input_cancels_grace_window() {
    let mut h = Harness::new();
    h.session.set_pacing(Pacing { enter_grace_ms: 10_000, ..Pacing::default() });
    let fe = h.frontend("ui");
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "echo hi").unwrap();
    let KeyResult::Scheduled { exec_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    h.session.human_input(b"\x15");
    let (state, reason) = h.session.exec_state(&exec_id).unwrap();
    assert_eq!(state, ExecState::Cancelled);
    assert_eq!(reason.as_deref(), Some("human_input"));
    h.session.tick(Instant::now() + Duration::from_secs(20));
    assert_eq!(h.pty_str(), "echo hi\x15", "cancelled exec never sends ENTER");
    assert!(names(&fe.lock().unwrap()).contains(&"exec_cancelled".to_string()));
    // frontend-side cancel of a fresh schedule
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "ls").unwrap();
    let KeyResult::Scheduled { exec_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    h.session.cancel_exec(&exec_id).unwrap();
    assert_eq!(h.session.exec_state(&exec_id).unwrap().0, ExecState::Cancelled);
    assert!(h.session.cancel_exec(&exec_id).is_err());
}

#[test]
fn rate_limit_reports_retry_after() {
    let mut h = Harness::new();
    h.session.set_pacing(Pacing { min_write_interval_ms: 200, ..Pacing::default() });
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "a").unwrap();
    match h.session.agent_type(1, "b") {
        Err(SessionError::RateLimited { retry_after_ms }) => assert!(retry_after_ms > 0 && retry_after_ms <= 200),
        other => panic!("{other:?}"),
    }
    assert_eq!(h.pty_str(), "a");
    std::thread::sleep(Duration::from_millis(220));
    h.session.agent_type(1, "b").unwrap();
    assert_eq!(h.pty_str(), "ab");
}

#[test]
fn affordance_mask_restricts_agents() {
    let mut h = Harness::new();
    let fe = h.frontend("ui");
    h.agent(1, "copilot");
    h.session.set_affordance_mask(Some(HashSet::from([Affordance::Snapshot])));
    assert_eq!(h.session.affordances(Actor::Agent { conn: 1 }), vec![Affordance::Snapshot]);
    assert!(matches!(h.session.agent_request_control(1), Err(SessionError::Masked(_))));
    assert!(h.session.snapshot(Actor::Agent { conn: 1 }).is_ok());
    // lift → full table again
    h.session.set_affordance_mask(None);
    assert!(h.session.affordances(Actor::Agent { conn: 1 }).contains(&Affordance::RequestControl));
    h.session.agent_request_control(1).unwrap();
    // read-only mid-lease: write tools vanish, snapshot stays
    h.session.set_affordance_mask(Some(HashSet::from([Affordance::Snapshot, Affordance::ReleaseControl])));
    assert!(matches!(h.session.agent_type(1, "x"), Err(SessionError::Masked(_))));
    assert_eq!(h.session.affordances(Actor::Agent { conn: 1 }), vec![Affordance::Snapshot, Affordance::ReleaseControl]);
    assert!(names(&fe.lock().unwrap()).contains(&"affordance_mask_changed".to_string()));
    assert_eq!(h.session.status().affordance_mask, Some(vec![Affordance::ReleaseControl, Affordance::Snapshot]));
}

#[test]
fn set_pacing_updates_ttls_and_notifies() {
    let mut h = Harness::new();
    let fe = h.frontend("ui");
    h.session.set_pacing(Pacing { lease_ttl_secs: 1, approval_ttl_secs: 1, ..Pacing::default() });
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.tick(Instant::now() + Duration::from_millis(1500));
    assert!(h.session.controller().is_human(), "lease expired under the new TTL");
    assert!(names(&fe.lock().unwrap()).contains(&"pacing_changed".to_string()));
    assert_eq!(h.session.status().pacing.lease_ttl_secs, 1);
}

#[test]
fn approval_expires_via_pacing_ttl() {
    let mut h = Harness::new();
    h.session.set_pacing(Pacing { approval_ttl_secs: 1, ..Pacing::default() });
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "sudo ls").unwrap();
    let KeyResult::Pending { approval_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    h.session.tick(Instant::now() + Duration::from_secs(2));
    assert_eq!(h.session.check_approval(&approval_id).unwrap().state, ApprovalState::Expired);
}

#[test]
fn example_policy_still_applies_in_headless_mode() {
    let mut h = Harness::headless();
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "rm -rf /").unwrap();
    assert!(matches!(h.session.agent_send_key(1, "ENTER").unwrap(), KeyResult::Denied { .. }));
    let _ = EXAMPLE_POLICY;
}

#[test]
fn lease_survives_while_approval_is_pending() {
    let mut h = Harness::with(&Harness::test_policy(), Duration::from_millis(30), Duration::from_secs(300));
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "sudo ls").unwrap();
    let KeyResult::Pending { approval_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    for i in 1..=5 {
        h.session.tick(Instant::now() + Duration::from_millis(25 * i));
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!h.session.controller().is_human(), "lease must not expire while the human is deciding");
    h.session.resolve_approval(&approval_id, Decision::Deny, "frontend").unwrap();
    h.session.tick(Instant::now() + Duration::from_secs(1));
    assert!(h.session.controller().is_human(), "after the decision the normal TTL applies");
}
