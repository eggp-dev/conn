//! `conn serve`: headless backend driven entirely over the socket by a frontend.
#[cfg(unix)]
use std::process::{Command, Stdio};
#[cfg(unix)]
use std::time::{Duration, Instant};

#[cfg(unix)]
use base64::Engine as _;
use serde_json::json;
#[cfg(unix)]
use serde_json::Value;
use conn_core::ipc::Client;

#[cfg(unix)]
fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s)
}

#[cfg(unix)]
#[test]
fn frontend_drives_headless_backend() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("s.sock");
    let mut child = Command::new(env!("CARGO_BIN_EXE_conn"))
        .args(["--socket", sock.to_str().unwrap(), "--audit", dir.path().join("a.jsonl").to_str().unwrap(), "--policy", dir.path().join("p.yaml").to_str().unwrap(), "--enter-grace-ms", "300", "serve", "--rows", "20", "--cols", "90"])
        .env("SHELL", "/bin/sh")
        .env("HOME", dir.path())
        .env_remove("CONN")
        .current_dir(dir.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !sock.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }

    // frontend attaches with output streaming
    let fe = Client::connect(&sock).unwrap();
    fe.hello_frontend("test-ui", true).unwrap();
    let events = fe.take_events().unwrap();
    let mut screen = String::new();
    let mut ev_names: Vec<String> = vec![];
    let pump = |screen: &mut String, ev_names: &mut Vec<String>, ms: u64| {
        let end = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < end {
            if let Ok(ev) = events.recv_timeout(Duration::from_millis(20)) {
                let name = ev["event"].as_str().unwrap_or("").to_string();
                if name == "output" {
                    let bytes = base64::engine::general_purpose::STANDARD.decode(ev["data"].as_str().unwrap()).unwrap();
                    screen.push_str(&String::from_utf8_lossy(&bytes));
                } else {
                    ev_names.push(name);
                }
            }
        }
    };

    // human types through the frontend
    fe.call("input", json!({ "data": b64("echo fe-$((10+5))\n") })).unwrap();
    pump(&mut screen, &mut ev_names, 800);
    assert!(screen.contains("fe-15"), "{screen}");
    let st = fe.call("status", json!({})).unwrap();
    assert_eq!(st["size"]["cols"], 90);
    assert_eq!(st["connectedFrontends"][0], "test-ui");
    assert_eq!(st["pacing"]["enterGraceMs"], 300);
    assert_eq!(st["promptActive"], false);

    // resize through the frontend
    fe.call("resize", json!({ "rows": 30, "cols": 100 })).unwrap();
    fe.call("input", json!({ "data": b64("stty size\n") })).unwrap();
    pump(&mut screen, &mut ev_names, 800);
    assert!(screen.contains("30 100"), "{screen}");

    // agent joins; ENTER is scheduled (grace) and the transport waits for it
    let agent = Client::connect(&sock).unwrap();
    agent.hello("agent", "copilot").unwrap();
    agent.call("request_control", json!({})).unwrap();
    agent.call("type", json!({ "text": "echo agent-$((3*3))" })).unwrap();
    let t0 = Instant::now();
    let r = agent.call("send_key", json!({ "key": "ENTER", "intent": "test" })).unwrap();
    assert_eq!(r["status"], "executed", "{r}");
    assert!(t0.elapsed() >= Duration::from_millis(250), "grace window was awaited");
    pump(&mut screen, &mut ev_names, 800);
    assert!(screen.contains("agent-9"), "{screen}");
    for e in ["control_granted", "agent_input", "exec_scheduled", "agent_exec"] {
        assert!(ev_names.contains(&e.to_string()), "missing {e}: {ev_names:?}");
    }

    // frontend cancels a scheduled exec via the human path
    agent.call("type", json!({ "text": "echo never-$((1+1))" })).unwrap();
    let handle = std::thread::spawn({
        let sock = sock.clone();
        move || {
            // a human presses Ctrl-U during the grace window
            let c = Client::connect(&sock).unwrap();
            c.hello("human", "x").unwrap();
            std::thread::sleep(Duration::from_millis(100));
            c.call("input", json!({ "data": b64("\u{15}") })).unwrap();
        }
    });
    let r = agent.call("send_key", json!({ "key": "ENTER", "intent": "test" })).unwrap();
    handle.join().unwrap();
    assert_eq!(r["status"], "cancelled", "{r}");
    assert_eq!(r["reason"], "human_input");
    pump(&mut screen, &mut ev_names, 500);
    assert!(!screen.contains("never-2"), "{screen}");
    assert!(ev_names.contains(&"exec_cancelled".to_string()));
    assert!(ev_names.contains(&"control_revoked".to_string()));

    // frontend restricts agents to observation; agent loses request_control
    let r = fe.call("set_affordances", json!({ "allow": ["snapshot"] })).unwrap();
    assert_eq!(r["allow"], json!(["snapshot"]));
    let aff: Vec<String> = serde_json::from_value(agent.call("affordances", json!({})).unwrap()).unwrap();
    assert_eq!(aff, vec!["snapshot"]);
    let err = agent.call("request_control", json!({})).unwrap_err();
    assert!(err.to_string().starts_with("masked"), "{err}");
    fe.call("set_affordances", json!({ "allow": Value::Null })).unwrap();

    // frontend changes pacing at runtime
    let r = fe.call("set_pacing", json!({ "minWriteIntervalMs": 50, "enterGraceMs": 0 })).unwrap();
    assert_eq!(r["minWriteIntervalMs"], 50);
    assert_eq!(r["leaseTtlSecs"], 60);

    // shut down
    fe.call("input", json!({ "data": b64("exit\n") })).unwrap();
    let status = child.wait().unwrap();
    assert!(status.success(), "{status:?}");
    let deadline = Instant::now() + Duration::from_secs(3);
    while sock.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!sock.exists());
}

/// An agent bound to one tab must not follow the human to another tab.
#[test]
fn agent_connection_stays_bound_to_its_session() {
    use conn_core::ipc::Hub;
    let a = conn_core::session::Session::new(test_session_config());
    let b = conn_core::session::Session::new(test_session_config());
    let sa = std::sync::Arc::new(parking_lot::Mutex::new(a));
    let sb = std::sync::Arc::new(parking_lot::Mutex::new(b));
    let hub = Hub::new();
    hub.add("t1", sa.clone());
    hub.add("t2", sb.clone());
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("h.sock");
    let _guard = conn_core::ipc::serve_in_background(sock.clone(), hub.clone()).unwrap();

    let agent = Client::connect(&sock).unwrap();
    let h = agent.hello("agent", "claude").unwrap();
    assert_eq!(h["attended"], "t1");
    agent.call("request_control", json!({})).unwrap();
    assert!(sa.lock().current_lease().is_some());

    // human moves to t2
    hub.set_attended("t2");
    let err = agent.call("snapshot", json!({})).unwrap_err();
    assert!(err.to_string().starts_with("unattended"), "bound to t1, human away → {err}");
    let err = agent.call("type", json!({ "text": "x" })).unwrap_err();
    assert!(err.to_string().starts_with("suspended"), "{err}");
    // and it must not have touched t2
    assert!(sb.lock().current_lease().is_none());
    // knocking works, and the human coming back resumes
    agent.call("request_attention", json!({ "reason": "look" })).unwrap();
    assert_eq!(sa.lock().status().attention_request.unwrap().reason.as_deref(), Some("look"));
    hub.set_attended("t1");
    assert!(agent.call("snapshot", json!({})).is_ok());
    // explicit session still works for a fresh human client
    let cli = Client::connect(&sock).unwrap();
    cli.hello("human", "cli").unwrap();
    let st = cli.call("status", json!({ "session": "t2" })).unwrap();
    assert_eq!(st["attended"], false);
}

fn test_session_config() -> conn_core::session::SessionConfig {
    use conn_core::policy::{Policy, PolicyStore, EXAMPLE_POLICY};
    struct Sink;
    impl std::io::Write for Sink { fn write(&mut self, b: &[u8]) -> std::io::Result<usize> { Ok(b.len()) } fn flush(&mut self) -> std::io::Result<()> { Ok(()) } }
    conn_core::session::SessionConfig {
        rows: 24, cols: 80,
        audit: conn_core::audit::Audit::null(),
        policy: PolicyStore::from_policy(Policy::parse(&EXAMPLE_POLICY.replace("require_intent: true", "require_intent: false")).unwrap()),
        pty_writer: Box::new(Sink), output: None, master: None,
        pacing: conn_core::Pacing::default(), render_prompt: false, shell_pid: None,
    }
}

#[test]
fn agent_opens_a_tab_that_starts_unattended() {
    use conn_core::ipc::Hub;
    use std::sync::Arc;
    let hub = Hub::new();
    hub.add("t1", Arc::new(parking_lot::Mutex::new(conn_core::session::Session::new(test_session_config()))));
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("h.sock");
    let _guard = conn_core::ipc::serve_in_background(sock.clone(), hub.clone()).unwrap();

    // no opener → tab affordances are not offered and open_tab is unsupported
    let agent = Client::connect(&sock).unwrap();
    agent.hello("agent", "codex").unwrap();
    let aff = agent.call("affordances", json!({})).unwrap();
    assert!(!aff.to_string().contains("open_tab"), "{aff}");
    let err = agent.call("open_tab", json!({ "reason": "x" })).unwrap_err();
    assert!(err.to_string().contains("unsupported"), "{err}");

    // host installs an opener: spawns a session and registers it (what the app does)
    let seq = Arc::new(std::sync::atomic::AtomicU32::new(1));
    let opened: Arc<parking_lot::Mutex<Vec<(String, Option<String>)>>> = Default::default();
    {
        let hub2 = hub.clone();
        let opened = opened.clone();
        hub.set_opener(Arc::new(move |agent_id, reason| {
            let n = seq.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            let id = format!("t{n}");
            hub2.add(&id, Arc::new(parking_lot::Mutex::new(conn_core::session::Session::new(test_session_config()))));
            opened.lock().push((agent_id.to_string(), reason.map(String::from)));
            Ok(id)
        }));
    }
    let aff = agent.call("affordances", json!({})).unwrap();
    assert!(aff.to_string().contains("open_tab") && aff.to_string().contains("switch_tab"), "{aff}");

    let r = agent.call("open_tab", json!({ "reason": "build in a separate shell" })).unwrap();
    assert_eq!(r["session"], "t2");
    assert_eq!(r["tab"], 2);
    assert_eq!(r["attended"], false);
    assert_eq!(opened.lock()[0], ("codex".to_string(), Some("build in a separate shell".to_string())));

    // the new tab is unattended: nothing to see, nothing to write — it knocks instead
    let st = hub.get("t2").unwrap().lock().status();
    assert_eq!(st.opened_by.as_deref(), Some("codex"));
    assert_eq!(st.attention_request.unwrap().reason.as_deref(), Some("build in a separate shell"));
    let err = agent.call("snapshot", json!({})).unwrap_err();
    assert!(err.to_string().starts_with("unattended"), "bound to the new tab, human elsewhere → {err}");
    let err = agent.call("request_control", json!({})).unwrap_err();
    assert!(err.to_string().starts_with("unattended"), "{err}");
    let tabs = agent.call("list_tabs", json!({})).unwrap();
    assert_eq!(tabs["tabs"], 2);
    assert_eq!(tabs["sessions"][1]["current"], true);
    assert_eq!(tabs["sessions"][0]["attended"], true);
    assert_eq!(tabs["sessions"][1]["openedBy"], "codex");
    // while unattended the agent may still move back to where the human is
    let aff = agent.call("affordances", json!({})).unwrap();
    assert!(aff.to_string().contains("switch_tab") && !aff.to_string().contains("open_tab"), "{aff}");

    // the human comes to the new tab → the agent may work there
    hub.set_attended("t2");
    assert!(agent.call("snapshot", json!({})).is_ok());
    assert!(hub.get("t2").unwrap().lock().status().attention_request.is_none());

    // switching by number moves only this connection
    let r = agent.call("switch_tab", json!({ "tab": 1 })).unwrap();
    assert_eq!(r["session"], "t1");
    assert_eq!(r["attended"], false, "the human is still on tab 2");
    assert_eq!(hub.attended_id().as_deref(), Some("t2"));
    let err = agent.call("snapshot", json!({})).unwrap_err();
    assert!(err.to_string().starts_with("unattended"), "{err}");
    let err = agent.call("switch_tab", json!({ "tab": 9 })).unwrap_err();
    assert!(err.to_string().contains("not_found") || err.to_string().contains("no tab"), "{err}");

    // the frontend can veto the affordance
    hub.get("t1").unwrap().lock().set_affordance_mask(Some(["snapshot", "request_control", "type", "send_key", "switch_tab"].iter().map(|a| serde_json::from_value(json!(a)).unwrap()).collect()));
    hub.set_attended("t1");
    let err = agent.call("open_tab", json!({})).unwrap_err();
    assert!(err.to_string().starts_with("masked"), "{err}");
}


#[test]
fn agent_receives_modes_in_hello_tabs_snapshot_and_change_events() {
    use conn_core::session::{AgentMode, Session};
    use conn_core::ipc::Hub;
    use std::sync::Arc;
    let sa = Arc::new(parking_lot::Mutex::new(Session::new(test_session_config())));
    let hub = Hub::new(); hub.add("mode-tab", sa.clone());
    let dir = tempfile::tempdir().unwrap(); let sock = dir.path().join("mode.sock");
    let _guard = conn_core::ipc::serve_in_background(sock.clone(), hub).unwrap();
    let agent = Client::connect(&sock).unwrap();
    let hello = agent.hello("agent", "mode-reader").unwrap();
    assert_eq!(hello["session"], "mode-tab");
    assert_eq!(hello["mode"], "autopilot");
    assert_eq!(hello["effectiveMode"], "autopilot");
    let events = agent.take_events().unwrap();
    sa.lock().set_mode(AgentMode::Copilot).unwrap();
    let event = events.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
    assert_eq!(event["event"], "mode_changed");
    assert_eq!(event["mode"], "copilot"); assert_eq!(event["effectiveMode"], "copilot");
    let tabs = agent.call("list_tabs", json!({})).unwrap();
    assert_eq!(tabs["sessions"][0]["mode"], "copilot");
    assert_eq!(tabs["sessions"][0]["effectiveMode"], "copilot");
    let snapshot = agent.call("snapshot", json!({})).unwrap();
    assert_eq!(snapshot["mode"], "copilot"); assert_eq!(snapshot["effectiveMode"], "copilot");
}
