//! Handoff UX: copilot proposals, control gate with reasons, hand-back, session-allow revoke.
mod common;

use std::time::{Duration, Instant};

use common::{Harness, SessionExt};
use conn_core::affordance::{Actor, Affordance};
use conn_core::session::{AgentMode, ControlOutcome, ControlRequestState, KeyResult, ProposalState, ServerEvent, SessionError};

fn names(evs: &[ServerEvent]) -> Vec<String> {
    evs.iter().map(|e| serde_json::to_value(e).unwrap()["event"].as_str().unwrap().to_string()).collect()
}

#[test]
fn copilot_proposal_committed_by_human() {
    let mut h = Harness::new();
    let fe = h.frontend("ui");
    h.agent(1, "claude");
    h.session.set_mode(AgentMode::Copilot).unwrap();
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "kubectl get pods").unwrap();
    assert_eq!(h.pty_str(), "", "copilot typing never reaches the shell");
    let p = h.session.proposal().unwrap().clone();
    assert_eq!((p.text.as_str(), p.state), ("kubectl get pods", ProposalState::Drafting));
    h.session.agent_send_key(1, "BACKSPACE").unwrap();
    h.session.agent_type(1, "s -A").unwrap();
    let r = h.session.agent_send_key(1, "ENTER").unwrap();
    let KeyResult::Proposed { proposal_id, cmd } = r else { panic!("{r:?}") };
    assert_eq!(cmd, "kubectl get pods -A");
    assert!(matches!(h.session.agent_type(1, "x"), Err(SessionError::ProposalPending(_))));
    let r = h.session.accept_proposal(&proposal_id).unwrap();
    assert!(matches!(r, KeyResult::Executed { .. }));
    assert_eq!(h.pty_str(), "kubectl get pods -A\r");
    assert_eq!(h.session.proposal_state(&proposal_id), Some(ProposalState::Executed));
    let ev = h.audit_events();
    let exec = ev.iter().find(|e| e.action == "exec").unwrap();
    assert_eq!(exec.actor, "claude");
    assert_eq!(exec.fields["by"], "human_commit");
    let n = names(&fe.lock().unwrap());
    assert!(n.contains(&"proposal_changed".to_string()) && n.contains(&"proposal_resolved".to_string()));
}

#[test]
fn copilot_confirm_is_granted_by_commit_but_deny_still_blocks() {
    let mut h = Harness::new();
    h.agent(1, "claude");
    h.session.set_mode(AgentMode::Copilot).unwrap();
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "kubectl delete pod x").unwrap();
    let KeyResult::Proposed { proposal_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    assert!(matches!(h.session.accept_proposal(&proposal_id).unwrap(), KeyResult::Executed { .. }));
    assert!(h.session.status().pending.is_empty(), "no separate approval prompt");
    assert_eq!(h.pty_str(), "kubectl delete pod x\r");
    let exec = h.audit_events().into_iter().find(|e| e.action == "exec").unwrap();
    assert_eq!(exec.fields["policy"], "confirm:delete resource");

    h.clear_pty();
    h.session.agent_type(1, "rm -rf /").unwrap();
    let KeyResult::Proposed { proposal_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    assert!(matches!(h.session.accept_proposal(&proposal_id).unwrap(), KeyResult::Denied { .. }));
    assert_eq!(h.pty_str(), "", "a denied proposal is never typed");
}

#[test]
fn proposal_is_rejected_by_human_typing_or_esc() {
    let mut h = Harness::new();
    h.agent(1, "claude");
    h.session.set_mode(AgentMode::Copilot).unwrap();
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "ls").unwrap();
    let KeyResult::Proposed { proposal_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    h.session.reject_proposal(&proposal_id).unwrap();
    assert_eq!(h.session.proposal_state(&proposal_id), Some(ProposalState::Rejected));
    assert!(h.session.accept_proposal(&proposal_id).is_err());
    assert_eq!(h.pty_str(), "");
    // human typing while drafting
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "pwd").unwrap();
    let id = h.session.proposal().unwrap().proposal_id.clone();
    h.session.human_input(b"e").unwrap();
    assert_eq!(h.session.proposal_state(&id), Some(ProposalState::Rejected));
    assert!(h.session.controller().is_human());
    assert_eq!(h.pty_str(), "e");
}

#[test]
fn observe_mode_only_allows_snapshot() {
    let mut h = Harness::new();
    h.agent(1, "claude");
    h.session.agent_request_control(1).unwrap();
    h.session.set_mode(AgentMode::Observe).unwrap();
    assert!(h.session.controller().is_human(), "switching to observe revokes the lease");
    assert_eq!(h.session.affordances(Actor::Agent { conn: 1 }), vec![Affordance::Snapshot]);
    assert!(matches!(h.session.agent_request_control(1), Err(SessionError::WrongMode(AgentMode::Observe))));
    h.session.set_mode(AgentMode::Autopilot).unwrap();
    assert!(h.session.agent_request_control(1).is_ok());
}

#[test]
fn control_gate_holds_requests_for_the_human() {
    let mut h = Harness::new();
    let fe = h.frontend("ui");
    let agent_events = h.agent(1, "claude");
    h.session.set_control_gate(true);
    let ControlOutcome::Pending { request_id } = h.session.agent_request_control_with(1, Some("delete 3 crashloop pods".into())).unwrap() else { panic!() };
    assert!(h.session.controller().is_human());
    assert_eq!(h.session.control_request_state(&request_id), Some(ControlRequestState::Pending));
    let req = fe.lock().unwrap().iter().find_map(|e| match e { ServerEvent::ControlRequested { request } => Some(request.clone()), _ => None }).unwrap();
    assert_eq!(req.reason.as_deref(), Some("delete 3 crashloop pods"));
    // repeated request while pending returns the same id
    assert!(matches!(h.session.agent_request_control_with(1, None).unwrap(), ControlOutcome::Pending { request_id: ref r } if *r == request_id));
    h.session.decide_control(&request_id, true).unwrap();
    assert_eq!(h.session.current_lease().unwrap().conn, 1);
    assert!(names(&agent_events.lock().unwrap()).contains(&"control_request_resolved".to_string()));
    // granted event carries the reason
    assert!(fe.lock().unwrap().iter().any(|e| matches!(e, ServerEvent::ControlGranted { reason: Some(r), .. } if r == "delete 3 crashloop pods")));

    // deny path
    h.session.human_take();
    let ControlOutcome::Pending { request_id } = h.session.agent_request_control_with(1, None).unwrap() else { panic!() };
    h.session.decide_control(&request_id, false).unwrap();
    assert!(h.session.controller().is_human());
    assert_eq!(h.session.control_request_state(&request_id), Some(ControlRequestState::Denied));

    // expiry via tick
    let ControlOutcome::Pending { request_id } = h.session.agent_request_control_with(1, None).unwrap() else { panic!() };
    h.session.tick(Instant::now() + Duration::from_secs(400));
    assert_eq!(h.session.control_request_state(&request_id), Some(ControlRequestState::Expired));

    // gate off → immediate
    h.session.set_control_gate(false);
    assert!(matches!(h.session.agent_request_control_with(1, None).unwrap(), ControlOutcome::Granted { .. }));
}

#[test]
fn hand_back_returns_control_to_last_agent() {
    let mut h = Harness::new();
    let fe = h.frontend("ui");
    let agent_events = h.agent(1, "claude");
    assert!(h.session.hand_back().is_err(), "nobody to hand back to yet");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "echo hi").unwrap();
    h.session.agent_send_key(1, "ENTER").unwrap();
    h.session.human_input(b"x").unwrap();
    assert!(h.session.controller().is_human());
    let st = h.session.status();
    assert_eq!(st.last_agent.as_ref().unwrap().last_cmd.as_deref(), Some("echo hi"));
    let info = h.session.hand_back().unwrap();
    assert_eq!(info.agent_id, "claude");
    assert!(matches!(h.session.agent_type(1, "y"), Err(SessionError::InputPending)));
    h.session.agent_interrupt(1).unwrap();
    assert!(h.session.agent_type(1, "y").is_ok());
    assert!(agent_events.lock().unwrap().iter().any(|e| matches!(e, ServerEvent::ControlHandedBack { last_cmd: Some(c), .. } if c == "echo hi")));
    assert!(names(&fe.lock().unwrap()).contains(&"control_handed_back".to_string()));
    // disconnected agent cannot receive control
    h.session.connection_closed(1);
    assert!(matches!(h.session.hand_back(), Err(SessionError::NotFound(_))));
}

#[test]
fn session_allow_can_be_revoked() {
    let mut h = Harness::new();
    let fe = h.frontend("ui");
    h.agent(1, "claude");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "sudo ls").unwrap();
    let KeyResult::Pending { approval_id, .. } = h.session.agent_send_key(1, "ENTER").unwrap() else { panic!() };
    h.session.resolve_approval(&approval_id, conn_core::approval::Decision::AllowSession, "frontend").unwrap();
    assert_eq!(h.session.status().session_allows, vec!["privilege escalation".to_string()]);
    assert!(h.session.revoke_session_allow("privilege escalation"));
    assert!(!h.session.revoke_session_allow("privilege escalation"));
    assert!(h.session.status().session_allows.is_empty());
    h.session.agent_type(1, "sudo id").unwrap();
    assert!(matches!(h.session.agent_send_key(1, "ENTER").unwrap(), KeyResult::Pending { .. }));
    assert!(names(&fe.lock().unwrap()).contains(&"session_allows_changed".to_string()));
}

#[test]
fn raw_control_request_survives_denial_and_immediate_grant() {
    use serde_json::json;
    let mut h = Harness::new();
    let fe = h.frontend("ui");
    h.agent(1, "raw-check");
    h.session.set_control_gate(true);
    let params = json!({"reason":"inspect", "command":"cat -- 'a b.md'\n", "custom":{"flag":true}});
    let original = json!({"method":"request_control", "params":params});
    let ControlOutcome::Pending {request_id} = h.session.agent_request_control_original(1, params.clone()).unwrap() else {panic!()};
    assert!(h.session.agent_request_control_original(1, json!({"command":"different"})).is_err());
    h.session.decide_control(&request_id, false).unwrap();
    assert_eq!(h.pty_str(), "");
    let request = fe.lock().unwrap().iter().find_map(|e| match e {ServerEvent::ControlRequested {request} => Some(request.clone()), _=>None}).unwrap();
    assert_eq!(request.original_request, original);
    let audit = h.audit_events().into_iter().find(|e| e.action == "control_request_resolved").unwrap();
    assert_eq!(serde_json::to_value(audit).unwrap()["originalRequest"], original);
    assert!(h.session.agent_request_control_original(1, json!({"command":123})).is_err());
    assert!(h.session.agent_request_control_original(1, json!({"command":"x".repeat(65537)})).is_err());
    h.session.set_control_gate(false);
    h.session.agent_request_control_original(1, params.clone()).unwrap();
    assert!(fe.lock().unwrap().iter().any(|e| matches!(e, ServerEvent::ControlGranted {original_request:Some(r),..} if r["params"] == params)));
    assert_eq!(h.pty_str(), "", "metadata never types into the shell");
}
