//! Policy and audit must see a complete command, independently of terminal wrapping.
mod common;

use common::{Harness, SessionExt};
use conn_core::approval::Decision;
use conn_core::session::{KeyResult, ServerEvent};

#[test]
fn wrapped_unicode_redirect_is_reviewed_and_audited_in_full() {
    let command = format!("printf '%s' '{}' > test-file.md", "협업 실험 🚀 ".repeat(24));
    for cols in [20, 80, 132] {
        let mut h = Harness::new();
        let events = h.frontend("ui");
        h.session.resize(4, cols);
        h.agent(1, "agent");
        h.session.agent_request_control(1).unwrap();
        h.session.pty_output("사용자@conn$ ".as_bytes());
        common::present(&mut h.session, vec!["사용자@conn$ ".into()]);
        h.session.agent_type(1, &command).unwrap();
        h.session.pty_output(command.as_bytes());
        common::present(&mut h.session, vec![command.clone()]);
        assert_ne!(h.session.screen().cursor_line(), command, "echo wraps at {cols} columns");

        let result = h.session.agent_send_key(1, "ENTER").unwrap();
        let KeyResult::Pending { approval_id, cmd, .. } = result else {
            panic!("redirect must need approval at {cols} columns: {result:?}");
        };
        assert_eq!(cmd, command);
        assert!(!h.pty_bytes().contains(&b'\r'), "no ENTER before approval");
        h.session.resolve_approval(&approval_id, Decision::Deny, "test").unwrap();
        let audit = h.audit_events();
        let request = audit.iter().find(|e| e.action == "approval_requested").unwrap();
        let exec = audit.iter().find(|e| e.action == "exec").unwrap();
        assert_eq!(request.fields["cmd"], command);
        assert_eq!(exec.fields["cmd"], command);
        assert_eq!(exec.fields["approval"], "denied");
        assert!(events.lock().unwrap().iter().any(|e| matches!(e,
            ServerEvent::AgentExec { cmd, policy, .. } if cmd == &command && policy == "confirm:denied"
        )));
    }
}

#[test]
fn wrapping_cannot_hide_a_dangerous_prefix_from_policy() {
    let command = format!("rm -- '{}' && printf '%s' 'done'", "example-".repeat(40));
    let mut h = Harness::new();
    h.session.resize(3, 40);
    h.agent(1, "agent");
    h.session.agent_request_control(1).unwrap();
    h.session.pty_output(b"dev$ ");
    common::present(&mut h.session, vec!["dev$ ".into()]);
    h.session.agent_type(1, &command).unwrap();
    h.session.pty_output(command.as_bytes());
        common::present(&mut h.session, vec![command.clone()]);
    let result = h.session.agent_send_key(1, "ENTER").unwrap();
    assert!(matches!(result, KeyResult::Denied { ref cmd, ref label }
        if cmd == &command && label == "run dangerous commands alone"), "{result:?}");
    assert!(!h.pty_bytes().contains(&b'\r'), "dangerous command never receives ENTER");
    let exec = h.audit_events().into_iter().find(|e| e.action == "exec").unwrap();
    assert_eq!(exec.fields["cmd"], command);
    assert_eq!(exec.fields["policy"], "deny");
}

#[cfg(unix)]
#[test]
fn real_pty_wrapped_unicode_write_waits_for_approval_and_records_exact_command() {
    // C.UTF-8 is not available on macOS; use its built-in UTF-8 locale.
    let locale = if cfg!(target_os = "macos") { "en_US.UTF-8" } else { "C.UTF-8" };
    unicode_write("/bin/sh", &["-i"], locale, true);
}

#[cfg(unix)]
#[test]
fn real_bash_pty_preserves_unicode_even_in_the_c_locale() {
    // macOS supplies Bash 3.2 here. A developer can also point this test at an
    // isolated older Bash build without replacing the system shell.
    let shell = std::env::var("CONN_TEST_BASH").unwrap_or_else(|_| "/bin/bash".into());
    unicode_write(&shell, &["--noprofile", "--norc", "-i"], "C", false);
}

#[cfg(unix)]
fn unicode_write(shell: &str, args: &[&str], locale: &str, expect_wrapping: bool) {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use conn_core::audit::Audit;
    use conn_core::backend::Profile;
    use conn_core::policy::{Policy, PolicyStore, EXAMPLE_POLICY};
    use conn_core::session::ConnKind;
    use conn_core::{Engine, EngineConfig};

    fn wait_for(mut ready: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if ready() { return true; }
            std::thread::sleep(Duration::from_millis(10));
        }
        ready()
    }

    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("result.txt");
    let (audit, records) = Audit::memory();
    // Do not inherit the developer's readline key bindings or terminal/locale.
    // Bash 3.2 under the C locale can interpret UTF-8 bytes as Meta commands:
    // the approval still contains the original text, but Enter may only redraw
    // `printf %s`. Explicit eight-bit input makes this fixture deterministic.
    let inputrc = dir.path().join("inputrc");
    std::fs::write(&inputrc, "set editing-mode emacs\nset input-meta on\nset output-meta on\nset convert-meta off\nset horizontal-scroll-mode off\n").unwrap();
    let mut profile = Profile::local("test".into(), shell.into());
    profile.args = args.iter().map(|arg| (*arg).into()).collect();
    let engine = Engine::spawn(EngineConfig {
        profile: Some(profile),
        cwd: Some(dir.path().to_path_buf()),
        rows: 24,
        cols: 40,
        env: vec![
            ("PS1".into(), "conn-test$ ".into()), ("PS2".into(), "conn-cont> ".into()),
            ("ENV".into(), "/dev/null".into()), ("BASH_ENV".into(), "/dev/null".into()),
            ("INPUTRC".into(), inputrc.to_string_lossy().into_owned()),
            ("HISTFILE".into(), "/dev/null".into()), ("PROMPT_COMMAND".into(), String::new()),
            ("TERM".into(), "xterm-256color".into()), ("LC_ALL".into(), locale.into()),
        ],
        audit: Some(audit),
        policy: Some(PolicyStore::from_policy(Policy::parse(EXAMPLE_POLICY).unwrap())),
        ..EngineConfig::default()
    }).unwrap();
    let session = engine.session();
    assert!(wait_for(|| session.lock().screen().cursor_line().contains("conn-test$")),
        "shell {shell} ({locale}) did not reach its prompt: {:?}", session.lock().screen().rows());
    let events = Arc::new(Mutex::new(Vec::new()));
    let text = "협업 실험 🚀 ".repeat(24);
    let command = format!("printf '%s' '{text}' > result.txt");
    {
        let mut s = session.lock();
        s.register_conn(1, ConnKind::Agent, "agent", Box::new(common::VecSink(events)));
        common::present(&mut s, vec![]);
        s.agent_request_control(1).unwrap();
        s.agent_type(1, &command).unwrap();
    }
    assert!(wait_for(|| session.lock().screen().rows().join("").contains("result.txt")),
        "shell {shell} ({locale}) did not echo the command: {:?}", session.lock().screen().rows());
    let approval_id = {
        let mut s = session.lock();
        // Legacy readline may use one horizontally scrolling row in a byte
        // locale. The UTF-8 shell case must still exercise actual VT wrapping.
        if expect_wrapping {
            assert!(s.screen().cursor().row > 0, "real echo wraps onto multiple rows");
        }
        common::present(&mut s, vec![command.clone()]);
        let result = s.agent_send_key_with(1, "ENTER", Some("write a temporary fixture".into())).unwrap();
        let KeyResult::Pending { approval_id, cmd, .. } = result else {
            panic!("wrapped redirect bypassed approval: {result:?}");
        };
        assert_eq!(cmd, command);
        assert!(!target.exists());
        approval_id
    };
    {
        let mut s = session.lock();
        common::present(&mut s, vec![command.clone()]);
        s.resolve_approval(&approval_id, Decision::Grant, "test").unwrap();
    }
    assert!(wait_for(|| std::fs::read_to_string(&target).is_ok_and(|s| s == text)),
        "shell {shell} ({locale}) did not write the submitted Unicode text; file: {:?}; screen: {:?}",
        std::fs::read(&target), session.lock().screen().rows());
    let records = records.lock().unwrap();
    let exec = records.iter().find(|e| e.actor == "agent" && e.action == "exec").unwrap();
    assert_eq!(exec.fields["cmd"], command);
    assert_eq!(exec.fields["policy"], "confirm");
    assert_eq!(exec.fields["approval"], "granted");
    engine.terminate().unwrap();
}
