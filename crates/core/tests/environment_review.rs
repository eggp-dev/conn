mod common;
use common::{Harness, SessionExt};
use conn_core::{
    approval::{ApprovalState, Decision},
    backend::{BackendKind, Profile, ShellKind},
    policy::{Policy, PolicyStore},
    session::{AgentMode, KeyResult, SessionError},
};
use std::time::{Duration, Instant};
fn profile() -> Profile {
    let mut p = Profile::local("target".into(), "/bin/bash".into());
    p.backend = BackendKind::Docker;
    p
}
fn harness() -> Harness {
    let mut h = Harness::new();
    h.session.set_execution_profile(profile());
    h.session
        .set_ttls(Duration::from_secs(2000), Duration::from_secs(300));
    h.agent(1, "same-name");
    h.session.agent_request_control(1).unwrap();
    h
}
fn submit(h: &mut Harness, cmd: &str) -> KeyResult {
    h.session.agent_type(1, cmd).unwrap();
    h.session.agent_send_key(1, "ENTER").unwrap()
}
#[test]
fn default_still_reviews_every_command() {
    let mut h = harness();
    assert!(matches!(
        submit(&mut h, "printf hello"),
        KeyResult::Pending { .. }
    ));
    assert!(!h.pty_str().contains('\r'));
}
#[test]
fn only_ordinary_known_dialect_commands_skip_environment_review() {
    let mut h = harness();
    h.session.start_environment_review(1).unwrap();
    assert!(matches!(
        submit(&mut h, "printf 'hello world'"),
        KeyResult::Executed { .. }
    ));
    for cmd in [
        r#"printf '%s' '[!*?~^]'"#,
        r#"printf "%s" "glob*""#,
        r#"printf '%s' '!!'"#,
    ] {
        assert!(
            matches!(submit(&mut h, cmd), KeyResult::Executed { .. }),
            "{cmd}"
        );
    }
    assert!(h.session.status().session_allows.is_empty());
}
#[test]
fn risks_deny_and_unsupported_grammar_keep_review() {
    for cmd in [
        "rm file",
        "sudo id",
        "eval 'echo hi'",
        "echo $HOME",
        "printf 'unfinished",
        "if true",
        "echo ok > file",
        "echo $(id)",
        "env -u UNUSED rm file",
        "env -S 'rm file'",
        "timeout 5 rm file",
        "busybox rm file",
        "python3.12 -c 'print(1)'",
        "command printf hello",
        "node --eval 'process.exit(0)'",
        "perl -pe 'print' file",
        "awk 'BEGIN {print 1}'",
        "sed -i 's/a/b/' file",
        "/bin/r[m] file",
        "/bin/r? file",
        "ls *",
        "!!",
        "!rm",
        "printf \"!!\"",
        "^ls^rm file",
        "coproc rm file",
        "repeat 1 rm file",
        "noglob rm file",
        "=rm file",
        "%1",
        "fc -s",
        "hash -p /bin/rm other",
        "alias other='rm file'",
        "trap 'rm file' EXIT",
        "prlimit --core=0 rm file",
        "chrt 0 rm file",
        "truncate -s 0 /srv/protected/data",
        "touch /srv/protected/data",
        "PATH=/tmp ls",
        "printf -v PROMPT_COMMAND 'rm file'",
    ] {
        let mut h = harness();
        h.session.start_environment_review(1).unwrap();
        assert!(
            matches!(submit(&mut h, cmd), KeyResult::Pending { .. }),
            "{cmd}"
        );
        assert!(!h.pty_str().contains('\r'), "{cmd}");
    }
    for cmd in ["rm -rf /", "printf hello; rm file", "echo hi && echo there"] {
        let mut h = harness();
        h.session.start_environment_review(1).unwrap();
        assert!(
            matches!(submit(&mut h, cmd), KeyResult::Denied { .. }),
            "{cmd}"
        );
    }
}
#[test]
fn unknown_shell_is_never_assumed_to_be_posix() {
    for shell in [
        ShellKind::Custom,
        ShellKind::PowerShell,
        ShellKind::Cmd,
        ShellKind::Fish,
    ] {
        let mut h = harness();
        let mut p = profile();
        p.shell = shell;
        h.session.set_execution_profile(p);
        h.session.start_environment_review(1).unwrap();
        assert!(matches!(
            submit(&mut h, "echo hello"),
            KeyResult::Pending { .. }
        ));
    }
}
#[test]
fn same_name_new_connection_does_not_inherit_grant() {
    let mut h = harness();
    h.session.start_environment_review(1).unwrap();
    h.agent(2, "same-name");
    assert!(h
        .session
        .environment_review_status(Some(2), Instant::now())
        .is_empty());
    h.session.connection_closed(1);
    h.agent(3, "same-name");
    assert!(h.session.status().environment_review.is_empty());
    h.session.agent_request_control(3).unwrap();
    h.session.agent_type(3, "echo hi").unwrap();
    assert!(matches!(
        h.session.agent_send_key(3, "ENTER").unwrap(),
        KeyResult::Pending { .. }
    ));
}
#[test]
fn enabling_does_not_resolve_waiting_approval_or_expand_category_allow() {
    let mut h = harness();
    let KeyResult::Pending { approval_id, .. } = submit(&mut h, "echo hello") else {
        panic!()
    };
    h.session.start_environment_review(1).unwrap();
    assert_eq!(
        h.session.check_approval(&approval_id).unwrap().state,
        ApprovalState::Pending
    );
    assert!(h
        .session
        .resolve_approval(&approval_id, Decision::AllowSession, "test")
        .is_err());
    assert!(!h.pty_str().contains('\r'));
    h.session
        .resolve_approval(&approval_id, Decision::Grant, "test")
        .unwrap();
    assert!(h.pty_str().ends_with('\r'));
}
#[test]
fn clock_expiry_and_stop_cancel_queued_input_without_clearing_it() {
    for expire in [false, true] {
        let mut h = harness();
        let mut pacing = h.session.pacing().clone();
        pacing.enter_grace_ms = 1_000_000;
        pacing.lease_ttl_secs = 2000;
        h.session.set_pacing(pacing);
        h.session.start_environment_review(1).unwrap();
        let KeyResult::Scheduled { exec_id, .. } = submit(&mut h, "echo queued") else {
            panic!()
        };
        if expire {
            h.session.tick(Instant::now() + Duration::from_secs(901));
        } else {
            assert!(h.session.stop_environment_review(1, "owner_stopped"));
        }
        assert!(h.session.execute_now_scheduled(&exec_id).is_err());
        assert!(!h.pty_str().contains('\r'));
        assert_eq!(h.session.input_line(), "echo queued");
        assert!(matches!(
            h.session.agent_type(1, "more"),
            Err(SessionError::InputUnverified)
        ));
        assert!(h.session.status().environment_review.is_empty());
    }
}
#[test]
fn lease_expiry_preserves_grant_but_human_takeover_revokes_it() {
    let mut h = harness();
    h.session
        .set_ttls(Duration::from_secs(1), Duration::from_secs(300));
    h.session.agent_request_control(1).unwrap();
    h.session.start_environment_review(1).unwrap();
    h.session.tick(Instant::now() + Duration::from_secs(2));
    assert!(h.session.controller().is_human());
    assert_eq!(h.session.status().environment_review.len(), 1);
    assert!(h.session.agent_type(1, "echo blocked").is_err());
    h.session.human_take();
    assert!(h.session.status().environment_review.is_empty());
}
#[test]
fn profile_sharing_close_and_human_input_invalidate() {
    for action in 0..4 {
        let mut h = harness();
        h.session.start_environment_review(1).unwrap();
        match action {
            0 => h.session.set_execution_profile(profile()),
            1 => h.session.set_shared(false).unwrap(),
            2 => h.session.process_exited(None),
            _ => h.session.human_input(b"x").unwrap(),
        }
        assert!(h.session.status().environment_review.is_empty());
    }
}
#[test]
fn mode_and_privacy_controls_are_preserved() {
    let mut h = harness();
    h.session.start_environment_review(1).unwrap();
    h.session.set_mode(AgentMode::Copilot).unwrap();
    h.session.agent_request_control(1).unwrap();
    assert!(matches!(
        submit(&mut h, "echo hello"),
        KeyResult::Proposed { .. }
    ));
    h.session.set_mode(AgentMode::Observe).unwrap();
    assert!(h.session.agent_type(1, "echo hi").is_err());
    h.session.set_shared(false).unwrap();
    assert!(h.session.start_environment_review(1).is_err());
}
#[test]
fn duplicate_enable_and_queries_never_extend_deadline() {
    let mut h = harness();
    h.session.start_environment_review(1).unwrap();
    let before = h.session.status().environment_review[0].remaining_ms;
    assert!(h.session.start_environment_review(1).is_err());
    let after = h.session.environment_review_status(Some(1), Instant::now());
    assert!(after[0].remaining_ms <= before);
    h.session.tick(Instant::now() + Duration::from_secs(900));
    assert!(h.session.status().environment_review.is_empty());
}
#[test]
fn public_agent_dispatch_can_only_read_its_own_state() {
    let mut h = harness();
    h.agent(2, "same-name");
    h.session.start_environment_review(1).unwrap();
    let s = std::sync::Arc::new(parking_lot::Mutex::new(h.session));
    for method in [
        "start_environment_review",
        "stop_environment_review",
        "approve",
    ] {
        assert_eq!(
            conn_core::agent_commands::dispatch(&s, 1, method, &serde_json::json!({"connId":1}))
                .unwrap_err()
                .code,
            "owner_required"
        );
    }
    let own = conn_core::agent_commands::dispatch(&s, 1, "status", &serde_json::json!({})).unwrap();
    let other =
        conn_core::agent_commands::dispatch(&s, 2, "status", &serde_json::json!({})).unwrap();
    assert_eq!(own["environmentReview"].as_array().unwrap().len(), 1);
    assert_eq!(other["environmentReview"].as_array().unwrap().len(), 0);
}
#[test]
fn existing_category_allows_do_not_lift_risk_in_uncertain_environment() {
    let mut store = PolicyStore::from_policy(Policy::parse(&Harness::test_policy()).unwrap());
    store.allow_for_session("privilege escalation");
    assert!(matches!(
        store
            .analyse_external_with_environment("sudo id", ShellKind::Posix, true)
            .decision,
        conn_core::policy::Decision::Confirm { .. }
    ));
}
#[test]
fn policy_reload_cancels_queued_command_before_any_enter() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("policy.yaml");
    std::fs::write(&path, Harness::test_policy()).unwrap();
    let mut h = harness();
    let (audit, _) = conn_core::audit::Audit::memory();
    h.session = conn_core::session::Session::new(conn_core::session::SessionConfig {
        rows: 24,
        cols: 80,
        audit,
        policy: PolicyStore::open(&path).unwrap(),
        pty_writer: Box::new(common::SharedBuf(h.pty.clone())),
        master: None,
        pacing: conn_core::Pacing {
            enter_grace_ms: 10000,
            lease_ttl_secs: 2000,
            ..Default::default()
        },
        shell_pid: None,
    });
    h.session.set_execution_profile(profile());
    h.agent(1, "agent");
    h.session.agent_request_control(1).unwrap();
    h.session.start_environment_review(1).unwrap();
    let KeyResult::Scheduled { exec_id, .. } = submit(&mut h, "echo queued") else {
        panic!()
    };
    std::fs::write(&path, "deny:\n  - command: echo\n    label: changed policy\ndefault: allow\nrequire_intent: false\n").unwrap();
    h.session.tick(Instant::now() + Duration::from_secs(11));
    assert!(h.session.execute_now_scheduled(&exec_id).is_err());
    assert!(!h.pty_str().contains('\r'));
    assert!(h.session.status().environment_review.is_empty());
}
#[test]
fn queued_profile_change_and_participant_removal_retain_input_without_execution() {
    for profile_change in [false, true] {
        let mut h = harness();
        let mut pacing = h.session.pacing().clone();
        pacing.enter_grace_ms = 100;
        pacing.lease_ttl_secs = 2000;
        h.session.set_pacing(pacing);
        h.session.start_environment_review(1).unwrap();
        let KeyResult::Scheduled { exec_id, .. } = submit(&mut h, "echo queued") else {
            panic!()
        };
        if profile_change {
            h.session.set_execution_profile(profile());
        } else {
            h.session.set_shared_with_agents(false, vec![]).unwrap();
        }
        h.session.tick(Instant::now() + Duration::from_secs(1));
        assert!(h.session.execute_now_scheduled(&exec_id).is_err());
        assert!(!h.pty_str().contains('\r'));
    }
}

#[test]
fn environment_overlay_metadata_excludes_unanalysable_and_risk_reviews() {
    let store = PolicyStore::from_policy(Policy::parse(&Harness::test_policy()).unwrap());
    assert!(
        store
            .analyse_external_with_environment("printf hello", ShellKind::Posix, false)
            .blanket_confirm
    );
    assert!(
        store
            .analyse_external_with_environment("printf hello", ShellKind::Posix, true)
            .blanket_confirm
    );
    for (cmd, shell) in [
        ("sudo id", ShellKind::Posix),
        ("/bin/r[m] file", ShellKind::Posix),
        ("!!", ShellKind::Posix),
        ("coproc rm file", ShellKind::Posix),
        ("env -u UNUSED rm file", ShellKind::Posix),
        ("node --eval 'process.exit(0)'", ShellKind::Posix),
        ("printf hello", ShellKind::Custom),
    ] {
        let analysis = store.analyse_external_with_environment(cmd, shell, true);
        assert!(!analysis.blanket_confirm, "{cmd}");
        assert!(
            matches!(
                analysis.decision,
                conn_core::policy::Decision::Confirm { .. }
            ),
            "{cmd}"
        );
    }
}

#[test]
fn unknown_commands_and_unchecked_protected_targets_never_reduce() {
    let policy =
        Harness::test_policy().replace("protected:\n", "protected:\n  - '/srv/protected/**'\n");
    let store = PolicyStore::from_policy(Policy::parse(&policy).unwrap());
    for cmd in [
        "truncate -s 0 /srv/protected/data",
        "prlimit --core=0 rm file",
        "unknown-command harmless",
    ] {
        let analysis = store.analyse_external_with_environment(cmd, ShellKind::Posix, true);
        assert!(
            matches!(
                analysis.decision,
                conn_core::policy::Decision::Confirm { .. }
            ),
            "{cmd}"
        );
        assert!(!analysis.blanket_confirm, "{cmd}");
        assert!(analysis.cwd.is_none());
        assert!(analysis.segments.iter().all(|s| s.targets.is_empty()));
    }
}

#[test]
fn eligible_queries_still_obey_risk_deny_and_independent_category_allows() {
    let policy = Policy::parse("deny:\n  - command: ls\n    args: '--blocked'\n    label: blocked query\nconfirm:\n  - command: cat\n    label: review read\ndefault: allow\n").unwrap();
    let mut store = PolicyStore::from_policy(policy);
    store.allow_for_session("review read");
    let risk = store.analyse_external_with_environment("cat file", ShellKind::Posix, true);
    assert!(
        matches!(risk.decision, conn_core::policy::Decision::Confirm {ref label} if label == "review read")
    );
    assert!(!risk.blanket_confirm);
    assert!(matches!(
        store
            .analyse_external_with_environment("ls --blocked", ShellKind::Posix, true)
            .decision,
        conn_core::policy::Decision::Deny { .. }
    ));
    assert!(matches!(
        store
            .analyse_external_with_environment("ls -l", ShellKind::Posix, true)
            .decision,
        conn_core::policy::Decision::Allow
    ));
}
