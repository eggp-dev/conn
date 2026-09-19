//! Exercise the actual MCP process and its dynamic tool catalog.
#[path = "../../core/tests/common/mod.rs"]
mod common;
use conn_core::{
    ipc::{self, Hub},
    session::SharedSession,
};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::{mpsc, Arc},
    time::Duration,
};
struct Mcp {
    child: Child,
    replies: mpsc::Receiver<Value>,
    id: u64,
}
impl Mcp {
    fn new(path: &std::path::Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_conn"))
            .args([
                "--socket",
                path.to_str().unwrap(),
                "mcp",
                "--tools",
                "dynamic",
                "--agent-id",
                "catalog-test",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let out = child.stdout.take().unwrap();
        let (tx, replies) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines() {
                let Ok(line) = line else { break };
                let v: Value = serde_json::from_str(&line).unwrap();
                if v.get("id").is_some() && tx.send(v).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            replies,
            id: 0,
        }
    }
    fn call(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        writeln!(
            self.child.stdin.as_mut().unwrap(),
            "{}",
            json!({"jsonrpc":"2.0","id":self.id,"method":method,"params":params})
        )
        .unwrap();
        let v = self
            .replies
            .recv_timeout(Duration::from_secs(5))
            .expect("MCP reply timeout");
        assert_eq!(v["id"], self.id);
        assert!(v.get("error").is_none(), "{v}");
        v["result"].clone()
    }
}
impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
#[test]
fn dynamic_catalog_keeps_authorized_recovery_after_source_loss() {
    for scenario in ["closed", "private", "removed", "exited"] {
        let old: SharedSession = Arc::new(common::Harness::headless().session.into());
        let target: SharedSession = Arc::new(common::Harness::headless().session.into());
        let hub = Hub::single("old", old.clone());
        hub.set_opener(Arc::new(|_, _| Err("unused".into())));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.sock");
        let _guard = ipc::serve_in_background(path.clone(), hub.clone()).unwrap();
        let mut m = Mcp::new(&path);
        m.call("initialize",json!({"protocolVersion":"2025-06-18","clientInfo":{"name":"audit","version":"1"},"capabilities":{}}));
        m.call("tools/list", json!({}));
        hub.add("target", target.clone());
        hub.set_attended("target");
        match scenario {
            "closed" => {
                hub.remove("old");
            }
            "private" => old.lock().set_shared(false).unwrap(),
            "removed" => old.lock().set_shared_with_agents(true, vec![999]).unwrap(),
            _ => old.lock().process_exited(Some(0)),
        }
        let tools = m.call("tools/list", json!({}));
        assert!(
            tools["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["name"] == "terminal_switch_tab"),
            "{scenario}: {tools}"
        );
        let switched = m.call(
            "tools/call",
            json!({"name":"terminal_switch_tab","arguments":{"tab":"target"}}),
        );
        assert_ne!(switched["isError"], true, "{scenario}: {switched}");
        common::present(&mut target.lock(), vec!["target visible".into()]);
        let snap = m.call(
            "tools/call",
            json!({"name":"terminal_snapshot","arguments":{}}),
        );
        assert_ne!(snap["isError"], true, "{scenario}: {snap}");
        target.lock().set_shared(false).unwrap();
        if scenario == "exited" {
            hub.remove("old");
        }
        let tools = m.call("tools/list", json!({}));
        assert!(
            !tools["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["name"] == "terminal_switch_tab"),
            "private destinations must not be offered"
        );
    }
}
#[test]
fn pending_admission_keeps_one_tool_that_succeeds_after_allow() {
    let tab: SharedSession = Arc::new(common::Harness::headless().session.into());
    common::present(&mut tab.lock(), vec!["admitted".into()]);
    let hub = Hub::single("tab", tab.clone());
    hub.set_admission_policy(conn_core::ipc::AdmissionPolicy::Ask);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mcp.sock");
    let _guard = ipc::serve_in_background(path.clone(), hub.clone()).unwrap();
    let mut m = Mcp::new(&path);
    m.call("initialize",json!({"protocolVersion":"2025-06-18","clientInfo":{"name":"audit","version":"1"},"capabilities":{}}));
    let tools = m.call("tools/list", json!({}));
    let names: Vec<&str> = tools["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"terminal_snapshot"), "a waiting agent needs a way to wait: {names:?}");
    assert!(!names.iter().any(|n| matches!(*n, "terminal_request_control" | "terminal_type" | "terminal_send_key" | "terminal_switch_tab")), "{names:?}");
    let early = m.call("tools/call", json!({"name":"terminal_list_tabs","arguments":{}}));
    assert!(early["isError"] == true && early.to_string().contains("admission_pending"), "{early}");
    let pending = hub.pending_admissions();
    assert_eq!(pending.len(), 1);
    let allow = { let hub = hub.clone(); let conn = pending[0].conn_id; std::thread::spawn(move || { std::thread::sleep(Duration::from_millis(200)); hub.decide_admission(conn, true) }) };
    let snap = m.call("tools/call", json!({"name":"terminal_snapshot","arguments":{}}));
    assert!(allow.join().unwrap());
    assert_ne!(snap["isError"], true, "{snap}");
    assert!(snap.to_string().contains("admitted"));
    let tools = m.call("tools/list", json!({}));
    assert!(tools["tools"].as_array().unwrap().iter().any(|t| t["name"] == "terminal_request_control"), "{tools}");
}
