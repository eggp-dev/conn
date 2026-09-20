#![cfg(unix)]
//! Real transport lifecycle regressions; unlike direct cleanup calls these exercise EOF.
mod common;
use common::{present, Harness};
use conn_core::ipc::{self, Client, Hub};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    sync::Arc,
    time::Duration,
};

#[test]
fn removed_participant_can_switch_to_another_allowed_tab() {
    let old = Arc::new(parking_lot::Mutex::new(Harness::new().session));
    let target = Arc::new(parking_lot::Mutex::new(Harness::new().session));
    let hub = Hub::single("old", old.clone());
    hub.set_opener(Arc::new(|_, _| Err("unused".into())));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.sock");
    let _guard = ipc::serve_in_background(path.clone(), hub.clone()).unwrap();
    let c = Client::connect(&path).unwrap();
    c.call("hello", json!({"kind":"agent","agentId":"audit"}))
        .unwrap();
    hub.add("target", target.clone());
    hub.set_attended("target");
    old.lock().set_shared_with_agents(true, vec![999]).unwrap();
    present(&mut target.lock(), vec!["target".into()]);
    assert_eq!(
        c.call("list_tabs", json!({})).unwrap()["sessions"][0]["id"],
        "target"
    );
    let result = c.call("switch_tab", json!({"tab":"target"}));
    assert!(
        result.is_ok(),
        "explicit switch to allowed target failed: {result:?}"
    );
}
#[test]
fn exited_shell_can_switch_to_another_allowed_tab() {
    let old = Arc::new(parking_lot::Mutex::new(Harness::new().session));
    let target = Arc::new(parking_lot::Mutex::new(Harness::new().session));
    let hub = Hub::single("old", old.clone());
    hub.set_opener(Arc::new(|_, _| Err("unused".into())));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.sock");
    let _guard = ipc::serve_in_background(path.clone(), hub.clone()).unwrap();
    let c = Client::connect(&path).unwrap();
    c.call("hello", json!({"kind":"agent","agentId":"audit"}))
        .unwrap();
    old.lock().process_exited(Some(0));
    hub.add("target", target.clone());
    hub.set_attended("target");
    present(&mut target.lock(), vec!["target".into()]);
    let result = c.call("switch_tab", json!({"tab":"target"}));
    assert!(
        result.is_ok(),
        "explicit switch from exited shell failed: {result:?}"
    );
}
fn send(s: &mut UnixStream, id: u64, method: &str, params: Value) {
    writeln!(s, "{}", json!({"id":id,"method":method,"params":params})).unwrap();
}
fn read_reply(r: &mut BufReader<UnixStream>, id: u64) -> Value {
    loop {
        let mut line = String::new();
        r.read_line(&mut line).unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        if v["id"] == id {
            return v;
        }
    }
}
#[test]
fn disconnected_pending_request_is_removed_promptly() {
    let s = Arc::new(parking_lot::Mutex::new(Harness::new().session));
    s.lock().set_control_gate(true);
    let hub = Hub::single("test", s.clone());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.sock");
    let _guard = ipc::serve_in_background(path.clone(), hub.clone()).unwrap();
    let mut socket = UnixStream::connect(&path).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reader = BufReader::new(socket.try_clone().unwrap());
    send(
        &mut socket,
        1,
        "hello",
        json!({"kind":"agent","agentId":"disconnect-audit"}),
    );
    read_reply(&mut reader, 1);
    present(&mut s.lock(), vec!["$".into()]);
    send(
        &mut socket,
        2,
        "request_control",
        json!({"reason":"audit pending disconnect","command":"pwd"}),
    );
    for _ in 0..100 {
        if !s.lock().status().control_requests.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(s.lock().status().control_requests.len(), 1);
    socket.shutdown(std::net::Shutdown::Both).unwrap();
    drop(reader);
    drop(socket);
    std::thread::sleep(Duration::from_secs(1));
    let agents = hub.agent_connections().len();
    let pending = s.lock().status().control_requests.len();
    // Resolve for orderly shutdown even if the assertion catches a leak.
    s.lock().set_shared(false).unwrap();
    assert_eq!(
        (agents, pending),
        (0, 0),
        "closed socket remains registered with a live approval card"
    );
}
#[test]
fn disconnected_agent_cannot_execute_a_scheduled_command() {
    let h = Harness::new();
    let pty = h.pty.clone();
    let s = Arc::new(parking_lot::Mutex::new(h.session));
    s.lock().set_pacing(conn_core::Pacing {
        enter_grace_ms: 3000,
        ..Default::default()
    });
    let hub = Hub::single("test", s.clone());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.sock");
    let _guard = ipc::serve_in_background(path.clone(), hub.clone()).unwrap();
    let mut socket = UnixStream::connect(&path).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reader = BufReader::new(socket.try_clone().unwrap());
    send(
        &mut socket,
        1,
        "hello",
        json!({"kind":"agent","agentId":"grace-disconnect"}),
    );
    read_reply(&mut reader, 1);
    present(&mut s.lock(), vec!["$".into()]);
    send(&mut socket, 2, "request_control", json!({}));
    assert!(read_reply(&mut reader, 2).get("error").is_none());
    send(
        &mut socket,
        3,
        "type",
        json!({"text":"echo disconnect-test"}),
    );
    assert!(read_reply(&mut reader, 3).get("error").is_none());
    send(
        &mut socket,
        4,
        "send_key",
        json!({"key":"ENTER","intent":"harmless audit"}),
    );
    for _ in 0..100 {
        if s.lock().scheduled_exec().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(s.lock().scheduled_exec().is_some());
    socket.shutdown(std::net::Shutdown::Both).unwrap();
    drop(reader);
    drop(socket);
    std::thread::sleep(Duration::from_millis(500));
    // Keep owner presentation fresh while advancing the normal engine deadline.
    present(&mut s.lock(), vec!["$ echo disconnect-test".into()]);
    for _ in 0..350 {
        present(&mut s.lock(), vec!["$ echo disconnect-test".into()]);
        s.lock().tick(std::time::Instant::now());
        std::thread::sleep(Duration::from_millis(10));
    }
    let bytes = pty.lock().unwrap().clone();
    s.lock().set_shared(false).unwrap();
    assert!(
        !bytes.contains(&b'\r'),
        "ENTER was delivered after socket disconnect: {bytes:?}"
    );
}

#[test]
fn idle_disconnect_removes_connection() {
    let s = Arc::new(parking_lot::Mutex::new(Harness::new().session));
    let hub = Hub::single("test", s);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.sock");
    let _guard = ipc::serve_in_background(path.clone(), hub.clone()).unwrap();
    let mut socket = UnixStream::connect(&path).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reader = BufReader::new(socket.try_clone().unwrap());
    send(
        &mut socket,
        1,
        "hello",
        json!({"kind":"agent","agentId":"idle-disconnect"}),
    );
    read_reply(&mut reader, 1);
    socket.shutdown(std::net::Shutdown::Both).unwrap();
    drop(reader);
    drop(socket);
    for _ in 0..100 {
        if hub.agent_connections().is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(hub.agent_connections().is_empty());
}

#[test]
fn navigation_catalog_recovers_without_leaking_or_granting_access() {
    use conn_core::affordance::Affordance;
    let old = Arc::new(parking_lot::Mutex::new(Harness::new().session));
    let target = Arc::new(parking_lot::Mutex::new(Harness::new().session));
    let hub = Hub::single("old", old.clone());
    hub.set_opener(Arc::new(|_, _| Err("unused".into())));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.sock");
    let _guard = ipc::serve_in_background(path.clone(), hub.clone()).unwrap();
    let c = Client::connect(&path).unwrap();
    c.call("hello", json!({"kind":"agent","agentId":"audit"}))
        .unwrap();
    hub.add("target", target.clone());
    hub.set_attended("target");
    hub.remove("old");
    assert!(c.call("snapshot", json!({})).is_err());
    assert_eq!(
        c.call("navigation_affordances", json!({})).unwrap(),
        json!(["switch_tab"])
    );
    assert_eq!(c.call("list_tabs", json!({})).unwrap()["current"], "old");
    target
        .lock()
        .set_affordance_mask(Some([Affordance::Snapshot].into_iter().collect()));
    assert_eq!(
        c.call("navigation_affordances", json!({})).unwrap(),
        json!([])
    );
    assert!(c.call("switch_tab", json!({"tab":"target"})).is_err());
    target.lock().set_affordance_mask(None);
    target
        .lock()
        .set_shared_with_agents(true, vec![999])
        .unwrap();
    assert_eq!(
        c.call("navigation_affordances", json!({})).unwrap(),
        json!([])
    );
    assert!(c.call("switch_tab", json!({"tab":"target"})).is_err());
    target.lock().set_shared(false).unwrap();
    assert_eq!(
        c.call("navigation_affordances", json!({})).unwrap(),
        json!([])
    );
}
#[test]
fn disconnect_rejects_a_waiting_copilot_proposal() {
    use conn_core::session::{AgentMode, ProposalState};
    let h = Harness::new();
    let pty = h.pty.clone();
    let s = Arc::new(parking_lot::Mutex::new(h.session));
    s.lock().set_mode(AgentMode::Copilot).unwrap();
    let hub = Hub::single("test", s.clone());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.sock");
    let _guard = ipc::serve_in_background(path.clone(), hub.clone()).unwrap();
    let mut socket = UnixStream::connect(&path).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reader = BufReader::new(socket.try_clone().unwrap());
    send(
        &mut socket,
        1,
        "hello",
        json!({"kind":"agent","agentId":"copilot-disconnect"}),
    );
    read_reply(&mut reader, 1);
    present(&mut s.lock(), vec!["$".into()]);
    send(&mut socket, 2, "request_control", json!({}));
    assert!(read_reply(&mut reader, 2).get("error").is_none());
    send(&mut socket, 3, "type", json!({"text":"echo proposal"}));
    assert!(read_reply(&mut reader, 3).get("error").is_none());
    send(
        &mut socket,
        4,
        "send_key",
        json!({"key":"ENTER","intent":"test"}),
    );
    std::thread::sleep(Duration::from_millis(150));
    let id = s.lock().proposal().unwrap().proposal_id.clone();
    assert_eq!(s.lock().proposal_state(&id), Some(ProposalState::Ready));
    socket.shutdown(std::net::Shutdown::Both).unwrap();
    drop(reader);
    drop(socket);
    for _ in 0..100 {
        if hub.agent_connections().is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(s.lock().proposal_state(&id), Some(ProposalState::Rejected));
    assert!(s.lock().current_lease().is_none());
    assert!(pty.lock().unwrap().is_empty());
}
#[test]
fn half_closed_pipeline_drains_reads_and_rejects_writes() {
    let s = Arc::new(parking_lot::Mutex::new(Harness::new().session));
    present(&mut s.lock(), vec!["$".into()]);
    let hub = Hub::single("test", s.clone());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.sock");
    let _guard = ipc::serve_in_background(path.clone(), hub.clone()).unwrap();
    let mut socket = UnixStream::connect(&path).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    // A one-shot client: everything is queued before its write side closes.
    send(&mut socket, 1, "hello", json!({"kind":"agent","agentId":"one-shot"}));
    send(&mut socket, 2, "snapshot", json!({}));
    send(&mut socket, 3, "request_control", json!({"reason":"late"}));
    send(&mut socket, 4, "type", json!({"text":"LATE_WRITE"}));
    send(&mut socket, 5, "list_tabs", json!({}));
    socket.shutdown(std::net::Shutdown::Write).unwrap();
    let replies: Vec<Value> = BufReader::new(socket)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| serde_json::from_str::<Value>(&l).ok())
        .filter(|v| v.get("id").is_some())
        .collect();
    let by_id = |id: u64| replies.iter().find(|v| v["id"] == id).unwrap_or_else(|| panic!("no reply for {id}: {replies:?}"));
    for id in [1, 2, 5] {
        assert!(by_id(id)["error"].is_null(), "read {id} must be answered: {replies:?}");
    }
    // Whether a write ran before the server observed EOF is a legitimate race;
    // it must still be answered, and nothing may stay held afterwards.
    for id in [3, 4] {
        let reply = by_id(id);
        let code = reply["error"]["code"].as_str().unwrap_or_default();
        assert!(reply["error"].is_null() || matches!(code, "connection_closing" | "not_controller"), "write {id}: {replies:?}");
    }
    assert!(matches!(s.lock().status().controller, conn_core::session::ControllerInfo::Human));
    assert_eq!(hub.agent_connections().len(), 0);
}
