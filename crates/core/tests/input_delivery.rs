mod common;
use common::SessionExt;
use conn_core::{
    approval::Decision,
    audit::Audit,
    policy::{Policy, PolicyStore},
    session::{ConnKind, KeyResult, ServerEvent, Session, SessionConfig, SessionError},
    Pacing,
};
use portable_pty::{MasterPty, PtySize};
use std::{
    io::{self, Read, Write},
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct State {
    bytes: Vec<u8>,
    remaining: Option<usize>,
    fail_flush: bool,
}
struct Writer(Arc<Mutex<State>>);
impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut s = self.0.lock().unwrap();
        if s.remaining == Some(0) {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "fixture"));
        }
        let n = s.remaining.unwrap_or(bytes.len()).min(bytes.len());
        s.bytes.extend_from_slice(&bytes[..n]);
        if let Some(left) = s.remaining.as_mut() {
            *left -= n;
        }
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.0.lock().unwrap().fail_flush {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "fixture"))
        } else {
            Ok(())
        }
    }
}
struct RejectedSize;
impl MasterPty for RejectedSize {
    fn resize(&self, _: PtySize) -> anyhow::Result<()> {
        anyhow::bail!("fixture resize failure")
    }
    fn get_size(&self) -> anyhow::Result<PtySize> {
        Ok(PtySize::default())
    }
    fn try_clone_reader(&self) -> anyhow::Result<Box<dyn Read + Send>> {
        Ok(Box::new(io::empty()))
    }
    fn take_writer(&self) -> anyhow::Result<Box<dyn Write + Send>> {
        Ok(Box::new(io::sink()))
    }
    #[cfg(unix)]
    fn process_group_leader(&self) -> Option<libc::pid_t> {
        None
    }
    #[cfg(unix)]
    fn as_raw_fd(&self) -> Option<std::os::fd::RawFd> {
        None
    }
}
fn fixture(
    master: Option<Box<dyn MasterPty + Send>>,
) -> (
    Session,
    Arc<Mutex<State>>,
    Arc<Mutex<Vec<conn_core::audit::Event>>>,
) {
    let state = Arc::new(Mutex::new(State::default()));
    let (audit, events) = Audit::memory();
    let mut s = Session::new(SessionConfig {
        rows: 24,
        cols: 80,
        audit,
        policy: PolicyStore::from_policy(Policy::parse(&common::Harness::test_policy()).unwrap()),
        pty_writer: Box::new(Writer(state.clone())),
        master,
        pacing: Pacing::default(),
        shell_pid: None,
    });
    s.register_conn(1, ConnKind::Agent, "fixture", Box::new(|_: ServerEvent| {}));
    s.agent_request_control(1).unwrap();
    (s, state, events)
}
#[test]
fn rejected_resize_does_not_publish_or_change_screen_dimensions() {
    let (mut s, _, _) = fixture(Some(Box::new(RejectedSize)));
    let frames = Arc::new(Mutex::new(Vec::new()));
    let out = frames.clone();
    s.set_output_frame_sink(Some(Box::new(move |frame| {
        out.lock().unwrap().push(frame.size.clone())
    })));
    let revision = s.screen().revision();
    assert!(matches!(s.resize(30, 104), Err(SessionError::ResizeFailed)));
    assert_eq!(s.screen().size().cols, 80);
    assert_eq!(s.screen().revision(), revision);
    assert!(frames.lock().unwrap().is_empty());
}
#[test]
fn partial_input_is_uncertain_and_not_automatically_replayed() {
    let (mut s, io, events) = fixture(None);
    io.lock().unwrap().remaining = Some(1);
    assert!(matches!(
        s.agent_type(1, "printf fixture"),
        Err(SessionError::InputDeliveryUnknown)
    ));
    io.lock().unwrap().remaining = None;
    assert!(matches!(
        s.agent_type(1, "printf fixture"),
        Err(SessionError::InputUnverified)
    ));
    assert!(matches!(
        s.agent_send_key(1, "ENTER"),
        Err(SessionError::InputUnverified)
    ));
    assert_eq!(io.lock().unwrap().bytes, b"p");
    assert!(!events.lock().unwrap().iter().any(|e| e.action == "exec"));
}
#[test]
fn uncertain_approval_delivery_cannot_be_granted_again() {
    let (mut s, io, events) = fixture(None);
    s.agent_type(1, "printf fixture > result").unwrap();
    let KeyResult::Pending { approval_id, .. } = s.agent_send_key(1, "ENTER").unwrap() else {
        panic!()
    };
    // Enter may have reached the shell before flush fails.
    io.lock().unwrap().fail_flush = true;
    assert!(matches!(
        s.resolve_approval(&approval_id, Decision::Grant, "test"),
        Err(SessionError::InputDeliveryUnknown)
    ));
    io.lock().unwrap().fail_flush = false;
    assert!(s
        .resolve_approval(&approval_id, Decision::Grant, "test")
        .is_err());
    assert_eq!(
        io.lock()
            .unwrap()
            .bytes
            .iter()
            .filter(|b| **b == b'\r')
            .count(),
        1
    );
    assert!(!events
        .lock()
        .unwrap()
        .iter()
        .any(|e| e.action == "exec" && e.fields["approval"] == "granted"));
}
