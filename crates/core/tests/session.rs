mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::Harness;
use conn_core::affordance::{Actor, Affordance};
use conn_core::approval::{ApprovalState, Decision};
use conn_core::authority::{AuthorityError, RevokeReason};
use conn_core::session::{KeyResult, ServerEvent, SessionError};

#[test]
fn human_to_agent_handover() {
    let mut h = Harness::new();
    h.agent(1, "copilot");
    assert!(h.session.controller().is_human());
    let lease = h.session.agent_request_control(1).unwrap();
    assert_eq!(lease.agent_id, "copilot");
    assert_eq!(h.session.current_lease().unwrap().conn, 1);
    let aff = h.session.affordances(Actor::Agent { conn: 1 });
    assert!(aff.contains(&Affordance::Type) && aff.contains(&Affordance::SendKey));
    assert!(!aff.contains(&Affordance::RequestControl));
}

/// The core invariant: a single human byte revokes the lease *before* it reaches the PTY.
#[test]
fn preemptive_takeover_order() {
    let mut h = Harness::new();
    let events = h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    let trace = Arc::new(Mutex::new(Vec::new()));
    h.session.set_trace(trace.clone());

    h.session.human_input(b"x");

    let t = trace.lock().unwrap().clone();
    assert_eq!(t, vec!["revoke", "pty_write", "event", "audit"], "revoke must precede the PTY write");
    assert!(h.session.controller().is_human());
    assert_eq!(h.pty_str(), "x");

    // the agent's next write fails and the write tools are gone
    assert!(matches!(h.session.agent_type(1, "y"), Err(SessionError::Authority(AuthorityError::NotController))));
    assert_eq!(h.session.affordances(Actor::Agent { conn: 1 }), vec![Affordance::Snapshot, Affordance::RequestControl]);
    assert_eq!(h.pty_str(), "x", "no agent bytes after the human byte");

    let ev = events.lock().unwrap();
    assert!(ev.iter().any(|e| matches!(e, ServerEvent::ControlRevoked { reason: RevokeReason::HumanInput, .. })));
    assert!(h.audit_actions().contains(&("human".into(), "takeover".into())));
}

#[test]
fn concurrent_agent_writes_never_land_after_human_byte() {
    use conn_core::session::SharedSession;
    let h = Harness::new();
    let pty = h.pty.clone();
    let session: SharedSession = Arc::new(parking_lot::Mutex::new(h.session));
    session.lock().register_conn(1, conn_core::session::ConnKind::Agent, "a", Box::new(common::VecSink(Default::default())));
    session.lock().agent_request_control(1).unwrap();

    let s2 = session.clone();
    let writer = std::thread::spawn(move || {
        let mut ok = 0;
        for _ in 0..2000 {
            if s2.lock().agent_type(1, "a").is_ok() {
                ok += 1;
            } else {
                break;
            }
        }
        ok
    });
    std::thread::sleep(Duration::from_millis(5));
    session.lock().human_input(b"H");
    let _ = writer.join().unwrap();
    let bytes = pty.lock().unwrap().clone();
    let pos = bytes.iter().position(|b| *b == b'H').unwrap();
    assert!(bytes[pos + 1..].iter().all(|b| *b != b'a'), "agent byte after human byte");
}

#[test]
fn lease_expiry_rejects_writes() {
    let mut h = Harness::with(&Harness::test_policy(), Duration::from_millis(5), Duration::from_secs(300));
    h.agent(1, "a");
    h.session.agent_request_control(1).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    assert!(matches!(h.session.agent_type(1, "x"), Err(SessionError::Authority(AuthorityError::Expired))));
    assert!(h.session.controller().is_human());
    assert_eq!(h.pty_str(), "");
}

#[test]
fn tick_expires_lease_and_notifies() {
    let mut h = Harness::with(&Harness::test_policy(), Duration::from_millis(5), Duration::from_secs(300));
    let events = h.agent(1, "a");
    h.session.agent_request_control(1).unwrap();
    h.session.tick(Instant::now() + Duration::from_secs(1));
    assert!(h.session.controller().is_human());
    assert!(events.lock().unwrap().iter().any(|e| matches!(e, ServerEvent::ControlRevoked { reason: RevokeReason::Expired, .. })));
}

#[test]
fn disconnect_returns_control_to_human() {
    let mut h = Harness::new();
    h.agent(1, "a");
    h.session.agent_request_control(1).unwrap();
    h.session.connection_closed(1);
    assert!(h.session.controller().is_human());
}

#[test]
fn invalid_or_foreign_lease_is_rejected() {
    let mut h = Harness::new();
    h.agent(1, "a");
    h.agent(2, "b");
    h.session.agent_request_control(1).unwrap();
    assert!(matches!(h.session.agent_type(2, "x"), Err(SessionError::Authority(AuthorityError::NotController))));
    assert!(matches!(h.session.agent_request_control(2), Err(SessionError::Authority(AuthorityError::Busy { .. }))));
    assert_eq!(h.session.affordances(Actor::Agent { conn: 2 }), vec![Affordance::Snapshot]);
    assert!(h.session.agent_release_control(2).is_err());
    h.session.agent_release_control(1).unwrap();
    assert!(h.session.controller().is_human());
}

#[test]
fn allow_executes_and_audits() {
    let mut h = Harness::new();
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "echo agent").unwrap();
    let r = h.session.agent_send_key(1, "ENTER").unwrap();
    assert!(matches!(r, KeyResult::Executed { ref cmd } if cmd == "echo agent"));
    assert_eq!(h.pty_str(), "echo agent\r");
    let ev = h.audit_events();
    let exec = ev.iter().find(|e| e.action == "exec").unwrap();
    assert_eq!(exec.actor, "copilot");
    assert_eq!(exec.fields["cmd"], "echo agent");
    assert_eq!(exec.fields["policy"], "allow");
}

#[test]
fn deny_blocks_and_clears_line_without_approval() {
    let mut h = Harness::new();
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "rm -rf /").unwrap();
    let r = h.session.agent_send_key(1, "ENTER").unwrap();
    assert!(matches!(r, KeyResult::Denied { .. }));
    assert_eq!(h.pty_str(), "rm -rf /\x15");
    assert!(h.session.status().pending.is_empty());
    assert!(h.session.resolve_first_pending(Decision::Grant, "cli").is_err());
    assert!(!h.stdout_str().contains("\x1b[?1049h"));
}

#[test]
fn confirm_then_deny_from_prompt() {
    let mut h = Harness::new();
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "rm -rf ./tmp").unwrap();
    let r = h.session.agent_send_key(1, "ENTER").unwrap();
    let id = match r {
        KeyResult::Pending { approval_id, label, .. } => {
            assert_eq!(label, "recursive delete");
            approval_id
        }
        other => panic!("{other:?}"),
    };
    assert_eq!(h.pty_str(), "rm -rf ./tmp", "ENTER must not reach the PTY");
    assert!(h.stdout_str().contains("\x1b[?1049h") && h.stdout_str().contains("recursive delete"));
    assert!(h.session.prompt_active());
    assert_eq!(h.session.check_approval(&id).unwrap().state, ApprovalState::Pending);
    assert!(h.session.affordances(Actor::Agent { conn: 1 }).contains(&Affordance::CheckApproval));
    common::present(&mut h.session, vec!["Approval: recursive delete".into()]);
    assert!(matches!(h.session.agent_type(1, "x"), Err(SessionError::ApprovalPending(_))));

    // PTY output while the prompt is up is held back, then flushed
    h.session.pty_output(b"late");
    assert!(!h.stdout_str().contains("late"));

    h.session.human_input(b"d");
    assert!(h.session.controller().lease().is_some(), "answering the prompt is not a takeover");
    assert_eq!(h.session.check_approval(&id).unwrap().state, ApprovalState::Denied);
    assert_eq!(h.pty_str(), "rm -rf ./tmp\x15");
    assert!(h.stdout_str().ends_with("\x1b[?1049llate"));
    assert!(!h.session.prompt_active());
    let exec = h.audit_events().into_iter().find(|e| e.action == "exec").unwrap();
    assert_eq!(exec.fields["policy"], "confirm");
    assert_eq!(exec.fields["approval"], "denied");
}

#[test]
fn confirm_then_grant_via_cli_and_session_allow() {
    let mut h = Harness::new();
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "kubectl delete pod a").unwrap();
    let KeyResult::Pending { approval_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    common::present(&mut h.session, vec!["Approval: delete resource".into()]);
    let info = h.session.resolve_approval(&approval_id, Decision::Grant, "cli").unwrap();
    assert_eq!(info.state, ApprovalState::Granted);
    assert_eq!(h.pty_str(), "kubectl delete pod a\r");

    // second time: [A] promotes the label for the session
    h.clear_pty();
    common::present(&mut h.session, vec!["$ ".into()]);
    h.session.agent_type(1, "kubectl delete pod b").unwrap();
    assert!(matches!(h.session.agent_send_key(1, "ENTER").unwrap(), KeyResult::Pending { .. }));
    common::present(&mut h.session, vec!["Approval: delete resource".into()]);
    h.session.human_input(b"A");
    assert_eq!(h.pty_str(), "kubectl delete pod b\r");
    assert_eq!(h.session.status().session_allows, vec!["delete resource".to_string()]);

    h.clear_pty();
    common::present(&mut h.session, vec!["$ ".into()]);
    h.session.agent_type(1, "kubectl delete pod c").unwrap();
    assert!(matches!(h.session.agent_send_key(1, "ENTER").unwrap(), KeyResult::Executed { .. }));
    assert_eq!(h.pty_str(), "kubectl delete pod c\r");
    // deny is never lifted
    h.session.agent_type(1, "rm -rf /").unwrap();
    assert!(matches!(h.session.agent_send_key(1, "ENTER").unwrap(), KeyResult::Denied { .. }));
}

#[test]
fn approval_expires_via_tick() {
    let mut h = Harness::with(&Harness::test_policy(), Duration::from_secs(60), Duration::from_millis(1));
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "sudo ls").unwrap();
    let KeyResult::Pending { approval_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    h.session.tick(Instant::now() + Duration::from_secs(1));
    assert_eq!(h.session.check_approval(&approval_id).unwrap().state, ApprovalState::Expired);
    assert_eq!(h.pty_str(), "sudo ls\x15");
    assert!(!h.session.prompt_active());
}

#[test]
fn interrupt_cancels_pending_approval() {
    let mut h = Harness::new();
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "sudo ls").unwrap();
    let KeyResult::Pending { approval_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    common::present(&mut h.session, vec!["Approval: sudo ls".into()]);
    h.session.agent_interrupt(1).unwrap();
    assert_eq!(h.session.check_approval(&approval_id).unwrap().state, ApprovalState::Denied);
    assert_eq!(h.pty_str(), "sudo ls\x15\x03");
}

#[test]
fn agent_disconnect_denies_its_pending_approval() {
    let mut h = Harness::new();
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "sudo ls").unwrap();
    let KeyResult::Pending { approval_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    h.session.connection_closed(1);
    assert_eq!(h.session.check_approval(&approval_id).unwrap().state, ApprovalState::Denied);
    assert!(!h.session.prompt_active());
}

#[test]
fn type_rejects_newlines() {
    let mut h = Harness::new();
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    assert!(matches!(h.session.agent_type(1, "rm -rf /\n"), Err(SessionError::InvalidInput(_))));
    assert_eq!(h.pty_str(), "");
    assert!(matches!(h.session.agent_send_key(1, "F13"), Err(SessionError::InvalidInput(_))));
}

#[test]
fn human_takeover_discards_command_tracking_without_recording_input() {
    let mut h = Harness::new();
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "echo ").unwrap();
    h.session.human_input(b"hi"); // takeover, line continues
    assert_eq!(h.session.input_line(), "");
    h.session.human_input(b"\r");
    let ev = h.audit_events();
    assert!(!ev.iter().any(|e| e.actor == "human" && e.action == "exec"));
}

#[test]
fn human_enter_inside_alternate_screen_is_not_audited() {
    let mut h = Harness::new();
    h.session.pty_output(b"\x1b[?1049h");
    h.session.human_input(b"jjj\r");
    assert!(!h.audit_actions().iter().any(|(a, act)| a == "human" && act == "exec"));
}

#[test]
fn tab_completion_falls_back_to_vt_model() {
    let mut h = Harness::new();
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.pty_output(b"dev$ ");
    common::present(&mut h.session, vec!["dev$ ".into()]);
    h.session.agent_type(1, "ls sr").unwrap();
    h.session.pty_output(b"ls sr");
    common::present(&mut h.session, vec!["dev$ ls sr".into()]);
    h.session.agent_send_key(1, "TAB").unwrap();
    h.session.pty_output(b"c/"); // shell completed to "ls src/"
    common::present(&mut h.session, vec!["dev$ ls src/".into()]);
    let r = h.session.agent_send_key(1, "ENTER").unwrap();
    assert!(matches!(r, KeyResult::Executed { ref cmd } if cmd == "ls src/"), "{r:?}");
}

#[test]
fn clean_tracker_wins_over_unrelated_or_stale_screen_output() {
    let mut h = Harness::new();
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.pty_output(b"$ ");
    common::present(&mut h.session, vec!["$ ".into()]);
    h.session.agent_type(1, "ls").unwrap();
    h.session.pty_output(b"rm -rf ./tmp"); // output is not the submitted input
    common::present(&mut h.session, vec!["$ rm -rf ./tmp".into()]);
    let r = h.session.agent_send_key(1, "ENTER").unwrap();
    assert!(matches!(r, KeyResult::Executed { ref cmd } if cmd == "ls"), "{r:?}");
}

#[test]
fn process_exit_revokes_and_limits_affordances() {
    let mut h = Harness::new();
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.process_exited(Some(0));
    assert!(h.session.controller().is_human());
    assert_eq!(h.session.affordances(Actor::Agent { conn: 1 }), vec![Affordance::Snapshot]);
    assert!(matches!(h.session.agent_request_control(1), Err(SessionError::ProcessExited)));
    assert!(h.audit_actions().contains(&("system".into(), "session_end".into())));
}

#[test]
fn snapshot_matches_screen_and_audits_observe() {
    let mut h = Harness::new();
    h.agent(1, "copilot");
    h.session.pty_output(b"dev$ kubectl get pods\r\napi-1  Running\r\ndev$ ");
    common::present(&mut h.session, vec!["dev$ kubectl get pods".into(), "api-1  Running".into(), "dev$".into()]);
    let s = h.session.snapshot(Actor::Agent { conn: 1 }).unwrap();
    assert_eq!(s.projection.screen, vec!["dev$ kubectl get pods", "api-1  Running", "dev$"]);
    assert!(s.process_alive);
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["controller"]["type"], "human");
    assert!(h.audit_actions().contains(&("copilot".into(), "observe".into())));
}

#[test]
fn human_take_via_cli() {
    let mut h = Harness::new();
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    assert!(h.session.affordances(Actor::Human).contains(&Affordance::Take));
    assert_eq!(h.session.human_take(), Some("lease#1".into()));
    assert!(h.session.controller().is_human());
    assert_eq!(h.session.human_take(), None);
}
