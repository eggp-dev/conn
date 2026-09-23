//! Real shell semantics: no synthetic replacement of the terminal screen.
#![cfg(unix)]
mod common;
use common::SessionExt;
use conn_core::{
    approval::Decision,
    backend::Profile,
    session::{ConnKind, KeyResult, ServerEvent, SessionError},
    Engine, EngineConfig,
};
use std::time::{Duration, Instant};

#[track_caller]
fn wait(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !predicate() {
        assert!(Instant::now() < deadline, "real shell timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn real_bash_edited_wrapped_and_wide_prompt_commands_keep_full_review_identity() {
    for prompt in [
        r"\[\e[31m\][DEV]\[\e[0m\]fixture$ ",
        r"\[\e[31m\][DEV]\[\e[0m\]한글$ ",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let rc = dir.path().join("inputrc");
        std::fs::write(
            &rc,
            "set editing-mode emacs\nset input-meta on\nset output-meta on\nset convert-meta off\n",
        )
        .unwrap();
        let mut profile = Profile::local("review-test".into(), "/bin/bash".into());
        profile.args = vec!["--noprofile".into(), "--norc".into(), "-i".into()];
        profile.env.insert("PS1".into(), prompt.into());
        profile
            .env
            .insert("INPUTRC".into(), rc.to_string_lossy().into_owned());
        profile.env.insert("HISTFILE".into(), "/dev/null".into());
        profile.env.insert(
            "LC_ALL".into(),
            if cfg!(target_os = "macos") {
                "en_US.UTF-8"
            } else {
                "C.UTF-8"
            }
            .into(),
        );
        let engine = Engine::spawn(EngineConfig {
            profile: Some(profile),
            cwd: Some(dir.path().into()),
            rows: 24,
            cols: 80,
            audit: Some(conn_core::audit::Audit::null()),
            ..Default::default()
        })
        .unwrap();
        let session = engine.session();
        wait(|| session.lock().screen().cursor_line().contains('$'));
        wait(|| session.lock().status().shell_integration.state == "active");
        {
            let mut s = session.lock();
            s.register_conn(1, ConnKind::Agent, "fixture", Box::new(|_: ServerEvent| {}));
            s.agent_request_control(1).unwrap();
        }
        let text = format!("WRAP_BEGIN_{}_END", "x".repeat(220));
        let cmd = format!("printf '%s' > result '{text}'");
        session.lock().agent_type(1, &cmd).unwrap();
        wait(|| session.lock().screen().rows().join("").contains("_END'"));
        let id = {
            let mut s = session.lock();
            s.agent_send_key(1, "LEFT").unwrap();
            let KeyResult::Pending {
                approval_id,
                cmd: reviewed,
                ..
            } = s
                .agent_send_key_with(1, "ENTER", Some("Write a disposable fixture file".into()))
                .unwrap()
            else {
                panic!("edited redirect bypassed review")
            };
            assert_eq!(reviewed, cmd);
            approval_id
        };
        assert!(!dir.path().join("result").exists());
        session
            .lock()
            .resolve_approval(&id, Decision::Grant, "test")
            .unwrap();
        wait(|| std::fs::read_to_string(dir.path().join("result")).is_ok_and(|v| v == text));
        wait(|| session.lock().screen().cursor_line().ends_with("$"));
        wait(|| !session.lock().review_required());
        // A middle-of-line denial must not splice the remaining suffix into the
        // next command. The known idle Bash prompt permits automatic cleanup.
        let next = "printf SAFE > untouched";
        session.lock().agent_type(1, next).unwrap();
        wait(|| session.lock().screen().cursor_line().contains("untouched"));
        let id = {
            let mut s = session.lock();
            for _ in 0.."untouched".len() {
                s.agent_send_key(1, "LEFT").unwrap();
            }
            let KeyResult::Pending { approval_id, .. } = s
                .agent_send_key_with(1, "ENTER", Some("Review only".into()))
                .unwrap()
            else {
                panic!()
            };
            approval_id
        };
        session
            .lock()
            .resolve_approval(&id, Decision::Deny, "test")
            .unwrap();
        wait(|| !session.lock().status().input_pending);
        wait(|| session.lock().screen().cursor_line().ends_with('$'));
        wait(|| !session.lock().review_required());
        session.lock().agent_type(1, "printf NEXT_OK").unwrap();
        assert!(matches!(
            session
                .lock()
                .agent_send_key_with(1, "ENTER", Some("Print a fixture marker".into()))
                .unwrap(),
            KeyResult::Executed { .. }
        ));
        wait(|| {
            session
                .lock()
                .screen()
                .rows()
                .iter()
                .any(|r| r.starts_with("NEXT_OK"))
        });
        assert!(!dir.path().join("untouched").exists());
        wait(|| session.lock().screen().cursor_line().ends_with('$'));
        wait(|| !session.lock().review_required());
        session.lock().agent_send_key(1, "UP").unwrap();
        assert!(matches!(
            session.lock().agent_send_key_with(
                1,
                "ENTER",
                Some("Never submit unknown history".into())
            ),
            Err(SessionError::InputUnverified)
        ));
        engine.terminate().unwrap();
    }
}
