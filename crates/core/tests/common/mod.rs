#![allow(dead_code)]
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use conn_core::audit::{Audit, Event};
use conn_core::policy::{Policy, PolicyStore, EXAMPLE_POLICY};
use conn_core::authority::ConnId;
use conn_core::session::{ConnKind, ControlOutcome, EventSink, KeyResult, LeaseInfo, ServerEvent, Session, SessionConfig, SessionError};
use conn_core::Pacing;

pub type Buf = Arc<Mutex<Vec<u8>>>;

pub struct SharedBuf(pub Buf);
impl Write for SharedBuf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct VecSink(pub Arc<Mutex<Vec<ServerEvent>>>);
impl EventSink for VecSink {
    fn send(&self, ev: ServerEvent) {
        self.0.lock().unwrap().push(ev);
    }
}

/// Feed a deterministic terminal output grid; never fabricate renderer frames.
pub fn present(session: &mut Session, screen: Vec<String>) {
    session.pty_output(format!("\x1b[2J\x1b[H{}",screen.join("\r\n")).as_bytes());
}

/// Capture rendered output through the sink the native renderer installs.
pub fn frame_sink(buf: &Buf) -> conn_core::session::OutputFrameSink {
    let buf = buf.clone();
    Box::new(move |frame| buf.lock().unwrap().extend_from_slice(&frame.data))
}

/// Shorthands for tests that do not care about the request reason or the intent.
pub trait SessionExt {
    fn agent_request_control(&mut self, conn: ConnId) -> Result<LeaseInfo, SessionError>;
    fn agent_request_control_with(&mut self, conn: ConnId, reason: Option<String>) -> Result<ControlOutcome, SessionError>;
    fn agent_send_key(&mut self, conn: ConnId, key: &str) -> Result<KeyResult, SessionError>;
}

impl SessionExt for Session {
    fn agent_request_control(&mut self, conn: ConnId) -> Result<LeaseInfo, SessionError> {
        match self.agent_request_control_with(conn, None)? {
            ControlOutcome::Granted { lease } => Ok(lease),
            ControlOutcome::Pending { request_id } => panic!("control request {request_id} is pending"),
        }
    }
    fn agent_request_control_with(&mut self, conn: ConnId, reason: Option<String>) -> Result<ControlOutcome, SessionError> {
        self.agent_request_control_original(conn, serde_json::json!({ "reason": reason }))
    }
    fn agent_send_key(&mut self, conn: ConnId, key: &str) -> Result<KeyResult, SessionError> {
        self.agent_send_key_with(conn, key, None)
    }
}

pub struct Harness {
    pub session: Session,
    pub pty: Buf,
    pub audit: Arc<Mutex<Vec<Event>>>,
}

impl Harness {
    /// The example policy with `require_intent` off, so the older tests can send a
    /// bare ENTER. Intent behaviour has its own tests.
    pub fn test_policy() -> String {
        EXAMPLE_POLICY.replace("require_intent: true", "require_intent: false")
    }

    pub fn new() -> Self {
        Self::with(&Self::test_policy(), Duration::from_secs(60), Duration::from_secs(300))
    }

    pub fn with(policy_yaml: &str, lease_ttl: Duration, approval_ttl: Duration) -> Self {
        let pty: Buf = Default::default();
        let (audit, store) = Audit::memory();
        let session = Session::new(SessionConfig {
            rows: 24,
            cols: 80,
            audit,
            policy: PolicyStore::from_policy(Policy::parse(policy_yaml).unwrap()),
            pty_writer: Box::new(SharedBuf(pty.clone())),
            master: None,
            pacing: Pacing {
                lease_ttl_secs: 0,
                approval_ttl_secs: 0,
                ..Pacing::default()
            },
            shell_pid: None,
        });
        let mut session = session;
        session.set_ttls(lease_ttl, approval_ttl);
        present(&mut session, vec![]);
        Self { session, pty, audit: store }
    }

    pub fn frontend(&mut self, name: &str) -> Arc<Mutex<Vec<ServerEvent>>> {
        let events = Arc::new(Mutex::new(Vec::new()));
        self.session.subscribe(name, Box::new(VecSink(events.clone())));
        events
    }

    pub fn agent(&mut self, conn: u64, id: &str) -> Arc<Mutex<Vec<ServerEvent>>> {
        let events = Arc::new(Mutex::new(Vec::new()));
        self.session.register_conn(conn, ConnKind::Agent, id, Box::new(VecSink(events.clone())));
        events
    }

    pub fn pty_bytes(&self) -> Vec<u8> {
        self.pty.lock().unwrap().clone()
    }
    pub fn pty_str(&self) -> String {
        String::from_utf8_lossy(&self.pty_bytes()).to_string()
    }
    pub fn clear_pty(&self) {
        self.pty.lock().unwrap().clear();
    }
    pub fn audit_actions(&self) -> Vec<(String, String)> {
        self.audit.lock().unwrap().iter().map(|e| (e.actor.clone(), e.action.clone())).collect()
    }
    pub fn audit_events(&self) -> Vec<Event> {
        self.audit.lock().unwrap().clone()
    }
}
