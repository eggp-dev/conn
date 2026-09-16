//! Private external sessions retain native rendering while excluding public clients.
mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use common::{Buf, SharedBuf, VecSink};
use conn_core::affordance::{Actor, Affordance};
use conn_core::audit::{Audit, Event};
use conn_core::ipc::{self, Client, Hub};
use conn_core::policy::{Policy, PolicyStore, EXAMPLE_POLICY};
use conn_core::session::{ConnKind, ServerEvent};
use conn_core::{Session, SessionConfig, SharedSession};
use serde_json::json;

fn policy() -> PolicyStore { PolicyStore::from_policy(Policy::parse(EXAMPLE_POLICY).unwrap()) }

fn fixture(private: bool) -> (SharedSession, Buf, Arc<Mutex<Vec<Event>>>) {
    let (audit, records) = Audit::memory();
    let bytes: Buf = Default::default();
    let cfg = SessionConfig {
        rows: 24, cols: 80, audit, policy: policy(),
        pty_writer: Box::new(SharedBuf(bytes.clone())), output: None,
        master: None, pacing: Default::default(), render_prompt: false, shell_pid: None,
    };
    let s = if private { Session::new_external_private(cfg) } else { Session::new(cfg) };
    (Arc::new(parking_lot::Mutex::new(s)), bytes, records)
}

#[test]
fn private_input_is_untracked_and_human_takeover_permanently_revokes_external_writes() {
    let (session, bytes, records) = fixture(true);
    let events = Arc::new(Mutex::new(Vec::new()));
    let trace = Arc::new(Mutex::new(Vec::new()));
    let mut s = session.lock();
    s.subscribe("native", Box::new(VecSink(events.clone())), true);
    s.set_trace(trace.clone());
    s.write_external(b"external-fixture").unwrap();
    assert!(s.write_external(&[b'x'; 1025]).is_err());
    s.human_input(b"human-fixture\r");
    assert!(!s.external_writer_active());
    assert!(s.write_external(b"must-not-arrive").is_err());
    assert_eq!(&*bytes.lock().unwrap(), b"external-fixturehuman-fixture\r");
    assert!(s.input_line().is_empty());
    s.pty_output(b"private output");
    s.tick(Instant::now());
    s.process_exited(Some(0));
    assert!(records.lock().unwrap().is_empty());
    assert!(trace.lock().unwrap().is_empty());
    assert!(events.lock().unwrap().iter().any(|e| matches!(e, ServerEvent::Output { .. })));
    assert!(!events.lock().unwrap().iter().any(|e| matches!(e, ServerEvent::HumanExec { .. } | ServerEvent::AgentExec { .. })));
}

#[test]
fn explicit_take_revokes_without_activity_records() {
    let (session, _, records) = fixture(true);
    let mut s = session.lock();
    assert!(s.external_writer_active());
    s.human_take();
    assert!(!s.external_writer_active());
    assert!(s.write_external(b"no").is_err());
    assert!(records.lock().unwrap().is_empty());
}

#[test]
fn terminal_protocol_responses_do_not_impersonate_human_takeover_or_restore_access() {
    let (session, bytes, records) = fixture(true);
    let mut s = session.lock();
    s.write_terminal_response(b"\x1b[1;1R").unwrap();
    assert!(s.external_writer_active());
    assert!(s.input_line().is_empty());
    s.human_input(b"x");
    assert!(!s.external_writer_active());
    s.write_terminal_response(b"\x1b[0n").unwrap();
    assert!(!s.external_writer_active());
    assert!(s.write_external(b"late").is_err());
    assert_eq!(&*bytes.lock().unwrap(), b"\x1b[1;1Rx\x1b[0n");
    assert!(records.lock().unwrap().is_empty());
    drop(s);
    assert!(ipc::dispatch(&session, 0, "terminal_response", &json!({"data":"x"})).is_err());
    assert!(fixture(false).0.lock().write_terminal_response(b"x").is_err());
}

#[test]
fn private_sessions_reject_agents_and_public_dispatch_even_with_frontend_identity() {
    let (session, _, records) = fixture(true);
    let denied = Arc::new(Mutex::new(Vec::new()));
    {
        let mut s = session.lock();
        s.register_conn(1, ConnKind::Agent, "agent", Box::new(VecSink(denied.clone())));
        s.register_frontend(2, "socket-ui", Box::new(VecSink(denied.clone())), true);
        s.pty_output(b"synthetic-secret");
        s.tick(Instant::now());
        assert!(s.conn_kind(1).is_none());
        assert!(s.conn_kind(2).is_none());
        assert!(s.agent_request_control(1).is_err());
        assert!(s.agent_request_attention(1, None).is_err());
        assert!(s.agent_type(1, "whoami").is_err());
        assert!(s.agent_send_key(1, "ENTER").is_err());
        assert!(s.agent_interrupt(1).is_err());
        assert!(s.agent_can(1, Affordance::OpenTab).is_err());
        assert!(s.snapshot(Actor::Agent { conn: 1 }).is_err());
        assert!(s.affordances(Actor::Agent { conn: 1 }).is_empty());
        assert!(s.status().connected_agents.is_empty());
        assert!(s.status().external_private);
    }
    for method in ["snapshot", "status", "input", "resize", "affordances", "analyse", "request_control", "set_mode", "unknown"] {
        let err = ipc::dispatch(&session, 2, method, &json!({})).unwrap_err();
        assert_eq!(err.code, "not_found");
        assert_eq!(err.message, "session unavailable");
    }
    assert!(ipc::dispatch_trusted(&session, 0, "snapshot", &json!({})).is_ok());
    assert!(denied.lock().unwrap().is_empty());
    assert!(records.lock().unwrap().is_empty());
}

#[test]
fn public_socket_hides_private_tabs_for_every_client_kind_and_blocks_explicit_ids() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("private-test.sock");
    let hub = Hub::new();
    let (private, _, records) = fixture(true);
    let (public, _, _) = fixture(false);
    hub.add("hidden-native-session", private.clone());
    hub.add("visible-session", public.clone());
    hub.set_attended("visible-session");
    let guard = ipc::serve_in_background(path.clone(), hub.clone()).unwrap();
    for kind in ["agent", "human", "frontend"] {
        let client = Client::connect(&path).unwrap();
        let events = client.take_events().unwrap();
        let hello = client.call("hello", json!({ "kind": kind, "name":"fixture", "streamOutput":true })).unwrap();
        assert!(!hello.to_string().contains("hidden-native-session"));
        let tabs = client.call("list_tabs", json!({})).unwrap();
        assert_eq!(tabs["tabs"], 1);
        assert_eq!(tabs["sessions"][0]["id"], "visible-session");
        assert_eq!(tabs["sessions"][0]["tab"], 1);
        assert!(!tabs.to_string().contains("hidden-native-session"));
        for method in ["snapshot", "status", "input", "resize", "set_attended", "request_control", "affordances"] {
            let error = client.call(method, json!({"session":"hidden-native-session"})).unwrap_err().to_string();
            assert!(error.contains("session unavailable"), "{method}: {error}");
            assert!(!error.contains("hidden-native-session"));
        }
        let error = client.call("switch_tab", json!({"tab":"hidden-native-session"})).unwrap_err().to_string();
        assert!(error.contains("session unavailable"));
        assert!(client.call("hello", json!({"kind":kind,"session":"hidden-native-session"})).is_err());
        private.lock().pty_output(b"synthetic-secret-output");
        private.lock().tick(Instant::now());
        std::thread::sleep(Duration::from_millis(25));
        for event in events.try_iter() {
            assert_ne!(event["session"], "hidden-native-session");
            assert!(!event.to_string().contains("synthetic-secret-output"));
        }
    }
    hub.set_attended("hidden-native-session");
    let client = Client::connect(&path).unwrap();
    let hello = client.hello("agent", "fixture").unwrap();
    assert!(hello["session"].is_null());
    assert!(hello["attended"].is_null());
    assert!(client.call("snapshot", json!({})).is_err());
    assert!(private.lock().status().connected_agents.is_empty());
    assert!(records.lock().unwrap().is_empty());
    drop(client);
    drop(guard);
}

#[cfg(unix)]
#[test]
fn direct_startup_preserves_argv_cwd_and_env_without_logging_hidden_input_or_starting_a_shell_after_exit() {
    use conn_core::{Engine, EngineConfig, LaunchSpec};
    let dir = tempfile::tempdir().unwrap();
    let mut profile = conn_core::backend::Profile::local("private-fixture".into(), "/bin/sh".into());
    profile.cwd = Some(dir.path().to_string_lossy().into_owned());
    profile.env.insert("FIXTURE_ENV".into(), "profile-value".into());
    let (audit, records) = Audit::memory();
    let output: Buf = Default::default();
    let script = r#"stty -echo; printf 'ready\n'; IFS= read -r answer; printf '%s\n%s\n%s\n' "$1" "$FIXTURE_ENV" "$answer" > result; exit 7"#;
    let engine = Engine::spawn(EngineConfig {
        external_private: true,
        launch: LaunchSpec::Program { executable: "/bin/sh".into(), argv: vec!["-c".into(), script.into(), "fixture".into(), "literal $value ; * with spaces".into()] },
        profile: Some(profile), policy: Some(policy()), audit: Some(audit),
        output: Some(Box::new(SharedBuf(output.clone()))), ..EngineConfig::default()
    }).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while !String::from_utf8_lossy(&output.lock().unwrap()).contains("ready") {
        assert!(Instant::now() < until, "startup program did not become ready");
        std::thread::sleep(Duration::from_millis(10));
    }
    let events = Arc::new(Mutex::new(Vec::new()));
    engine.subscribe("native", Box::new(VecSink(events.clone())), false);
    engine.write_input(b"synthetic-hidden-value\r");
    let until = Instant::now() + Duration::from_secs(5);
    while !engine.has_exited() {
        assert!(Instant::now() < until, "startup did not exit");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(engine.wait(), Some(7));
    assert_eq!(std::fs::read_to_string(dir.path().join("result")).unwrap(), "literal $value ; * with spaces\nprofile-value\nsynthetic-hidden-value\n");
    assert!(!String::from_utf8_lossy(&output.lock().unwrap()).contains("synthetic-hidden-value"));
    assert!(engine.session().lock().input_line().is_empty());
    assert!(records.lock().unwrap().is_empty());
    assert!(!events.lock().unwrap().iter().any(|e| matches!(e, ServerEvent::HumanExec { .. } | ServerEvent::AgentExec { .. })));
    assert!(engine.write_external(b"late").is_err());
}

#[cfg(unix)]
#[test]
fn private_spawn_failure_is_generic_and_does_not_record_the_program() {
    use conn_core::{Engine, EngineConfig, LaunchSpec};
    let (audit, records) = Audit::memory();
    let result = Engine::spawn(EngineConfig {
        external_private: true,
        launch: LaunchSpec::Program { executable: "/no-such-external-fixture".into(), argv: vec!["synthetic-secret".into()] },
        shell: Some("/bin/sh".into()), audit: Some(audit), policy: Some(policy()), ..EngineConfig::default()
    });
    let Err(error) = result else { panic!("invalid executable started") };
    assert_eq!(error.to_string(), "External session could not start");
    assert!(records.lock().unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn external_backpressure_fails_closed_without_blocking_human_takeover() {
    use conn_core::{Engine, EngineConfig, LaunchSpec};
    let output: Buf = Default::default();
    let engine = Engine::spawn(EngineConfig {
        external_private: true,
        launch: LaunchSpec::Program { executable: "/bin/sh".into(), argv: vec!["-c".into(), "stty raw -echo; printf ready; sleep 30".into()] },
        shell: Some("/bin/sh".into()), policy: Some(policy()),
        output: Some(Box::new(SharedBuf(output.clone()))), ..EngineConfig::default()
    }).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !String::from_utf8_lossy(&output.lock().unwrap()).contains("ready") {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let began = Instant::now();
    let mut stopped = false;
    for _ in 0..4096 {
        if engine.write_external(&[b'x'; 1024]).is_err() { stopped = true; break; }
    }
    assert!(stopped, "PTY backpressure must revoke rather than queue indefinitely");
    engine.session().lock().human_take();
    assert!(began.elapsed() < Duration::from_secs(2));
    assert!(!engine.session().lock().external_writer_active());
    engine.terminate().unwrap();
}
