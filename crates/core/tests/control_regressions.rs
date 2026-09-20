//! Regressions from the 0.3.0/0.4.1 hands-on collaboration review.
mod common;

use common::{Harness, SessionExt};
use conn_core::affordance::Actor;
use conn_core::approval::{ApprovalState, Decision};
use conn_core::backend::{Profile, ShellKind};
use conn_core::session::{AgentMode, ConnKind, ControlOutcome, KeyResult, ProposalState, ServerEvent, SessionError};
use serde_json::json;

#[test]
fn mode_switch_refuses_physical_input_until_cancelled() {
    let mut h = Harness::new();
    h.agent(1, "claude");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "echo mode-switch-test").unwrap();
    assert!(matches!(h.session.set_mode(AgentMode::Copilot), Err(SessionError::InputPending)));
    assert_eq!(h.session.mode(), AgentMode::Autopilot);
    assert_eq!(h.pty_str(), "echo mode-switch-test", "rejected switch neither types nor executes");
    h.session.agent_interrupt(1).unwrap();
    h.session.set_mode(AgentMode::Copilot).unwrap();
    h.session.agent_type(1, "echo new-proposal").unwrap();
    let KeyResult::Proposed { proposal_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    h.session.accept_proposal(&proposal_id).unwrap();
    assert_eq!(h.pty_str(), "echo mode-switch-test\x03echo new-proposal\r");
}

#[test]
fn cancelled_grace_retains_tracking_and_cannot_hide_pending_input() {
    let mut h = Harness::new();
    h.agent(1, "claude");
    let mut pacing = h.session.pacing().clone(); pacing.enter_grace_ms = 5000; pacing.lease_ttl_secs = 60; pacing.approval_ttl_secs = 300;
    h.session.set_pacing(pacing);
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "echo pending").unwrap();
    let KeyResult::Scheduled { exec_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    h.session.cancel_exec(&exec_id).unwrap();
    assert!(matches!(h.session.set_mode(AgentMode::Copilot), Err(SessionError::InputPending)));
    h.session.human_input(b"\x15");
    h.session.set_mode(AgentMode::Copilot).unwrap();
}

#[test]
fn dirty_history_input_blocks_mode_switch_even_when_tracker_text_is_empty() {
    let mut h = Harness::new();
    h.agent(1, "claude");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_send_key(1, "UP").unwrap();
    assert!(matches!(h.session.set_mode(AgentMode::Copilot), Err(SessionError::InputPending)));
}

#[test]
fn tab_return_preserves_lease_but_release_requires_new_control_decision() {
    let mut h = Harness::new();
    h.agent(1, "claude");
    h.session.agent_request_control(1).unwrap();
    h.session.set_attended(false);
    h.session.set_control_gate(true);
    h.session.set_attended(true);
    assert!(h.session.current_lease().is_some());
    h.session.agent_release_control(1).unwrap();
    assert!(h.session.current_lease().is_none());
    assert!(matches!(h.session.agent_request_control_with(1, None).unwrap(), ControlOutcome::Pending { .. }));
}

#[test]
fn human_return_does_not_bypass_control_gate() {
    let mut h = Harness::new();
    h.agent(1, "claude");
    h.session.agent_request_control(1).unwrap();
    h.session.human_take();
    h.session.set_control_gate(true);
    assert!(h.session.current_lease().is_none());
    assert!(matches!(h.session.agent_request_control_with(1, None).unwrap(), ControlOutcome::Pending { .. }));
    h.session.set_attended(true);
    common::present(&mut h.session, vec![]);
}

#[test]
fn copilot_interrupt_reaches_pty_and_rejects_proposal() {
    let mut h = Harness::new();
    h.agent(1, "claude");
    h.session.set_mode(AgentMode::Copilot).unwrap();
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "echo proposed").unwrap();
    let id = h.session.proposal().unwrap().proposal_id.clone();
    h.session.agent_interrupt(1).unwrap();
    assert_eq!(h.pty_bytes(), b"\x03");
    assert_eq!(h.session.proposal_state(&id), Some(ProposalState::Rejected));
    h.session.set_attended(false);
    h.session.agent_interrupt(1).unwrap();
    assert_eq!(h.pty_bytes(), b"\x03\x03", "background interrupt reaches the same shared shell");
}

#[test]
fn revoked_sharing_cannot_append_to_existing_shell_input() {
    let mut h=Harness::new();h.agent(1,"claude");h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1,"echo stale").unwrap();h.session.set_shared(false).unwrap();
    assert!(h.session.agent_type(1,"echo proposed").is_err());
    assert!(h.session.agent_interrupt(1).is_err());
    h.session.set_attended(true);common::present(&mut h.session,vec!["echo stale".into()]);
    h.session.human_input(b"\x15");
    assert_eq!(h.pty_str(),"echo stale\x15");
}

#[test]
fn review_required_allow_session_rejection_keeps_request_pending() {
    let mut h = Harness::new();
    let mut profile = Profile::local("powershell".into(), "pwsh".into()); profile.shell = ShellKind::PowerShell;
    h.session.set_execution_profile(profile);
    h.agent(1, "claude");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "Write-Output hello").unwrap();
    let KeyResult::Pending { approval_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    let session = std::sync::Arc::new(parking_lot::Mutex::new(h.session));
    let err = conn_core::ipc::dispatch(&session, 1, "approve", &json!({"approvalId":approval_id,"decision":"grant"})).unwrap_err();
    assert_eq!(err.code, "owner_required", "the requesting agent cannot approve its own command");
    let mut s = session.lock();
    assert!(matches!(s.resolve_approval(&approval_id, Decision::AllowSession, "cli"), Err(SessionError::InvalidInput(_))));
    assert_eq!(s.check_approval(&approval_id).unwrap().state, ApprovalState::Pending);
    assert!(s.status().session_allows.is_empty());
    assert_eq!(String::from_utf8_lossy(&h.pty.lock().unwrap()), "Write-Output hello");
    s.resolve_approval(&approval_id, Decision::Grant, "cli").unwrap();
    assert_eq!(String::from_utf8_lossy(&h.pty.lock().unwrap()), "Write-Output hello\r");
}

#[test]
fn modes_are_available_in_presented_snapshot_and_changes_reach_agents() {
    let mut h=Harness::new();let events=h.agent(1,"claude");
    h.session.set_mode(AgentMode::Copilot).unwrap();
    let snap=h.session.snapshot(Actor::Agent{conn:1}).unwrap();
    assert_eq!(snap.mode,AgentMode::Copilot);assert_eq!(snap.effective_mode,AgentMode::Copilot);
    h.session.set_mode(AgentMode::Observe).unwrap();
    assert!(events.lock().unwrap().iter().any(|e| matches!(e,ServerEvent::ModeChanged {mode:AgentMode::Observe,effective_mode:AgentMode::Observe})));
}

#[test]
fn status_distinguishes_same_named_sockets_without_duplicate_agent_labels() {
    let mut h = Harness::new();
    h.agent(2, "copilot"); h.agent(1, "copilot");
    let st = h.session.status();
    assert_eq!(st.connected_agents, ["copilot"]);
    assert_eq!(st.agent_connections.iter().map(|c| c.conn_id).collect::<Vec<_>>(), [1, 2]);
    h.session.connection_closed(1);
    assert_eq!(h.session.status().agent_connections.len(), 1);
    h.session.connection_closed(2);
    assert!(h.session.status().connected_agents.is_empty());
}

#[test]
fn grace_cosign_records_distinct_live_and_saved_evidence() {
    let mut h = Harness::new();
    let events = h.frontend("ui"); h.agent(1, "claude");
    let mut pacing = h.session.pacing().clone(); pacing.enter_grace_ms = 5000; pacing.lease_ttl_secs = 60; pacing.approval_ttl_secs = 300; h.session.set_pacing(pacing);
    h.session.agent_request_control(1).unwrap(); h.session.agent_type(1, "echo test").unwrap();
    let KeyResult::Scheduled { exec_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    h.session.execute_now_scheduled(&exec_id).unwrap();
    assert!(events.lock().unwrap().iter().any(|e| matches!(e, ServerEvent::ExecCosigned {exec_id:id,agent_id} if id == &exec_id && agent_id == "claude")));
    assert!(h.audit_events().iter().any(|e| e.action == "exec" && e.fields["by"] == "human_cosign"));
    assert!(!events.lock().unwrap().iter().any(|e| matches!(e, ServerEvent::ApprovalResolved {..})));
}

#[cfg(unix)]
#[test]
fn real_pty_copilot_interrupt_stops_running_shell_command() {
    use conn_core::{Engine, EngineConfig};
    use conn_core::policy::{Policy, PolicyStore};
    use std::time::{Duration, Instant};
    fn wait_for(mut predicate: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if predicate() { return; }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(predicate(), "PTY did not reach expected state in five seconds");
    }
    let dir = tempfile::tempdir().unwrap();
    let mut profile = Profile::local("interrupt-test".into(), "/bin/sh".into());
    profile.args = vec!["-i".into()];
    let engine = Engine::spawn(EngineConfig {
        profile: Some(profile), cwd: Some(dir.path().to_path_buf()),
        env: vec![("PS1".into(), "conn-interrupt$ ".into()), ("ENV".into(), String::new())],
        audit: Some(conn_core::audit::Audit::null()),
        policy: Some(PolicyStore::from_policy(Policy::parse(&Harness::test_policy()).unwrap())),
        ..EngineConfig::default()
    }).unwrap();
    let session = engine.session();
    wait_for(|| session.lock().screen().cursor_line().contains("conn-interrupt$"));
    {
        let mut s = session.lock();
        let id = 1;
        s.register_conn(id, ConnKind::Agent, "interrupt-test", Box::new(|_: ServerEvent| {}));
        common::present(&mut s, vec![]);
        s.agent_request_control(id).unwrap();
        s.agent_type(id, "sleep 30").unwrap();
        s.agent_send_key(id, "ENTER").unwrap();
        s.set_mode(AgentMode::Copilot).unwrap();
    }
    // Wait for the sleep job to start before delivering Ctrl-C.
    std::thread::sleep(Duration::from_millis(150));
    {
        let mut s = session.lock();
        let id = s.status().agent_connections[0].conn_id;
        common::present(&mut s, vec!["sleep 30".into()]);
        s.agent_interrupt(id).unwrap();
    }
    // A terminal's signal handling may flush bytes queued with Ctrl-C. Wait for
    // the shell's fresh prompt before sending the human's follow-up command.
    wait_for(|| session.lock().screen().cursor_line().contains("conn-interrupt$"));
    session.lock().human_input(b"printf 'interrupt-result-%s\\n' $((40+2))\r");
    wait_for(|| session.lock().screen().rows().iter().any(|r| r.trim() == "interrupt-result-42"));
    engine.terminate().unwrap();
}

#[test]
fn human_input_denies_a_pending_approval_instead_of_editing_its_command() {
    let mut h = Harness::new();
    h.agent(1, "a");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "sudo ls").unwrap();
    let KeyResult::Pending { approval_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!("expected an approval") };
    h.session.human_input(b"x");
    // The reviewed text is gone before the human's byte lands: clear line, then "x".
    assert_eq!(h.pty_str(), "sudo ls\x15x");
    assert!(h.session.status().pending.is_empty(), "the approval must not outlive human input");
    assert_eq!(h.session.check_approval(&approval_id).unwrap().state, ApprovalState::Denied);
    // Approving late can no longer submit a line that was never reviewed.
    assert!(h.session.resolve_approval(&approval_id, Decision::Grant, "human").is_err());
    assert_eq!(h.pty_str(), "sudo ls\x15x", "nothing is submitted by a late approval");
}
