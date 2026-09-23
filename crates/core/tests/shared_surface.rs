//! Shared-session observation is independent of desktop and tab focus.
mod common;
use common::{Harness, SessionExt, present};
use conn_core::{affordance::Actor, session::{SessionError, ConnKind}, ipc::{self,Hub,Client}};
use serde_json::json;
use std::sync::Arc;

#[test]
fn background_screen_contains_output_only_and_no_scrollback() {
    let mut h=Harness::new(); h.agent(1,"a"); h.session.set_attended(false);
    h.session.human_input(b"UN_ECHOED_PASSWORD\r").unwrap();
    h.session.pty_output(b"User: demo\r\nPassword: ******");
    let snap=h.session.snapshot(Actor::Agent{conn:1}).unwrap();
    assert!(snap.projection.screen.join("\n").contains("Password: ******"));
    assert!(!serde_json::to_string(&snap).unwrap().contains("UN_ECHOED_PASSWORD"));
    h.session.pty_output(b"\x1b[2J\x1b[Hnew screen");
    assert!(!h.session.snapshot(Actor::Agent{conn:1}).unwrap().projection.screen.join("\n").contains("demo"));
}

#[test]
fn sharing_keeps_pty_and_requires_selected_connection_immediately() {
    let mut h=Harness::new(); h.agent(1,"selected"); h.agent(2,"other");
    h.session.set_shared(false).unwrap();
    h.session.human_input(b"authenticated\r").unwrap();
    h.session.set_shared_with_agents(true,vec![1]).unwrap();
    assert_eq!(h.pty_str(),"authenticated\r","sharing never restarts or clears the shell");
    assert!(h.session.snapshot(Actor::Agent{conn:1}).is_ok());
    present(&mut h.session,vec!["authenticated shell".into()]);
    assert!(h.session.snapshot(Actor::Agent{conn:1}).is_ok());
    assert!(h.session.snapshot(Actor::Agent{conn:2}).is_err());
    assert!(h.session.agent_request_control(2).is_err());
    h.session.agent_request_control(1).unwrap();
    h.session.set_shared(false).unwrap();
    assert!(h.session.current_lease().is_none());
    assert!(h.session.agent_type(1,"leak").is_err());
    h.session.human_input(b"human continues\r").unwrap();
    assert!(h.pty_str().ends_with("human continues\r"));
}

#[test]
fn physical_input_is_not_forgotten_by_sharing_transition() {
    let mut h=Harness::new(); h.agent(1,"a");
    h.session.set_shared(false).unwrap(); h.session.human_input(b"unfinished").unwrap();
    let generation=h.session.surface_generation();let audit_len=h.audit_events().len();
    assert!(matches!(h.session.set_shared_with_agents(true,vec![1]),Err(SessionError::InputPending)));
    assert!(h.session.is_private());assert_eq!(h.session.surface_generation(),generation);
    assert_eq!(h.audit_events().len(),audit_len);assert_eq!(h.pty_str(),"unfinished");
    h.session.human_input(b"\x03").unwrap();
    h.session.set_shared_with_agents(true,vec![1]).unwrap();present(&mut h.session,vec!["$".into()]);
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1,"typed by agent").unwrap();
    h.session.set_shared(false).unwrap();
    assert!(h.session.status().input_pending);
    assert!(h.session.current_lease().is_none());
    assert!(h.session.snapshot(Actor::Agent{conn:1}).is_err());
    assert!(h.pty_str().ends_with("typed by agent"), "revocation leaves physical input with its owner");
    assert!(matches!(h.session.set_shared_with_agents(true,vec![1]),Err(SessionError::InputPending)));
    assert!(h.session.is_private());
    h.session.human_input(b"\x03").unwrap();
    assert!(!h.session.status().input_pending);
    h.session.set_shared_with_agents(true,vec![1]).unwrap();
    h.session.set_shared(false).unwrap();
}

#[test]
fn persistent_private_clients_refresh_tools_on_selection_removal_and_stop() {
    use std::{sync::mpsc::Receiver,time::{Duration,Instant}};
    fn catalog_event(events: &Receiver<serde_json::Value>) {
        let deadline=Instant::now()+Duration::from_secs(2);
        loop {
            let event=events.recv_timeout(deadline.saturating_duration_since(Instant::now())).expect("catalog invalidation");
            if event.get("session").is_none() {
                assert_eq!(event,json!({"event":"tools_changed"}));return;
            }
        }
    }
    let mut h=Harness::new();h.session.set_shared(false).unwrap();
    let s=Arc::new(parking_lot::Mutex::new(h.session));let hub=Hub::single("private-session",s.clone());
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("catalog.sock");
    let _guard=ipc::serve_in_background(path.clone(),hub).unwrap();
    let a=Client::connect(&path).unwrap();let b=Client::connect(&path).unwrap();
    let a_id=a.call("hello",json!({"kind":"agent","agentId":"same-name"})).unwrap()["conn"].as_u64().unwrap();
    let b_id=b.call("hello",json!({"kind":"agent","agentId":"same-name"})).unwrap()["conn"].as_u64().unwrap();
    let a_events=a.take_events().unwrap();let b_events=b.take_events().unwrap();
    assert!(s.lock().conn_kind(a_id).is_none(),"private candidates have no session subscription");
    assert!(a.call("affordances",json!({})).is_err());
    s.lock().set_shared_with_agents(true,vec![a_id]).unwrap();
    catalog_event(&a_events);
    assert!(b_events.recv_timeout(Duration::from_millis(100)).is_err(),"unselected same-name client learns nothing");
    present(&mut s.lock(),vec!["shared screen".into()]);
    assert!(a.call("affordances",json!({})).unwrap().as_array().unwrap().iter().any(|v|v=="snapshot"));
    assert!(b.call("snapshot",json!({"session":"private-session"})).is_err());
    s.lock().set_shared_with_agents(true,vec![b_id]).unwrap();
    catalog_event(&a_events);catalog_event(&b_events);
    assert!(a.call("affordances",json!({})).is_err());
    present(&mut s.lock(),vec!["now only b".into()]);
    assert!(b.call("snapshot",json!({})).is_ok());
    s.lock().set_shared(false).unwrap();
    catalog_event(&b_events);
    assert!(b.call("affordances",json!({})).is_err());
    assert_eq!(b.call("sessions",json!({})).unwrap()["sessions"],json!([]));
}

#[test]
fn untrusted_frontend_registration_and_owner_dispatch_are_rejected() {
    let mut h=Harness::new(); h.agent(1,"a");
    h.session.register_frontend(99,"spoof",Box::new(common::VecSink(Default::default())));
    assert_ne!(h.session.conn_kind(99),Some(ConnKind::Frontend));
    let s=Arc::new(parking_lot::Mutex::new(h.session));
    for method in ["input","resize","set_shared","publish_surface","set_mode","approve","decide_control","set_attended"] {
        assert_eq!(ipc::dispatch(&s,1,method,&json!({})).unwrap_err().code,"owner_required", "{method}");
    }
    assert_eq!(ipc::dispatch(&s,99,"snapshot",&json!({})).unwrap_err().code,"owner_required");
}

#[test]
fn socket_identity_cannot_escalate_and_private_candidates_do_not_disclose_screen() {
    let mut h=Harness::new(); h.session.set_shared(false).unwrap();
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
fn explicit_switch_recovers_from_a_closed_or_private_binding() {
    for close_old in [true,false] {
        let old=Arc::new(parking_lot::Mutex::new(Harness::new().session));
        let target=Arc::new(parking_lot::Mutex::new(Harness::new().session));
        let hub=Hub::single("old",old.clone());
        hub.set_opener(Arc::new(|_,_|Err("not needed".into())));
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("switch.sock");
        let _guard=ipc::serve_in_background(path.clone(),hub.clone()).unwrap();
        let client=Client::connect(&path).unwrap();
        client.call("hello",json!({"kind":"agent","agentId":"switch-test"})).unwrap();
        hub.add("target",target.clone());hub.set_attended("target");
        if close_old { hub.remove("old"); } else { old.lock().set_shared(false).unwrap(); }
        present(&mut target.lock(),vec!["target visible".into()]);
        assert!(client.call("snapshot",json!({})).is_err(),"never silently follow the human");
        let switched=client.call("switch_tab",json!({"tab":"target"})).unwrap();
        assert_eq!(switched["session"],"target");
        let tabs=client.call("list_tabs",json!({})).unwrap();
        assert_eq!(tabs["current"],"target","closed={close_old}: success must update the persistent binding");
        assert_eq!(tabs["sessions"][0]["current"],true);
        let snapshot=client.call("snapshot",json!({})).unwrap();
        assert_eq!(snapshot["screen"][0],"target visible");
        // Idempotent switches and subsequent discovery calls must keep the binding.
        client.call("switch_tab",json!({"tab":1})).unwrap();
        assert_eq!(client.call("list_tabs",json!({})).unwrap()["current"],"target");
        target.lock().set_shared(false).unwrap();
        assert!(client.call("switch_tab",json!({"tab":"target"})).is_err());
        assert!(client.call("snapshot",json!({})).is_err());
    }
}

#[test]
fn private_activity_is_not_recorded_and_history_is_not_backfilled() {
    let mut h=Harness::new(); h.session.set_shared(false).unwrap();
    let before=h.audit_events().len();
    h.session.set_control_gate(true); h.session.set_attended(false);h.session.set_attended(true);
    h.session.human_input(b"PRIVATE_TEST_INPUT\r").unwrap();
    assert_eq!(h.audit_events().len(),before);
    h.session.set_shared_with_agents(true,vec![]).unwrap();
    assert_eq!(h.audit_events().last().unwrap().action,"sharing_started");
    assert!(!format!("{:?}",h.audit_events()).contains("PRIVATE_TEST_INPUT"));
}

#[test]
fn background_approval_survives_tab_switch_but_requires_human_decision() {
    use conn_core::{session::KeyResult,approval::{Decision,ApprovalState}};
    let mut h=Harness::new();h.agent(1,"a");h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1,"sudo ls").unwrap();
    let KeyResult::Pending{approval_id,..}=h.session.agent_send_key(1,"ENTER").unwrap() else {panic!()};
    h.session.set_attended(false);
    assert_eq!(h.session.check_approval(&approval_id).unwrap().state,ApprovalState::Pending);
    assert_eq!(h.pty_str(),"sudo ls");
    h.session.resolve_approval(&approval_id,Decision::Grant,"human").unwrap();
    assert_eq!(h.pty_str(),"sudo ls\r");
}

#[test]
fn background_grace_executes_but_human_input_still_cancels() {
    use conn_core::{session::{KeyResult,ExecState},Pacing};use std::time::{Duration,Instant};
    for takeover in [false,true] {
        let mut h=Harness::new();h.agent(1,"a");
        h.session.set_pacing(Pacing{enter_grace_ms:100,..Pacing::default()});
        h.session.agent_request_control(1).unwrap();h.session.agent_type(1,"echo safe").unwrap();
        let KeyResult::Scheduled{exec_id,..}=h.session.agent_send_key(1,"ENTER").unwrap() else {panic!()};
        h.session.set_attended(false);
        if takeover {h.session.human_input(b"\x03").unwrap();}
        h.session.tick(Instant::now()+Duration::from_millis(150));
        assert_eq!(h.session.exec_state(&exec_id).unwrap().0,if takeover {ExecState::Cancelled} else {ExecState::Executed});
        assert_eq!(h.pty_str(),if takeover {"echo safe\x03"} else {"echo safe\r"});
    }
}

#[test]
fn idle_screen_needs_no_renderer_heartbeat() {
    let mut h=Harness::new();h.agent(1,"a");h.session.agent_request_control(1).unwrap();
    h.session.set_attended(false);
    std::thread::sleep(std::time::Duration::from_millis(3050));
    assert!(h.session.snapshot(Actor::Agent{conn:1}).is_ok());
    h.session.agent_type(1,"late").unwrap();assert_eq!(h.pty_str(),"late");
}

#[test]
fn external_origin_can_share_without_restoring_external_writer_or_startup_history() {
    use conn_core::{Session,SessionConfig, audit::Audit,policy::{Policy,PolicyStore},Pacing};
    let output:common::Buf=Default::default();let (audit,log)=Audit::memory();
    let mut s=Session::new_external_private(SessionConfig{
        rows:24,cols:80,audit,policy:PolicyStore::from_policy(Policy::allow_all()),pty_writer:Box::new(common::SharedBuf(output.clone())),
        master:None,pacing:Pacing::default(),shell_pid:None,
    });
    s.write_external(b"SYNTHETIC_TEST_PASSWORD").unwrap();
    let generation=s.surface_generation();
    assert!(matches!(s.set_shared_with_agents(true,vec![1]),Err(SessionError::InputPending)));
    assert!(s.is_private());assert!(s.external_writer_active());assert_eq!(s.surface_generation(),generation);
    assert!(log.lock().unwrap().is_empty());
    s.write_external(b"\r").unwrap();
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
    let mut h=Harness::new();h.agent(1,"a");h.agent(2,"b");h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1,"sudo ls").unwrap();
    let KeyResult::Pending{approval_id,..}=h.session.agent_send_key(1,"ENTER").unwrap() else {panic!()};
    let session=Arc::new(parking_lot::Mutex::new(h.session));
    assert!(ipc::dispatch(&session,2,"check_approval",&json!({"approvalId":approval_id})).is_err());
    assert_eq!(ipc::dispatch(&session,1,"check_approval",&json!({"approvalId":approval_id})).unwrap()["cmd"],"sudo ls");
}

#[test]
fn public_discovery_and_hello_do_not_reveal_unselected_shared_session() {
    let mut h=Harness::new();h.session.set_shared(false).unwrap();
    let s=Arc::new(parking_lot::Mutex::new(h.session));let hub=Hub::single("selected-only",s.clone());
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("selected.sock");let _guard=ipc::serve_in_background(path.clone(),hub).unwrap();
    let selected=Client::connect(&path).unwrap();let id=selected.hello("agent","chosen").unwrap()["conn"].as_u64().unwrap();
    s.lock().set_shared_with_agents(true,vec![id]).unwrap();present(&mut s.lock(),vec!["authenticated".into()]);
    let other=Client::connect(&path).unwrap();let hello=other.hello("agent","other").unwrap();
    assert!(hello["session"].is_null());assert!(hello["attended"].is_null());
    assert_eq!(other.call("list_tabs",json!({})).unwrap()["sessions"],json!([]));
    assert_eq!(selected.call("list_tabs",json!({})).unwrap()["sessions"][0]["id"],"selected-only");
    assert!(other.call("snapshot",json!({"session":"selected-only"})).is_err());
    assert_eq!(selected.call("snapshot",json!({"session":"selected-only"})).unwrap()["screen"][0],json!("authenticated"));
}

#[test]
fn explicit_hello_session_is_respected_without_moving_humans_view() {
    let a=Harness::new();let b=Harness::new();
    let sa=Arc::new(parking_lot::Mutex::new(a.session));let sb=Arc::new(parking_lot::Mutex::new(b.session));
    let hub=Hub::new();hub.add("a",sa);hub.add("b",sb);
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("binding.sock");let _guard=ipc::serve_in_background(path.clone(),hub.clone()).unwrap();
    let client=Client::connect(&path).unwrap();let hello=client.call("hello",json!({"kind":"agent","agentId":"a","session":"b"})).unwrap();
    assert_eq!(hello["session"],"b");assert_eq!(hub.attended_id().as_deref(),Some("a"));
    assert!(client.call("snapshot",json!({})).is_ok());
}

#[test]
fn output_is_immediately_observable_without_renderer_acknowledgement() {
    let mut h=Harness::new();h.agent(1,"a");
    h.session.agent_request_control(1).unwrap();h.session.agent_type(1,"echo ready").unwrap();
    h.session.pty_output(b"$ echo ready");h.session.set_attended(false);
    assert!(h.session.status().surface_available);
    assert_eq!(h.session.snapshot(Actor::Agent{conn:1}).unwrap().projection.screen[0],"$ echo ready");
    h.session.agent_send_key(1,"ENTER").unwrap();assert_eq!(h.pty_str(),"echo ready\r");
}

#[test]
fn agent_cannot_submit_a_cursor_line_with_hidden_text() {
    // The resolved command is echoed to the agent and may come from the raw cursor row.
    use conn_core::session::KeyResult;
    let mut h=Harness::new();h.agent(1,"a");h.session.agent_request_control(1).unwrap();
    h.session.pty_output(b"\x1b[2J\x1b[Htoken: \x1b[8mHIDDEN_TOKEN\x1b[28m");
    assert!(!h.session.snapshot(Actor::Agent{conn:1}).unwrap().projection.screen.join("\n").contains("HIDDEN_TOKEN"));
    let err=h.session.agent_send_key(1,"ENTER").unwrap_err();
    assert!(err.to_string().contains("concealed"),"{err}");
    assert!(!h.pty_str().contains('\r'),"nothing may be submitted");
    // Once the hidden text is gone the same agent continues normally.
    h.session.pty_output(b"\r\x1b[2K$ ");
    h.session.agent_type(1,"echo ok").unwrap();
    assert!(matches!(h.session.agent_send_key(1,"ENTER").unwrap(),KeyResult::Executed{..}|KeyResult::Scheduled{..}));
}
