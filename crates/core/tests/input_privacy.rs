mod common;
use common::{Harness, SessionExt};
use conn_core::{affordance::{Actor, Affordance}, audit::Audit, session::SessionError};
use std::{collections::HashSet, sync::{Arc, Mutex}};

#[test]
fn human_secrets_never_become_commands_titles_or_trace_payloads() {
    let mut h = Harness::new();
    let events = h.frontend("native");
    let trace = Arc::new(Mutex::new(Vec::new()));
    h.session.set_trace(trace.clone());
    for input in [b"SYNTHETIC_PASSWORD\r".as_slice(), "한글_SYNTHETIC_KEY\r".as_bytes(), b"\x1b[200~SYNTHETIC_PASTE\x1b[201~\r"] {
        h.session.human_input(input).unwrap();
        assert!(h.session.input_line().is_empty());
    }
    assert!(!format!("{:?}", h.audit_events()).contains("SYNTHETIC"));
    assert!(!format!("{:?}", trace.lock().unwrap()).contains("SYNTHETIC"));
    assert!(!format!("{:?}", events.lock().unwrap()).contains("SYNTHETIC"));
    assert!(h.pty_str().contains("SYNTHETIC_PASSWORD"), "input must still reach the child");
}

#[test]
fn untracked_partial_input_cannot_be_appended_to_or_reconstructed_by_an_agent() {
    let mut h = Harness::new(); h.agent(1, "agent");
    h.session.human_input(b"SYNTHETIC_PARTIAL").unwrap();
    h.session.agent_request_control(1).unwrap();
    assert!(matches!(h.session.agent_type(1, "echo unsafe"), Err(SessionError::InputPending)));
    assert!(matches!(h.session.agent_send_key(1, "ENTER"), Err(SessionError::InputPending)));
    h.session.agent_interrupt(1).unwrap();
    h.session.agent_type(1, "echo safe").unwrap();
    assert_eq!(h.session.input_line(), "echo safe");
    assert!(!format!("{:?}", h.audit_events()).contains("SYNTHETIC_PARTIAL"));
}

#[test]
fn screen_permission_blocks_reads_but_reenabling_reveals_existing_output() {
    let mut h = Harness::new(); h.agent(1, "agent");
    h.session.pty_output(b"SYNTHETIC_ECHOED_SECRET");
    common::present(&mut h.session, vec!["SYNTHETIC_ECHOED_SECRET".into()]);
    h.session.set_affordance_mask(Some(HashSet::new()));
    assert!(h.session.snapshot(Actor::Agent {conn:1}).is_err());
    assert!(h.session.snapshot(Actor::Human).unwrap().projection.screen.join("\n").contains("SYNTHETIC_ECHOED_SECRET"));
    h.session.set_affordance_mask(Some([Affordance::Snapshot].into_iter().collect()));
    assert!(h.session.snapshot(Actor::Agent {conn:1}).unwrap().projection.screen.join("\n").contains("SYNTHETIC_ECHOED_SECRET"));
}

#[cfg(unix)]
#[test]
fn audit_files_are_owner_only_and_symlink_targets_are_not_opened() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = tempfile::tempdir().unwrap(); let path=dir.path().join("audit.jsonl");
    let audit=Audit::open(&path).unwrap();
    audit.record("agent", "exec", serde_json::json!({"cmd":"SYNTHETIC_AGENT_ARGUMENT"}));
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    drop(Audit::open(&path).unwrap());
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    let link=dir.path().join("link"); symlink(&path,&link).unwrap();
    assert!(Audit::open(&link).is_err());
}

#[cfg(unix)]
#[test]
fn real_hidden_password_input_in_an_ordinary_tab_is_not_audited() {
    use conn_core::{Engine, EngineConfig, backend::Profile, policy::{Policy, PolicyStore}};
    use common::Buf;
    use std::time::{Duration, Instant};
    let (audit, events)=Audit::memory();
    let output: Buf=Default::default();
    let mut profile=Profile::local("fixture".into(),"/bin/sh".into());
    profile.args=vec!["-c".into(),"stty -echo; printf 'READY'; IFS= read -r answer; printf 'DONE'".into()];
    let engine=Engine::spawn(EngineConfig {
        profile:Some(profile),audit:Some(audit),
        policy:Some(PolicyStore::from_policy(Policy::parse("default: deny\n").unwrap())),
        output_frame:Some(common::frame_sink(&output)),..Default::default()
    }).unwrap();
    let deadline=Instant::now()+Duration::from_secs(5);
    while !String::from_utf8_lossy(&output.lock().unwrap()).contains("READY") {
        assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(10));
    }
    engine.write_input(b"SYNTHETIC_HIDDEN_PASSWORD\r").unwrap();
    while !engine.has_exited() {
        assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(engine.wait(),Some(0));
    assert!(!String::from_utf8_lossy(&output.lock().unwrap()).contains("SYNTHETIC"));
    assert!(!format!("{:?}",events.lock().unwrap()).contains("SYNTHETIC"));
    assert!(engine.session().lock().input_line().is_empty());
}

#[test]
fn empty_cursor_motion_is_not_pending_and_erasure_requires_visible_evidence() {
    let mut h = Harness::new();
    h.session.pty_output(b"prompt$ ");
    for key in [b"\x1b[D".as_slice(), b"\x0c", b"\x7f", b"\x17"] {
        h.session.human_input(key).unwrap();
        assert!(!h.session.status().input_pending);
    }
    h.session.human_input(b"abc").unwrap();
    h.session.pty_output(b"abc");
    h.session.human_input(b"\x7f\x7f\x7f").unwrap();
    assert!(h.session.status().input_pending, "keys alone never certify erasure");
    h.session.pty_output(b"\x08\x08\x08\x1b[K");
    assert!(!h.session.status().input_pending, "the shared grid returned to its empty boundary");
    assert!(h.session.input_line().is_empty());
}

#[test]
fn home_then_ctrl_u_keeps_suffix_blocked_and_redraw_does_not_clear_it() {
    let mut h = Harness::new(); h.agent(1, "agent");
    h.session.pty_output(b"prompt$ ");
    h.session.human_input(b"printf human").unwrap(); h.session.pty_output(b"printf human");
    h.session.human_input(b"\x01\x15").unwrap(); h.session.pty_output(b"\x1b[9G");
    assert!(h.session.status().input_pending);
    h.session.human_input(b"\x0c").unwrap(); h.session.pty_output(b"\x1b[2J\x1b[Hprompt$ printf human\x1b[9G");
    h.session.agent_request_control(1).unwrap();
    assert!(matches!(h.session.agent_type(1, "echo agent"), Err(SessionError::InputPending)));
    assert!(matches!(h.session.agent_send_key(1, "ENTER"), Err(SessionError::InputPending)));
    // Delete the suffix at Home; only its actual echo can reopen input.
    h.session.human_input(b"\x0b").unwrap(); h.session.pty_output(b"\x1b[K");
    assert!(!h.session.status().input_pending);
    assert!(!format!("{:?}", h.audit_events()).contains("printf human"));
}

#[test]
fn hidden_or_unechoed_input_is_not_inferred_from_empty_screen() {
    let mut h = Harness::new();
    h.session.pty_output(b"Password: ");
    h.session.human_input(b"SYNTHETIC_SECRET").unwrap();
    h.session.human_input(b"\x15").unwrap();
    h.session.pty_output(b"\x07");
    assert!(h.session.status().input_pending, "no visible edit was observed");
    assert!(h.session.input_line().is_empty());
    assert!(!format!("{:?}", h.audit_events()).contains("SYNTHETIC_SECRET"));
    h.session.human_input(b"\x03").unwrap();
    assert!(!h.session.status().input_pending);
}

#[test]
fn typing_and_erasing_echoed_in_one_chunk_recovers_without_a_cancel() {
    let mut h = Harness::new();
    h.session.pty_output(b"prompt$ ");
    h.session.human_input(b"x").unwrap();
    h.session.human_input(b"\x7f").unwrap();
    h.session.pty_output(b"x\x08 \x08");
    assert!(!h.session.status().input_pending);
}
