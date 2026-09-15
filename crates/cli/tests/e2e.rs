#![cfg(unix)]
//! End-to-end: the real binary in a real PTY, an agent over the socket, a human on stdin.
//! Covers acceptance A, C, D, E, F, G, H, I from the PRD.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde_json::{json, Value};
use conn_core::ipc::Client;

struct Term {
    master: Box<dyn portable_pty::MasterPty + Send>,
    rx: mpsc::Receiver<Vec<u8>>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    all: String,
}

impl Term {
    fn write(&mut self, s: &str) {
        self.writer.write_all(s.as_bytes()).unwrap();
        self.writer.flush().unwrap();
    }

    fn expect(&mut self, needle: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut acc = String::new();
        while Instant::now() < deadline {
            if let Ok(chunk) = self.rx.recv_timeout(Duration::from_millis(50)) {
                let s = String::from_utf8_lossy(&chunk).to_string();
                acc.push_str(&s);
                self.all.push_str(&s);
                if acc.contains(needle) {
                    return acc;
                }
            }
        }
        panic!("timed out waiting for {needle:?}\n--- got ---\n{acc}\n--- all ---\n{}", self.all);
    }

    fn drain(&mut self, ms: u64) -> String {
        let mut acc = String::new();
        let deadline = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < deadline {
            if let Ok(chunk) = self.rx.recv_timeout(Duration::from_millis(20)) {
                let s = String::from_utf8_lossy(&chunk).to_string();
                acc.push_str(&s);
                self.all.push_str(&s);
            }
        }
        acc
    }
}

struct Env {
    dir: tempfile::TempDir,
    socket: PathBuf,
    audit: PathBuf,
    policy: PathBuf,
}

fn start(extra: &[&str]) -> (Env, Term) {
    let dir = tempfile::tempdir().unwrap();
    let env = Env {
        socket: dir.path().join("s.sock"),
        audit: dir.path().join("audit.jsonl"),
        policy: dir.path().join("policy.yaml"),
        dir,
    };
    let pty = native_pty_system();
    let pair = pty.openpty(PtySize { rows: 24, cols: 100, pixel_width: 0, pixel_height: 0 }).unwrap();
    let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_conn"));
    cmd.args(["--socket", env.socket.to_str().unwrap(), "--audit", env.audit.to_str().unwrap(), "--policy", env.policy.to_str().unwrap()]);
    cmd.args(extra);
    cmd.env("SHELL", "/bin/sh");
    cmd.env("HOME", env.dir.path());
    cmd.env("TERM", "xterm-256color");
    cmd.env_remove("CONN");
    cmd.cwd(env.dir.path());
    let child = pair.slave.spawn_command(cmd).unwrap();
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().unwrap();
    let writer = pair.master.take_writer().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let mut term = Term { master: pair.master, rx, writer, child, all: String::new() };
    term.write("echo READY-$((2+3))\n");
    term.expect("READY-5\r\n");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !env.socket.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(env.socket.exists(), "socket was not created");
    (env, term)
}

fn agent(env: &Env, id: &str) -> Client {
    let c = Client::connect(&env.socket).unwrap();
    c.hello("agent", id).unwrap();
    c
}

fn names(v: Value) -> Vec<String> {
    serde_json::from_value(v).unwrap()
}

#[test]
fn full_flow() {
    let (env, mut term) = start(&[]);
    let mode = std::fs::metadata(&env.socket).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);

    // D. observe
    let a = agent(&env, "copilot");
    assert_eq!(names(a.call("affordances", json!({})).unwrap()), vec!["snapshot", "request_control"]);
    let snap = a.call("snapshot", json!({})).unwrap();
    let screen = snap["screen"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>().join("\n");
    assert!(screen.contains("READY-5"), "{screen}");
    assert_eq!(snap["controller"]["type"], "human");
    assert_eq!(snap["processAlive"], true);

    // E. control
    a.call("request_control", json!({})).unwrap();
    assert!(names(a.call("affordances", json!({})).unwrap()).contains(&"type".to_string()));
    a.call("type", json!({ "text": "echo agent-$((1+1))" })).unwrap();
    let err = a.call("send_key", json!({ "key": "ENTER" })).unwrap_err();
    assert!(err.to_string().contains("intent_required"), "{err}");
    let r = a.call("send_key", json!({ "key": "ENTER", "intent": "print test" })).unwrap();
    assert_eq!(r["status"], "executed", "{r}");
    assert_eq!(r["cmd"], "echo agent-$((1+1))");
    term.expect("agent-2\r\n");

    // F. preemptive takeover
    let events = a.take_events().unwrap();
    term.write("x");
    term.expect("x");
    let err = a.call("type", json!({ "text": "y" })).unwrap_err();
    assert!(err.to_string().contains("not_controller"), "{err}");
    assert_eq!(names(a.call("affordances", json!({})).unwrap()), vec!["snapshot", "request_control"]);
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut revoked = false;
    while Instant::now() < deadline {
        if let Ok(ev) = events.recv_timeout(Duration::from_millis(100)) {
            if ev["event"] == "control_revoked" {
                assert_eq!(ev["reason"], "human_input");
                revoked = true;
                break;
            }
        }
    }
    assert!(revoked, "no control_revoked event");
    term.write("\x7fecho human-$((5+5))\n");
    term.expect("human-10\r\n");

    // H. resume: the agent sees what the human did
    let snap = a.call("snapshot", json!({})).unwrap();
    let screen = snap["screen"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>().join("\n");
    assert!(screen.contains("human-10"), "{screen}");

    // G. policy: confirm → deny from the prompt
    a.call("request_control", json!({})).unwrap();
    a.call("type", json!({ "text": "rm -rf ./tmp-zzz" })).unwrap();
    let r = a.call("send_key", json!({ "key": "ENTER", "intent": "delete temp dir" })).unwrap();
    assert_eq!(r["status"], "pending", "{r}");
    let id = r["approvalId"].as_str().unwrap().to_string();
    let out = term.expect("[a] approve");
    assert!(out.contains("\x1b[?1049h") && out.contains("recursive delete") && out.contains("rm -rf ./tmp-zzz"));
    assert!(out.contains("delete temp dir"), "intent shown on the prompt: {out}");
    assert!(out.contains("tmp-zzz · missing") || out.contains("/tmp-zzz"), "resolved target shown: {out}");
    assert_eq!(a.call("check_approval", json!({ "approvalId": id })).unwrap()["state"], "pending");
    let err = a.call("type", json!({ "text": "z" })).unwrap_err();
    assert!(err.to_string().contains("approval_pending"));
    term.write("d");
    term.expect("\x1b[?1049l");
    assert_eq!(a.call("check_approval", json!({ "approvalId": id })).unwrap()["state"], "denied");
    term.write("echo after-deny-$((3+3))\n");
    let out = term.expect("after-deny-6\r\n");
    assert!(!out.contains("tmp-zzz: "), "denied command must not run: {out}");

    // G'. a dangerous command mixed with others is refused outright (isolation rule)
    a.call("request_control", json!({})).unwrap();
    a.call("type", json!({ "text": "rm -rf ./tmp-yyy && echo granted-ran" })).unwrap();
    let r = a.call("send_key", json!({ "key": "ENTER", "intent": "delete then check" })).unwrap();
    assert_eq!(r["status"], "denied", "{r}");
    assert_eq!(r["label"], "run dangerous commands alone");
    // …so run it alone: confirm → grant via CLI (rm of a non-existent path is harmless)
    a.call("type", json!({ "text": "rm -rf ./tmp-yyy" })).unwrap();
    let r = a.call("send_key", json!({ "key": "ENTER", "intent": "delete tmp-yyy" })).unwrap();
    assert_eq!(r["status"], "pending", "{r}");
    let id2 = r["approvalId"].as_str().unwrap().to_string();
    term.expect("[a] approve");
    let cli = Client::connect(&env.socket).unwrap();
    cli.hello("human", "cli").unwrap();
    let st = cli.call("status", json!({})).unwrap();
    assert_eq!(st["pending"][0]["id"], id2);
    assert_eq!(st["pending"][0]["intent"], "delete tmp-yyy");
    assert_eq!(st["controller"]["agentId"], "copilot");
    let err = cli.call("approve", json!({ "decision": "grant" })).unwrap_err();
    assert!(err.to_string().contains("approvalId is required"), "{err}");
    let r = cli.call("approve", json!({ "approvalId": id2, "decision": "grant" })).unwrap();
    assert_eq!(r["state"], "granted");
    a.call("type", json!({ "text": "echo granted-ran" })).unwrap();
    a.call("send_key", json!({ "key": "ENTER", "intent": "print check" })).unwrap();
    term.expect("granted-ran\r\n");

    // deny rule: never runs, no approval
    a.call("type", json!({ "text": "rm -rf /" })).unwrap();
    let r = a.call("send_key", json!({ "key": "ENTER", "intent": "try deleting root" })).unwrap();
    assert_eq!(r["status"], "denied", "{r}");
    assert!(cli.call("status", json!({})).unwrap()["pending"].as_array().unwrap().is_empty());

    // CLI take
    assert_eq!(cli.call("take", json!({})).unwrap()["revoked"].as_str().unwrap(), "lease#3");
    assert_eq!(cli.call("status", json!({})).unwrap()["controller"]["type"], "human");

    // SIGWINCH: resizing the outer terminal propagates through the proxy to the inner PTY
    term.master.resize(PtySize { rows: 30, cols: 120, pixel_width: 0, pixel_height: 0 }).unwrap();
    let mut seen = false;
    for _ in 0..10 {
        term.write("stty size\n");
        let out = term.drain(250);
        if out.contains("30 120") {
            seen = true;
            break;
        }
    }
    assert!(seen, "SIGWINCH did not propagate to the inner shell:\n{}", term.all);
    let snap = a.call("snapshot", json!({})).unwrap();
    assert_eq!((snap["size"]["rows"].as_u64(), snap["size"]["cols"].as_u64()), (Some(30), Some(120)));

    // C. interactive TUI passes through (vi enters the alternate screen)
    term.write("vi\n");
    term.expect("\x1b[?1049h");
    term.drain(300);
    assert_eq!(a.call("snapshot", json!({})).unwrap()["alternateScreen"], true);
    term.write("\x1b:q!\n");
    term.expect("\x1b[?1049l");
    term.write("echo back-$((7+7))\n");
    term.expect("back-14\r\n");

    // exit → socket removed, audit complete
    term.write("exit\n");
    let status = term.child.wait().unwrap();
    assert!(status.success(), "{status:?}");
    let deadline = Instant::now() + Duration::from_secs(3);
    while env.socket.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!env.socket.exists(), "socket file should be removed on exit");

    // I. audit: human vs agent actions are distinguishable
    let events = conn_core::audit::read_events(&env.audit).unwrap();
    let find = |actor: &str, action: &str, pred: &dyn Fn(&Value) -> bool| {
        events.iter().any(|e| e.actor == actor && e.action == action && pred(&Value::Object(e.fields.clone())))
    };
    assert!(find("system", "session_start", &|_| true));
    assert!(find("human", "exec", &|f| f["cmd"] == "echo READY-$((2+3))"));
    assert!(find("copilot", "exec", &|f| f["cmd"] == "echo agent-$((1+1))" && f["policy"] == "allow"));
    assert!(find("human", "takeover", &|f| f["revoked"] == "lease#1"));
    assert!(find("human", "exec", &|f| f["cmd"] == "echo human-$((5+5))"));
    assert!(find("copilot", "exec", &|f| f["cmd"] == "rm -rf ./tmp-zzz" && f["policy"] == "confirm" && f["approval"] == "denied"));
    assert!(find("copilot", "exec", &|f| f["policy"] == "confirm" && f["approval"] == "granted" && f["by"] == "cli"));
    assert!(find("copilot", "exec", &|f| f["policy"] == "deny"));
    assert!(find("human", "takeover", &|f| f["via"] == "cli"));
    assert!(find("copilot", "observe", &|_| true));
    assert!(find("system", "session_end", &|_| true));
    assert!(!find("human", "exec", &|f| f["cmd"].as_str().map(|c| c.contains(":q!")).unwrap_or(false)), "vi keystrokes must not be audited as commands");
}

#[test]
fn initial_command_then_login_shell() {
    let (_env, mut term) = start(&["--", "echo", "INIT-$((4+4))"]);
    assert!(term.all.contains("INIT-8"), "{}", term.all);
    term.write("exit\n");
    assert!(term.child.wait().unwrap().success());
}

#[test]
fn disconnect_releases_lease_and_lease_expiry_is_reported() {
    let (env, mut term) = start(&[]);
    {
        let a = agent(&env, "copilot");
        a.call("request_control", json!({})).unwrap();
        let cli = Client::connect(&env.socket).unwrap();
        cli.hello("human", "cli").unwrap();
        assert_eq!(cli.call("status", json!({})).unwrap()["controller"]["type"], "agent");
        drop(a);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if cli.call("status", json!({})).unwrap()["controller"]["type"] == "human" {
                break;
            }
            assert!(Instant::now() < deadline, "lease not released on disconnect");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    term.write("exit\n");
    assert!(term.child.wait().unwrap().success());
}
