//! The native input boundary must cancel suggestions in the same Session transaction.
//! This fixture uses a no-echo child, so output/control events cannot hide a missing cancel.
#![cfg(unix)]
use crate::{engine, extensions::JobStatus, Harness};
use serde_json::{json, Value};
use std::{
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

fn setup() -> (tempfile::TempDir, Arc<Harness>, String) {
    let dir = tempfile::tempdir().unwrap();
    let mut profile = conn_core::backend::Profile::local("quiet".into(), "/bin/sh".into());
    profile.args = vec![
        "-c".into(),
        "stty -echo; printf ready; exec cat >/dev/null".into(),
    ];
    profile.cwd = Some(dir.path().to_string_lossy().into());
    std::fs::write(
        dir.path().join("profiles.json"),
        serde_json::to_vec(&conn_core::profiles::Profiles {
            version: 1,
            revision: 0,
            default_profile: "quiet".into(),
            profiles: vec![profile],
        })
        .unwrap(),
    )
    .unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();
    let h = Arc::new(Harness::new(
        dir.path().into(),
        dir.path().join("conn.sock"),
        Arc::new(move |event, _| {
            if event == "ss:output" {
                let _ = ready_tx.send(());
            }
        }),
    ));
    let result = h.invoke("start", json!({"rows":24,"cols":80})).unwrap();
    let id = result["session"].as_str().unwrap().to_owned();
    h.invoke("attend", json!({"session":id})).unwrap();
    h.invoke("attach_output", json!({"session":id})).unwrap();
    ready_rx
        .recv_timeout(Duration::from_secs(4))
        .expect("no-echo child did not start");
    let session = engine(&h.state, &id).unwrap().session();
    let mut s = session.lock();
    s.pty_output(b"ready");
    drop(s);
    (dir, h, id)
}

fn assert_owner_action_cancels_atomically(action: &'static str, extras: Value) {
    let (_dir, h, id) = setup();
    let session = engine(&h.state, &id).unwrap().session();
    let held = session.lock();
    assert!(matches!(
        held.status().controller,
        conn_core::session::ControllerInfo::Human
    ));
    let frame = held.authoritative_surface().unwrap();
    let job = h
        .state
        .extensions
        .seed_ready_for_test(super::token(&id, &frame));
    let mut args = extras;
    args["session"] = json!(id);
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker_harness = h.clone();
    let worker = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        done_tx.send(worker_harness.invoke(action, args)).unwrap();
    });
    started_rx.recv_timeout(Duration::from_secs(1)).unwrap();

    // With the old cancel-before-Session implementation this job became Cancelled
    // while the writer was blocked, allowing another completion to register in the gap.
    let deadline = Instant::now() + Duration::from_millis(120);
    while Instant::now() < deadline {
        assert_eq!(
            h.state.extensions.completion(&id, &job.id).unwrap().status,
            JobStatus::Ready,
            "{action} cancelled before acquiring the session transaction"
        );
        assert!(matches!(done_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        std::thread::sleep(Duration::from_millis(5));
    }
    drop(held);
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("owner action blocked")
        .unwrap();
    worker.join().unwrap();
    assert_eq!(
        h.state.extensions.completion(&id, &job.id).unwrap().status,
        JobStatus::Cancelled,
        "{action} must cancel even when the human already has control and no output arrives"
    );
    assert_eq!(
        session.lock().output_seq(),
        frame.output_seq,
        "fixture must not rely on echo cancellation"
    );
    assert!(h
        .invoke("completion_accept", json!({"session":id,"id":job.id}))
        .is_err());
}

#[test]
fn no_echo_human_input_cancels_in_session_transaction() {
    assert_owner_action_cancels_atomically("input", json!({"data":"x"}));
}

#[test]
fn take_without_agent_lease_still_cancels_in_session_transaction() {
    assert_owner_action_cancels_atomically("take", json!({}));
}

#[test]
fn quiet_resize_cancels_in_session_transaction() {
    assert_owner_action_cancels_atomically("resize", json!({"rows":30,"cols":100}));
}

#[test]
fn stop_sharing_cancels_ready_completion_in_session_transaction() {
    assert_owner_action_cancels_atomically("set_sharing", json!({"shared":false}));
}

#[test]
fn resize_cancels_ready_completion_in_session_transaction() {
    assert_owner_action_cancels_atomically("resize", json!({"rows":30,"cols":90}));
}
