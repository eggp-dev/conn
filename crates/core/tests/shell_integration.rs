#![cfg(unix)]
mod common;
use common::{Buf, SessionExt};
use conn_core::{
    audit::{Audit, Event},
    backend::Profile,
    policy::{Policy, PolicyStore},
    Engine, EngineConfig,
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

struct Shell {
    engine: Engine,
    events: Arc<Mutex<Vec<Event>>>,
    output: Buf,
    _dir: tempfile::TempDir,
}
impl Shell {
    fn new(shell: &str, private: bool) -> Self {
        Self::with_prompt(shell, private, "")
    }
    fn with_prompt(shell: &str, private: bool, prompt: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let (audit, events) = Audit::memory();
        let output = Buf::default();
        let mut profile = Profile::local("test".into(), shell.into());
        profile.args = if shell.ends_with("bash") {
            vec!["--norc".into(), "-i".into()]
        } else {
            vec!["-i".into()]
        };
        profile.env.insert("PS1".into(), "conn-test$ ".into());
        profile.env.insert("HISTFILE".into(), "/dev/null".into());
        profile.env.insert("PROMPT_COMMAND".into(), prompt.into());
        profile.env.insert("HISTCONTROL".into(), "".into());
        let inputrc = dir.path().join("inputrc");
        std::fs::write(
            &inputrc,
            "set editing-mode emacs\nset input-meta on\nset output-meta on\nset convert-meta off\n",
        )
        .unwrap();
        profile
            .env
            .insert("INPUTRC".into(), inputrc.to_string_lossy().into());
        profile
            .env
            .insert("ZDOTDIR".into(), dir.path().to_string_lossy().into());
        let engine = Engine::spawn(EngineConfig {
            external_private: private,
            profile: Some(profile),
            cwd: Some(dir.path().into()),
            audit: Some(audit),
            policy: Some(PolicyStore::from_policy(
                Policy::parse("default: allow\nrequire_intent: false\n").unwrap(),
            )),
            output_frame: Some(common::frame_sink(&output)),
            ..Default::default()
        })
        .unwrap();
        let me = Self {
            engine,
            events,
            output,
            _dir: dir,
        };
        if !private {
            me.until(|| me.engine.session().lock().status().shell_integration.state != "starting");
        }
        me
    }
    fn until(&self, f: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(8);
        while !f() {
            assert!(
                Instant::now() < deadline,
                "timeout: {:?}\n{}\n{:?}",
                self.engine.session().lock().status().shell_integration,
                String::from_utf8_lossy(&self.output.lock().unwrap()),
                self.events.lock().unwrap()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn send(&self, command: &str) {
        self.engine.write_input(format!("{command}\r").as_bytes());
    }
    fn starts(&self) -> Vec<Event> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.action == "shell_command_started")
            .cloned()
            .collect()
    }
    fn ends(&self) -> Vec<Event> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.action == "shell_command_finished")
            .cloned()
            .collect()
    }
    fn run(&self, command: &str, count: usize) {
        self.send(command);
        self.until(|| self.ends().len() == count);
    }
}

fn lifecycle(shell: &str) {
    let h = Shell::new(shell, false);
    assert_eq!(
        h.engine.session().lock().status().shell_integration.state,
        "active"
    );
    h.run("printf '%s\\n' '한글 🚀'; false", 1);
    let starts = h.starts();
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0].actor, "human");
    assert_eq!(starts[0].fields["cmd"], "printf '%s\\n' '한글 🚀'; false");
    assert_eq!(
        starts[0].fields["cwd"],
        h._dir
            .path()
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(
        starts[0].fields["commandId"],
        h.ends()[0].fields["commandId"]
    );
    assert_eq!(h.ends()[0].fields["exitCode"], 1);
    h.run("printf '%s\\n' '한글 🚀'; false", 2);
    assert_ne!(
        h.starts()[0].fields["commandId"],
        h.starts()[1].fields["commandId"]
    );
    // read runs inside the shell. Its answer must not become another command.
    h.send("printf 'AUTH_READY'; read -r -s answer; printf '\\nAUTH_DONE\\n'");
    h.until(|| {
        String::from_utf8_lossy(&h.output.lock().unwrap()).contains("AUTH_READY")
            && h.starts().len() == 3
    });
    h.send("SYNTHETIC_PASSWORD");
    h.until(|| h.ends().len() == 3);
    assert!(!format!("{:?}", h.events.lock().unwrap()).contains("SYNTHETIC_PASSWORD"));
    // A nested shell / REPL is one outer command, not its inner keystrokes.
    h.send("/bin/sh");
    h.until(|| h.starts().len() == 4);
    h.send("echo SYNTHETIC_INNER_INPUT");
    h.send("exit");
    h.until(|| h.ends().len() == 4);
    assert_eq!(h.starts().len(), 4);
    assert!(!format!("{:?}", h.events.lock().unwrap()).contains("SYNTHETIC_INNER_INPUT"));
    // A forced shell exit has no completion hook; never invent exit success.
    h.send("exec /bin/sh -c 'exit 0'");
    h.until(|| h.engine.has_exited());
    assert_eq!(h.starts().len(), 5);
    assert!(h.ends().last().unwrap().fields["exitCode"].is_null());
}

#[test]
fn existing_prompt_command_and_exit_status_are_preserved() {
    let h = Shell::with_prompt(
        "/bin/bash",
        false,
        "printf 'USER_PROMPT_STATUS=%s\\n' \"$?\"",
    );
    h.run("false", 1);
    assert_eq!(h.ends()[0].fields["exitCode"], 1);
    h.until(|| String::from_utf8_lossy(&h.output.lock().unwrap()).contains("USER_PROMPT_STATUS=1"));
    h.until(|| h.engine.session().lock().completion_prompt_ready());
    h.run("true", 2);
    assert_eq!(h.ends()[1].fields["exitCode"], 0);
}

#[test]
fn existing_debug_trap_is_not_replaced() {
    let h = Shell::with_prompt("/bin/bash", false, "trap ':' DEBUG");
    assert_eq!(
        h.engine
            .session()
            .lock()
            .status()
            .shell_integration
            .reason
            .as_deref(),
        Some("hook_conflict")
    );
    h.send("echo NO_RECORDING_WITH_CONFLICT");
    h.until(|| {
        String::from_utf8_lossy(&h.output.lock().unwrap()).contains("NO_RECORDING_WITH_CONFLICT")
    });
    assert!(h.starts().is_empty());
}

#[test]
fn ignored_history_is_not_reconstructed_from_typed_input() {
    let h = Shell::new("/bin/bash", false);
    h.run("HISTCONTROL=ignorespace", 1);
    h.send(" echo INTENTIONALLY_OMITTED");
    h.until(|| {
        String::from_utf8_lossy(&h.output.lock().unwrap()).contains("INTENTIONALLY_OMITTED")
    });
    h.until(|| h.engine.session().lock().completion_prompt_ready());
    h.run("true", 2);
    assert!(!format!("{:?}", h.starts()).contains("INTENTIONALLY_OMITTED"));
}

#[test]
fn agent_submission_links_to_shell_execution_without_human_attribution() {
    use conn_core::session::ConnKind;
    let h = Shell::new("/bin/bash", false);
    {
        let session = h.engine.session();
        let mut s = session.lock();
        s.register_conn(7, ConnKind::Agent, "test-agent", Box::new(|_| {}));
        common::present(&mut s, vec![]);
        s.agent_request_control(7).unwrap();
        s.agent_type(7, "pwd").unwrap();
        s.agent_send_key(7, "ENTER").unwrap();
    }
    h.until(|| h.ends().len() == 1);
    let starts = h.starts();
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0].actor, "test-agent");
    let records = h.events.lock().unwrap();
    let submitted = records.iter().find(|e| e.action == "exec").unwrap();
    assert!(!submitted.fields["submissionId"].is_null());
    assert_eq!(
        submitted.fields["submissionId"],
        starts[0].fields["submissionId"]
    );
}

// macOS PTYs have small buffers. Readline can block echoing a long input while
// the parent is still writing it; the output reader must not wait for that
// writer's session lock before draining more bytes from the PTY.
fn long_input_does_not_block_output(agent: bool) {
    use conn_core::session::ConnKind;
    let h = Shell::new("/bin/bash", false);
    let payload = "abcdef".repeat(500);
    let command = format!("printf 'LONG_OUTPUT<%s>\\n' '{payload}'");
    let session = h.engine.session();
    if agent {
        let mut s = session.lock();
        s.register_conn(7, ConnKind::Agent, "long-input", Box::new(|_| {}));
        s.agent_request_control(7).unwrap();
    }
    let (done, completion) = std::sync::mpsc::channel();
    let writer = std::thread::spawn(move || {
        let mut s = session.lock();
        if agent { s.agent_type(7, &command).unwrap(); }
        else { s.human_input(command.as_bytes()); }
        drop(s);
        let _ = done.send(());
    });
    let completed = completion.recv_timeout(Duration::from_secs(5)).is_ok();
    if !completed {
        // Kill independently of the session lock so a regression fails rather
        // than leaving the test runner stuck in the same deadlock.
        h.engine.terminate().unwrap();
    }
    assert!(completed, "long PTY input blocked the output reader");
    writer.join().unwrap();
    if agent { h.engine.session().lock().agent_send_key(7, "ENTER").unwrap(); }
    else { h.engine.write_input(b"\r"); }
    let expected = format!("LONG_OUTPUT<{payload}>\r\n");
    h.until(|| String::from_utf8_lossy(&h.output.lock().unwrap()).contains(&expected));
    if agent { h.engine.session().lock().agent_release_control(7).unwrap(); }
    h.until(|| h.engine.session().lock().completion_prompt_ready());
    h.send("printf 'NEXT_OUTPUT_OK\\n'");
    // Bash/readline may emit bracketed-paste mode sequences between the echoed
    // newline and command output. Check the rendered row, not adjacent raw bytes;
    // the echoed printf command cannot satisfy this exact-line assertion.
    h.until(|| h.engine.session().lock().screen().rows().iter()
        .any(|row| row.trim_end() == "NEXT_OUTPUT_OK"));
}

#[test]
fn long_agent_input_drains_pty_echo_without_deadlock() {
    long_input_does_not_block_output(true);
}

#[test]
fn long_human_paste_drains_pty_echo_without_deadlock() {
    long_input_does_not_block_output(false);
}

#[test]
fn bash_lifecycle_and_authentication_input() {
    lifecycle("/bin/bash");
}
#[test]
fn zsh_lifecycle_and_authentication_input() {
    if let Ok(path) = std::env::var("CONN_TEST_ZSH") {
        lifecycle(&path);
    } else if let Some(path) = conn_core::backend::executable("zsh") {
        lifecycle(path.to_str().unwrap());
    } else {
        eprintln!("zsh is unavailable; set CONN_TEST_ZSH to exercise it");
    }
}
#[test]
fn private_shell_never_installs_integration() {
    let h = Shell::new("/bin/bash", true);
    h.send("echo PRIVATE_COMMAND");
    h.until(|| String::from_utf8_lossy(&h.output.lock().unwrap()).contains("PRIVATE_COMMAND"));
    assert_eq!(
        h.engine
            .session()
            .lock()
            .status()
            .shell_integration
            .reason
            .as_deref(),
        Some("private_session")
    );
    assert!(h.events.lock().unwrap().is_empty());
}

#[test]
fn prompt_array_tail_does_not_disarm_human_command_recording() {
    let h = Shell::with_prompt("/bin/bash", false,
        r#"if [[ -z ${CONN_ARRAY_TEST-} ]]; then CONN_ARRAY_TEST=1; PROMPT_COMMAND[1]="printf ARRAY_HOOK_OK"; fi"#);
    h.run("printf HUMAN_ARRAY_COMMAND", 1);
    assert_eq!(h.starts()[0].fields["cmd"], "printf HUMAN_ARRAY_COMMAND");
    h.run("false", 2);
    assert_eq!(h.ends()[1].fields["exitCode"], 1);
    h.until(|| String::from_utf8_lossy(&h.output.lock().unwrap()).contains("ARRAY_HOOK_OK"));
}

#[test]
fn completion_prompt_confirmation_clears_before_async_execution_hooks() {
    let h=Shell::new("/bin/bash",false);
    let session=h.engine.session();
    assert!(session.lock().completion_prompt_ready());
    {
        let mut s=session.lock();
        // Long enough that a slow runner still observes the command running below;
        // 0.15s let it finish between the start hook and the assertion on macOS CI.
        s.human_input(b"sleep 1\r");
        // Holding Session prevents the reader from processing the queued start
        // hook. Enter itself must close the automatic completion window.
        assert!(!s.completion_prompt_ready());
        assert!(!s.status().completion_prompt_ready);
    }
    h.until(||h.starts().len()==1);
    assert!(!session.lock().completion_prompt_ready());
    h.until(||h.ends().len()==1);
    assert!(session.lock().completion_prompt_ready());
    {
        let mut s=session.lock();
        s.register_conn(7,conn_core::session::ConnKind::Agent,"test-agent",Box::new(|_|{}));
        common::present(&mut s,vec!["conn-test$ ".into()]);
        s.agent_request_control(7).unwrap();
        s.agent_type(7,"sleep 0.15").unwrap();
        s.agent_send_key(7,"ENTER").unwrap();
        s.agent_release_control(7).unwrap();
        assert!(!s.completion_prompt_ready(),"agent execution invalidates prompt before releasing control");
    }
    h.until(||h.ends().len()==2);
    assert!(session.lock().completion_prompt_ready());
    h.engine.terminate().unwrap();
}

#[test]
fn nested_foreground_commands_require_review_across_a_sharing_boundary() {
    use conn_core::{policy::Decision,approval::Decision as Approval,session::{ConnKind,KeyResult}};
    let h=Shell::new("/bin/bash",false);let session=h.engine.session();
    assert!(!session.lock().review_required());
    assert!(matches!(session.lock().analyse_line("printf local").decision,Decision::Allow));
    // Unclassified programs, including editors and nested shells, still need
    // review. Do not apply this machine's filesystem assumptions.
    h.send("/bin/sh");h.until(||h.starts().len()==1);
    {
        let mut s=session.lock();
        assert!(s.review_required());assert!(s.status().review_required);
        s.set_shared(false).unwrap();s.set_shared_with_agents(true,vec![7]).unwrap();
        assert!(s.review_required(),"sharing does not turn the foreground into a verified local prompt");
        let analysis=s.analyse_line("printf remote");
        assert!(matches!(analysis.decision,Decision::Confirm{..}));
        assert!(analysis.cwd.is_none() && analysis.segments.iter().all(|segment|segment.targets.is_empty()));
        s.register_conn(7,ConnKind::Agent,"review-test",Box::new(|_|{}));
        common::present(&mut s,vec!["remote$ ".into()]);
        s.agent_request_control(7).unwrap();s.agent_type(7,"printf remote").unwrap();
        let KeyResult::Pending{approval_id,..}=s.agent_send_key(7,"ENTER").unwrap() else {panic!("foreground execution must be reviewed")};
        assert!(s.resolve_approval(&approval_id,Approval::AllowSession,"test").is_err());
        s.resolve_approval(&approval_id,Approval::Deny,"test").unwrap();
    }
    h.send("exit");h.until(||session.lock().completion_prompt_ready());
    assert!(!session.lock().review_required(),"only the matching outer-shell end restores local trust");
    h.engine.terminate().unwrap();
}

#[test]
fn delayed_private_hook_payloads_are_discarded_after_sharing() {
    let h=Shell::new("/bin/bash",false);let session=h.engine.session();
    let proof=h._dir.path().join("private-hook-proof");
    {
        let mut s=session.lock();s.set_shared(false).unwrap();
        s.human_input(b"printf PRIVATE_HOOK_PAYLOAD > private-hook-proof\r");
        // The child completes while Session is locked, so its start/end mailbox
        // records cannot be consumed until after the sharing flag has changed.
        let until=Instant::now()+Duration::from_secs(3);
        while !proof.exists() { assert!(Instant::now()<until);std::thread::sleep(Duration::from_millis(10)); }
        s.set_shared(true).unwrap();
    }
    h.until(||session.lock().completion_prompt_ready());
    assert!(h.starts().is_empty() && h.ends().is_empty());
    assert!(!format!("{:?}",h.events.lock().unwrap()).contains("PRIVATE_HOOK_PAYLOAD"));
    // A newly confirmed prompt followed by new shared input resumes recording.
    h.run("true",1);
    assert_eq!(h.starts()[0].fields["cmd"],"true");
    h.engine.terminate().unwrap();
}


#[test]
fn idle_cancel_empty_enter_and_ignored_history_restore_prompt_policy() {
    use conn_core::policy::Decision;
    let h = Shell::new("/bin/bash", false);
    let session = h.engine.session();
    for input in [b"\x03".as_slice(), b"\r", b"\r\r"] {
        assert!(session.lock().completion_prompt_ready());
        h.engine.write_input(input);
        h.until(|| session.lock().completion_prompt_ready());
        assert!(!session.lock().review_required());
        assert!(matches!(session.lock().analyse_line("python3 --version").decision, Decision::Allow));
    }
    {
        let mut s = session.lock();
        s.register_conn(7, conn_core::session::ConnKind::Agent, "test", Box::new(|_| {}));
        s.agent_request_control(7).unwrap();
        s.agent_interrupt(7).unwrap();
        s.agent_release_control(7).unwrap();
    }
    h.until(|| session.lock().completion_prompt_ready());
    h.send("HISTCONTROL=ignorespace");
    h.until(|| session.lock().completion_prompt_ready());
    let records = h.starts().len();
    h.send(" sleep 0.25");
    h.until(|| session.lock().review_required());
    h.until(|| session.lock().completion_prompt_ready());
    assert_eq!(h.starts().len(), records, "history-excluded text is not recorded");
    assert!(matches!(session.lock().analyse_line("pwd; python3 --version").decision, Decision::Allow));
    h.engine.terminate().unwrap();
}

#[test]
fn actual_readline_empty_edits_and_midline_kill_preserve_command_boundary() {
    let h = Shell::new("/bin/bash", false);
    let session = h.engine.session();
    h.until(|| session.lock().screen().cursor_line().ends_with("conn-test$"));
    for edit in [b"\x1b[D".as_slice(), b"\x0c", b"\x7f"] {
        h.engine.write_input(edit);
        assert!(!session.lock().status().input_pending);
    }
    for (text, erase) in [("abc", b"\x7f\x7f\x7f".as_slice()), ("oneword", b"\x17"), ("word", b"\x15"), ("word", b"\x01\x1b[3~\x1b[3~\x1b[3~\x1b[3~")] {
        h.engine.write_input(text.as_bytes());
        h.until(|| session.lock().screen().cursor_line().ends_with(text));
        assert!(session.lock().status().input_pending);
        h.engine.write_input(erase);
        h.until(|| !session.lock().status().input_pending);
    }
    h.engine.write_input(b"printf HUMAN_SUFFIX");
    h.until(|| session.lock().screen().cursor_line().contains("HUMAN_SUFFIX"));
    h.engine.write_input(b"\x01\x15");
    h.until(|| session.lock().screen().cursor().col == 11);
    {
        let mut s = session.lock();
        s.register_conn(1, conn_core::session::ConnKind::Agent, "agent", Box::new(common::VecSink(Default::default())));
        s.agent_request_control(1).unwrap();
        assert!(matches!(s.agent_type(1, "printf AGENT"), Err(conn_core::session::SessionError::InputPending)));
        assert!(matches!(s.agent_send_key(1, "ENTER"), Err(conn_core::session::SessionError::InputPending)));
    }
    h.engine.write_input(b"\x0b");
    h.until(|| !session.lock().status().input_pending);
    h.run("printf BOUNDARY_OK", 1);
    assert!(!format!("{:?}", h.starts()).contains("HUMAN_SUFFIX"));
    h.engine.terminate().unwrap();
}
