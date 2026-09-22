#![cfg(unix)]
mod common;
use conn_core::{Engine, EngineConfig, backend::Profile, session::{ConnKind, ServerEvent}};
use std::{sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}}, time::{Duration, Instant}};

#[test]
fn slow_consumer_applies_every_byte_before_exit_without_holding_up_pty_reads() {
    let payload_bytes: usize = std::env::var("CONN_OUTPUT_TEST_BYTES").ok().and_then(|n| n.parse().ok()).unwrap_or(4 * 1024 * 1024);
    let dir = tempfile::tempdir().unwrap();
    let payload = dir.path().join("payload");
    std::fs::write(&payload, vec![b'x'; payload_bytes]).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let tail = Arc::new(Mutex::new(Vec::<u8>::new()));
    let capture_count = count.clone(); let capture_tail = tail.clone();
    let mut profile = Profile::local("output-test".into(), "/bin/sh".into());
    profile.args = vec!["-c".into(), "stty -opost; sleep 0.1; cat payload; printf '\nFINAL_OUTPUT\n'".into()];
    let started = Instant::now();
    let engine = Engine::spawn(EngineConfig {
        cwd: Some(dir.path().into()), profile: Some(profile),
        audit: Some(conn_core::audit::Audit::null()),
        output_frame: Some(Box::new(move |frame| {
            // Force a backlog much longer than the former exit timeout.
            std::thread::sleep(Duration::from_millis(3));
            capture_count.fetch_add(frame.data.len(), Ordering::SeqCst);
            let mut last = capture_tail.lock().unwrap();
            last.extend_from_slice(&frame.data);
            if last.len() > 64 { let n = last.len()-64; last.drain(..n); }
        })), ..Default::default()
    }).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    engine.session().lock().register_conn(1, ConnKind::Frontend, "test", Box::new(common::VecSink(events.clone())));
    let deadline = Instant::now()+Duration::from_secs(120);
    while !engine.has_exited() {
        assert!(Instant::now()<deadline, "output drain hung");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(engine.wait(), Some(0));
    assert_eq!(count.load(Ordering::SeqCst), payload_bytes + b"\nFINAL_OUTPUT\n".len());
    assert!(tail.lock().unwrap().ends_with(b"\nFINAL_OUTPUT\n"));
    assert!(events.lock().unwrap().iter().any(|e| matches!(e, ServerEvent::ProcessExited { exit_code: Some(0) })));
    eprintln!("drained {payload_bytes} bytes through a slow owner in {:?}, final output precedes exit", started.elapsed());
}
