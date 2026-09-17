//! The agent sees owner-presented frames, never the independent policy VT buffer.
mod common;
use common::{Harness, present};
use conn_core::{affordance::Actor, screen::{SurfaceFrame,Cursor}, session::{SessionError, ConnKind}, ipc::{self,Hub,Client}};
use serde_json::json;
use std::sync::Arc;

fn frame(h: &Harness, lines: &[&str]) -> SurfaceFrame {
    SurfaceFrame {surface_id:"test-owner".into(),generation:h.session.surface_generation(),revision:h.session.output_seq()+2,
        output_seq:h.session.output_seq(),rows:24,cols:80,cursor:Some(Cursor{row:0,col:0}),
        screen:lines.iter().map(|s|s.to_string()).collect(),alternate_screen:false,visible:true,image:None,image_unavailable:true}
}

#[test]
fn presented_rows_win_over_raw_output_and_no_owner_means_no_snapshot() {
    let mut h=Harness::headless(); h.agent(1,"a");
    h.session.pty_output(b"INTERNAL_PRIVATE_BUFFER");
    assert!(matches!(h.session.snapshot(Actor::Agent{conn:1}),Err(SessionError::SurfaceUnavailable)));
    let shown=frame(&h,&["Password: ******"]); h.session.publish_surface(shown).unwrap();
    let snapshot=h.session.snapshot(Actor::Agent{conn:1}).unwrap();
    assert_eq!(snapshot.projection.screen,["Password: ******"]);
    assert!(!serde_json::to_string(&snapshot).unwrap().contains("INTERNAL_PRIVATE_BUFFER"));
    h.session.invalidate_surface();
    assert!(h.session.snapshot(Actor::Human).is_err(),"native helpers cannot fall back to VT either");
}

#[test]
fn output_sequence_generation_and_cursor_validation_are_fail_closed() {
    let mut h=Harness::headless(); h.agent(1,"a");
    let stale=frame(&h,&["before"]); h.session.pty_output(b"after");
    assert!(h.session.publish_surface(stale).is_err());
    let old=frame(&h,&["after"]); h.session.invalidate_surface();
    assert!(h.session.publish_surface(old).is_err());
    let mut bad=frame(&h,&["out"]); bad.cursor=Some(Cursor{row:24,col:0});
    assert!(h.session.publish_surface(bad).is_err());
    let mut hidden=frame(&h,&["out"]); hidden.visible=false;
    h.session.publish_surface(hidden).unwrap();
    assert!(h.session.snapshot(Actor::Agent{conn:1}).is_err());
}

#[test]
fn sharing_keeps_pty_and_requires_selected_connection_plus_new_frame() {
    let mut h=Harness::headless(); h.agent(1,"selected"); h.agent(2,"other");
    h.session.set_shared(false).unwrap();
    h.session.human_input(b"authenticated\r");
    h.session.set_shared_with_agents(true,vec![1]).unwrap();
    assert_eq!(h.pty_str(),"authenticated\r","sharing never restarts or clears the shell");
    assert!(h.session.snapshot(Actor::Agent{conn:1}).is_err());
    present(&mut h.session,vec!["authenticated shell".into()]);
    assert!(h.session.snapshot(Actor::Agent{conn:1}).is_ok());
    assert!(h.session.snapshot(Actor::Agent{conn:2}).is_err());
    assert!(h.session.agent_request_control(2).is_err());
    h.session.agent_request_control(1).unwrap();
    h.session.set_shared(false).unwrap();
    assert!(h.session.current_lease().is_none());
    assert!(h.session.agent_type(1,"leak").is_err());
    h.session.human_input(b"human continues\r");
    assert!(h.pty_str().ends_with("human continues\r"));
}

#[test]
fn physical_input_is_not_forgotten_by_sharing_transition() {
    let mut h=Harness::headless(); h.agent(1,"a");
    h.session.set_shared(false).unwrap(); h.session.human_input(b"unfinished");
    h.session.set_shared_with_agents(true,vec![1]).unwrap(); present(&mut h.session,vec!["unfinished".into()]);
    h.session.agent_request_control(1).unwrap();
    assert!(matches!(h.session.agent_type(1,"append"),Err(SessionError::InputPending)));
    h.session.human_input(b"\x15"); h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1,"typed by agent").unwrap();
    h.session.set_shared(false).unwrap();
    assert!(h.session.status().input_pending);
    assert!(h.session.current_lease().is_none());
    assert!(h.session.snapshot(Actor::Agent{conn:1}).is_err());
    assert!(h.pty_str().ends_with("typed by agent"), "revocation leaves physical input with its owner");
    h.session.set_shared_with_agents(true,vec![1]).unwrap();present(&mut h.session,vec!["typed by agent".into()]);
    h.session.agent_request_control(1).unwrap();
    assert!(matches!(h.session.agent_type(1,"unsafe append"),Err(SessionError::InputPending)));
    h.session.human_input(b"\x15");
    assert!(!h.session.status().input_pending);
    h.session.set_shared(false).unwrap();
}

#[test]
fn untrusted_frontend_registration_and_owner_dispatch_are_rejected() {
    let mut h=Harness::headless(); h.agent(1,"a");
    h.session.register_frontend(99,"spoof",Box::new(common::VecSink(Default::default())),true);
    assert_ne!(h.session.conn_kind(99),Some(ConnKind::Frontend));
    let s=Arc::new(parking_lot::Mutex::new(h.session));
    for method in ["input","resize","set_shared","publish_surface","set_mode","approve","decide_control","set_attended"] {
        assert_eq!(ipc::dispatch(&s,1,method,&json!({})).unwrap_err().code,"owner_required", "{method}");
    }
    assert_eq!(ipc::dispatch(&s,99,"snapshot",&json!({})).unwrap_err().code,"owner_required");
}

#[test]
fn socket_identity_cannot_escalate_and_private_candidates_do_not_disclose_screen() {
    let mut h=Harness::headless(); h.session.set_shared(false).unwrap();
    let s=Arc::new(parking_lot::Mutex::new(h.session)); let hub=Hub::single("secret",s.clone());
    let dir=tempfile::tempdir().unwrap(); let path=dir.path().join("surface.sock");
    let _guard=ipc::serve_in_background(path.clone(),hub.clone()).unwrap();
    let client=Client::connect(&path).unwrap();
    assert!(client.call("status",json!({})).is_err());
    assert!(client.call("hello",json!({"kind":"frontend","streamOutput":true})).is_err());
    let hello=client.call("hello",json!({"kind":"agent","agentId":"candidate"})).unwrap();
    assert_eq!(hello["protocolVersion"],2);
    assert_eq!(hub.agent_connections()[0].agent_id,"candidate");
    assert_eq!(client.call("sessions",json!({})).unwrap()["sessions"],json!([]));
    assert!(client.call("snapshot",json!({"session":"secret"})).is_err());
    assert!(client.call("set_attended",json!({"session":"secret"})).is_err());
    assert!(client.call("hello",json!({"kind":"human"})).is_err());
}

#[test]
fn completion_acceptance_is_bound_to_frame_and_cannot_execute() {
    let mut h=Harness::headless();
    let shown=frame(&h,&["$ echo"]);let revision=shown.revision;let generation=shown.generation;
    h.session.publish_surface(shown).unwrap();
    assert!(h.session.accept_completion("test-owner",generation,revision,"hello\r").is_err());
    h.session.accept_completion("test-owner",generation,revision,"hello").unwrap();
    assert_eq!(h.pty_str(),"hello");
    h.session.invalidate_surface();
    assert!(h.session.accept_completion("test-owner",generation,revision,"stale").is_err());
}

#[test]
fn private_activity_is_not_recorded_and_history_is_not_backfilled() {
    let mut h=Harness::headless(); h.session.set_shared(false).unwrap();
    let before=h.audit_events().len();
    h.session.set_control_gate(true); h.session.set_attended(false);h.session.set_attended(true);
    h.session.human_input(b"PRIVATE_TEST_INPUT\r");
    assert_eq!(h.audit_events().len(),before);
    h.session.set_shared_with_agents(true,vec![]).unwrap();
    assert_eq!(h.audit_events().last().unwrap().action,"sharing_started");
    assert!(!format!("{:?}",h.audit_events()).contains("PRIVATE_TEST_INPUT"));
}

#[test]
fn obscuring_surface_preserves_decision_but_cannot_execute_without_fresh_frame() {
    use conn_core::{session::KeyResult,approval::{Decision,ApprovalState}};
    let mut h=Harness::headless();h.agent(1,"a");h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1,"sudo ls").unwrap();
    let KeyResult::Pending{approval_id,..}=h.session.agent_send_key(1,"ENTER").unwrap() else {panic!()};
    h.session.invalidate_surface();
    assert_eq!(h.session.check_approval(&approval_id).unwrap().state,ApprovalState::Pending);
    assert!(matches!(h.session.resolve_approval(&approval_id,Decision::Grant,"human"),Err(SessionError::SurfaceUnavailable)));
    assert_eq!(h.pty_str(),"sudo ls");
    present(&mut h.session,vec!["sudo ls".into()]);
    h.session.resolve_approval(&approval_id,Decision::Grant,"human").unwrap();
    assert_eq!(h.pty_str(),"sudo ls\r");
}

#[test]
fn hidden_attention_cancels_pending_decisions_instead_of_resuming_them() {
    use conn_core::{session::KeyResult,approval::{Decision,ApprovalState}};
    let mut h=Harness::headless();h.agent(1,"a");h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1,"sudo ls").unwrap();
    let KeyResult::Pending{approval_id,..}=h.session.agent_send_key(1,"ENTER").unwrap() else {panic!()};
    h.session.set_attended(false);
    assert_eq!(h.session.check_approval(&approval_id).unwrap().state,ApprovalState::Denied);
    h.session.set_attended(true);present(&mut h.session,vec![]);
    assert!(h.session.resolve_approval(&approval_id,Decision::Grant,"human").is_err());
    assert!(h.session.current_lease().is_none());
    assert!(!h.pty_str().contains('\r'));
}

#[test]
fn layout_changes_do_not_cancel_grace_but_execution_waits_for_presentation() {
    use conn_core::{session::{KeyResult,ExecState},Pacing};use std::time::{Duration,Instant};
    let mut h=Harness::headless();h.agent(1,"a");
    h.session.set_pacing(Pacing{enter_grace_ms:100,..Pacing::default()});
    h.session.agent_request_control(1).unwrap();h.session.agent_type(1,"echo safe").unwrap();
    let KeyResult::Scheduled{exec_id,..}=h.session.agent_send_key(1,"ENTER").unwrap() else {panic!()};
    h.session.invalidate_surface();h.session.tick(Instant::now()+Duration::from_millis(150));
    assert_eq!(h.session.exec_state(&exec_id).unwrap().0,ExecState::Scheduled);
    assert_eq!(h.pty_str(),"echo safe");
    present(&mut h.session,vec!["echo safe".into()]);h.session.tick(Instant::now()+Duration::from_millis(200));
    assert_eq!(h.session.exec_state(&exec_id).unwrap().0,ExecState::Executed);
    assert_eq!(h.pty_str(),"echo safe\r");
}

#[test]
fn absent_surface_eventually_cancels_grace_and_keeps_pending_input_tracked() {
    use conn_core::{session::{KeyResult,ExecState},Pacing};use std::time::{Duration,Instant};
    let mut h=Harness::headless();h.agent(1,"a");h.session.set_pacing(Pacing{enter_grace_ms:100,..Pacing::default()});
    h.session.agent_request_control(1).unwrap();h.session.agent_type(1,"echo safe").unwrap();
    let KeyResult::Scheduled{exec_id,..}=h.session.agent_send_key(1,"ENTER").unwrap() else {panic!()};
    h.session.invalidate_surface();h.session.tick(Instant::now()+Duration::from_secs(4));
    assert_eq!(h.session.exec_state(&exec_id).unwrap().0,ExecState::Cancelled);
    assert_eq!(h.pty_str(),"echo safe");
    h.session.set_shared(false).unwrap();
    assert!(h.session.status().input_pending);
    assert!(h.session.snapshot(Actor::Agent{conn:1}).is_err());
}

#[test]
fn stale_presented_heartbeat_cannot_authorize_observation_or_writes() {
    let mut h=Harness::headless();h.agent(1,"a");h.session.agent_request_control(1).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(3050));
    assert!(h.session.snapshot(Actor::Agent{conn:1}).is_err());
    assert!(h.session.agent_type(1,"late").is_err());
    assert!(h.pty_bytes().is_empty());
}

#[test]
fn external_origin_can_share_without_restoring_external_writer_or_startup_history() {
    use conn_core::{Session,SessionConfig, audit::Audit,policy::{Policy,PolicyStore},Pacing};
    let output:common::Buf=Default::default();let (audit,log)=Audit::memory();
    let mut s=Session::new_external_private(SessionConfig{
        rows:24,cols:80,audit,policy:PolicyStore::from_policy(Policy::allow_all()),pty_writer:Box::new(common::SharedBuf(output.clone())),
        output:None,master:None,pacing:Pacing::default(),render_prompt:false,shell_pid:None,
    });
    s.write_external(b"SYNTHETIC_TEST_PASSWORD\r").unwrap();
    assert!(log.lock().unwrap().is_empty());
    s.set_shared_with_agents(true,vec![1]).unwrap();
    assert!(s.external_origin());assert!(!s.is_private());assert!(!s.external_writer_active());
    assert!(s.write_external(b"late input").is_err());
    s.register_conn(1,ConnKind::Agent,"selected",Box::new(|_|{}));
    present(&mut s,vec!["User: demo-user".into(),"Password: ******".into(),"Connected".into()]);
    let snap=s.snapshot(Actor::Agent{conn:1}).unwrap();
    assert!(snap.projection.screen.join("\n").contains("******"));
    assert!(!serde_json::to_string(&snap).unwrap().contains("SYNTHETIC_TEST_PASSWORD"));
    assert!(!format!("{:?}",log.lock().unwrap()).contains("SYNTHETIC_TEST_PASSWORD"));
    assert!(s.status().review_required,"external origin never inherits ordinary local policy after sharing");
}

#[test]
fn public_agent_cannot_read_another_agents_approval_command() {
    use conn_core::session::KeyResult;
    let mut h=Harness::headless();h.agent(1,"a");h.agent(2,"b");h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1,"sudo ls").unwrap();
    let KeyResult::Pending{approval_id,..}=h.session.agent_send_key(1,"ENTER").unwrap() else {panic!()};
    let session=Arc::new(parking_lot::Mutex::new(h.session));
    assert!(ipc::dispatch(&session,2,"check_approval",&json!({"approvalId":approval_id})).is_err());
    assert_eq!(ipc::dispatch(&session,1,"check_approval",&json!({"approvalId":approval_id})).unwrap()["cmd"],"sudo ls");
}

#[test]
fn public_discovery_and_hello_do_not_reveal_unselected_shared_session() {
    let mut h=Harness::headless();h.session.set_shared(false).unwrap();
    let s=Arc::new(parking_lot::Mutex::new(h.session));let hub=Hub::single("selected-only",s.clone());
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("selected.sock");let _guard=ipc::serve_in_background(path.clone(),hub).unwrap();
    let selected=Client::connect(&path).unwrap();let id=selected.hello("agent","chosen").unwrap()["conn"].as_u64().unwrap();
    s.lock().set_shared_with_agents(true,vec![id]).unwrap();present(&mut s.lock(),vec!["authenticated".into()]);
    let other=Client::connect(&path).unwrap();let hello=other.hello("agent","other").unwrap();
    assert!(hello["session"].is_null());assert!(hello["attended"].is_null());
    assert_eq!(other.call("list_tabs",json!({})).unwrap()["sessions"],json!([]));
    assert_eq!(selected.call("list_tabs",json!({})).unwrap()["sessions"][0]["id"],"selected-only");
    assert!(other.call("snapshot",json!({"session":"selected-only"})).is_err());
    assert_eq!(selected.call("snapshot",json!({"session":"selected-only"})).unwrap()["screen"],json!(["authenticated"]));
}

#[test]
fn explicit_hello_session_is_respected_without_moving_humans_view() {
    let a=Harness::headless();let b=Harness::headless();
    let sa=Arc::new(parking_lot::Mutex::new(a.session));let sb=Arc::new(parking_lot::Mutex::new(b.session));
    let hub=Hub::new();hub.add("a",sa);hub.add("b",sb);
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("binding.sock");let _guard=ipc::serve_in_background(path.clone(),hub.clone()).unwrap();
    let client=Client::connect(&path).unwrap();let hello=client.call("hello",json!({"kind":"agent","agentId":"a","session":"b"})).unwrap();
    assert_eq!(hello["session"],"b");assert_eq!(hub.attended_id().as_deref(),Some("a"));
    assert!(client.call("snapshot",json!({})).is_err());
}

#[test]
fn unseen_output_blocks_enter_interrupt_and_new_control_until_presented() {
    let mut h=Harness::headless();h.agent(1,"a");h.agent(2,"b");
    h.session.agent_request_control(1).unwrap();h.session.agent_type(1,"echo ready").unwrap();
    h.session.pty_output(b"$ echo ready");
    assert!(!h.session.status().surface_available);
    assert!(matches!(h.session.agent_send_key(1,"ENTER"),Err(SessionError::SurfaceUnavailable)));
    assert!(matches!(h.session.agent_interrupt(1),Err(SessionError::SurfaceUnavailable)));
    assert!(matches!(h.session.agent_request_control(2),Err(SessionError::SurfaceUnavailable)));
    assert_eq!(h.pty_str(),"echo ready");
    present(&mut h.session,vec!["$ echo ready".into()]);
    h.session.agent_send_key(1,"ENTER").unwrap();
    assert_eq!(h.pty_str(),"echo ready\r");
}

#[test]
fn socket_enter_waits_for_owner_to_present_the_typed_echo() {
    let h=Harness::headless();let pty=h.pty.clone();
    let s=Arc::new(parking_lot::Mutex::new(h.session));let hub=Hub::single("shell",s.clone());
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("render-wait.sock");
    let _guard=ipc::serve_in_background(path.clone(),hub).unwrap();
    let client=Client::connect(&path).unwrap();client.hello("agent","render-wait").unwrap();
    client.call("request_control",json!({"session":"shell"})).unwrap();
    client.call("type",json!({"session":"shell","text":"echo rendered"})).unwrap();
    s.lock().pty_output(b"$ echo rendered");
    let (tx,rx)=std::sync::mpsc::channel();
    let worker=std::thread::spawn(move||tx.send(client.call("send_key",json!({"session":"shell","key":"ENTER"}))).unwrap());
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(rx.try_recv().is_err(),"ENTER waits instead of using unseen output");
    assert_eq!(pty.lock().unwrap().as_slice(),b"echo rendered");
    present(&mut s.lock(),vec!["$ echo rendered".into()]);
    let result=rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap().unwrap();
    assert_eq!(result["status"],"executed");
    worker.join().unwrap();
    assert_eq!(pty.lock().unwrap().as_slice(),b"echo rendered\r");
}

#[test]
fn same_revision_is_an_immutable_frame_while_identical_heartbeats_are_allowed() {
    let mut h=Harness::headless();h.agent(1,"a");
    let baseline=frame(&h,&["$ visible"]);
    h.session.publish_surface(baseline.clone()).unwrap();
    h.session.publish_surface(baseline.clone()).unwrap();
    let mut mutations=Vec::new();
    let mut changed=baseline.clone();changed.screen=vec!["different text".into()];mutations.push(changed);
    let mut changed=baseline.clone();changed.cursor=Some(Cursor{row:1,col:1});mutations.push(changed);
    let mut changed=baseline.clone();changed.image=Some(conn_core::screen::SurfaceImage{mime_type:"image/png".into(),data:"c3ludGhldGlj".into()});mutations.push(changed);
    let mut changed=baseline.clone();changed.alternate_screen=true;mutations.push(changed);
    for changed in mutations {
        assert!(h.session.publish_surface(changed).is_err());
        assert_eq!(h.session.authoritative_surface().unwrap(),baseline);
    }
    let mut next=baseline.clone();next.revision+=1;next.screen=vec!["$ newer".into()];
    h.session.publish_surface(next.clone()).unwrap();
    assert!(h.session.accept_completion(&baseline.surface_id,baseline.generation,baseline.revision," stale").is_err());
    assert_eq!(h.session.authoritative_surface().unwrap(),next);
}
