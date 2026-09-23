//! The structural guard: intent, normalisation, opaque constructs, isolation,
//! protected paths, resolved targets.
mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use common::{Harness, SessionExt};
use conn_core::policy::{Decision, EXAMPLE_POLICY};
use conn_core::session::{KeyResult, SessionError};

fn strict() -> Harness {
    Harness::with(EXAMPLE_POLICY, Duration::from_secs(60), Duration::from_secs(300))
}

#[test]
fn enter_requires_intent_and_records_it() {
    let mut h = strict();
    h.agent(1, "claude");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "echo hi").unwrap();
    assert!(matches!(h.session.agent_send_key(1, "ENTER"), Err(SessionError::IntentRequired)));
    assert!(matches!(h.session.agent_send_key_with(1, "ENTER", Some("  ".into())), Err(SessionError::IntentRequired)));
    assert_eq!(h.pty_str(), "echo hi", "nothing sent yet");
    let r = h.session.agent_send_key_with(1, "ENTER", Some("print greeting".into())).unwrap();
    assert!(matches!(r, KeyResult::Executed { .. }));
    let exec = h.audit_events().into_iter().find(|e| e.action == "exec").unwrap();
    assert_eq!(exec.fields["intent"], "print greeting");
}

#[test]
fn approval_carries_intent_and_resolved_targets() {
    let mut h = strict();
    h.session.set_cwd_override(Some(PathBuf::from("/Users/e/dev/proj")));
    h.agent(1, "claude");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "rm -rf ../old-proj").unwrap();
    let r = h.session.agent_send_key_with(1, "ENTER", Some("clean old-proj".into())).unwrap();
    let KeyResult::Pending { approval_id, label, .. } = r else { panic!("{r:?}") };
    assert_eq!(label, "recursive delete");
    let st = h.session.status();
    let req = st.pending.iter().find(|p| p.id == approval_id).unwrap();
    assert_eq!(req.intent.as_deref(), Some("clean old-proj"));
    let a = req.analysis.as_ref().unwrap();
    assert_eq!(a.cwd.as_deref(), Some("/Users/e/dev/proj"));
    assert_eq!(a.segments.len(), 1);
    assert_eq!(Path::new(&a.segments[0].targets[0].path), Path::new("/Users/e/dev/old-proj"));
    assert!(!a.segments[0].targets[0].exists);
}

#[test]
fn dangerous_segment_must_be_alone() {
    let mut h = strict();
    let events = h.frontend("ui");
    h.session.set_cwd_override(Some(PathBuf::from("/Users/e/dev/hello-mel")));
    h.agent(1, "copilot");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "cd .. && rm -rf -- hello-mel && test ! -e hello-mel && echo 'hello-mel removed'").unwrap();
    let r = h.session.agent_send_key_with(1, "ENTER", Some("remove hello-mel".into())).unwrap();
    let KeyResult::Denied { label, .. } = r else { panic!("{r:?}") };
    assert_eq!(label, "run dangerous commands alone");
    assert!(events.lock().unwrap().iter().any(|event| matches!(event, conn_core::session::ServerEvent::AgentExec {policy, ..} if policy == "deny:run dangerous commands alone")));
    assert!(h.session.status().input_pending, "unverified foreground input is retained for explicit recovery");
    assert!(h.session.status().pending.is_empty(), "no approval was offered");
    // the analysis still shows what it would have deleted
    let a = h.session.analyse_line("cd .. && rm -rf -- hello-mel");
    assert!(a.isolation_violation);
    assert_eq!(Path::new(&a.segments[1].targets[0].path), Path::new("/Users/e/dev/hello-mel"));
}

#[test]
fn evasions_normalise_to_the_real_command() {
    for line in ["\\rm -rf x", "/bin/rm -rf x", "command rm -rf x", "r''m -rf x", "sudo rm -rf x", "env A=1 rm -rf x"] {
        // Each policy case is independent of cancellation/recovery semantics.
        let mut h = strict(); h.agent(1, "claude"); h.session.agent_request_control(1).unwrap();
        common::present(&mut h.session, vec!["$ ".into()]);
        h.session.agent_type(1, line).unwrap();
        let r = h.session.agent_send_key_with(1, "ENTER", Some("t".into())).unwrap();
        match r {
            KeyResult::Pending { approval_id, ref label, .. } => {
                assert!(label == "recursive delete" || label == "privilege escalation", "{line}: {label}");
                h.session.resolve_approval(&approval_id, conn_core::approval::Decision::Deny, "test").unwrap();
            }
            other => panic!("{line}: {other:?}"),
        }
    }
}

#[test]
fn opaque_constructs_ask_by_default() {
    let mut h = strict();
    h.agent(1, "claude");
    h.session.agent_request_control(1).unwrap();
    for line in ["sh -c 'ls'", "eval \"$CMD\"", "find . -name '*.log' -delete", "python3 -c 'print(1)'", "echo $(whoami)"] {
        // Each policy case is independent of cancellation/recovery semantics.
        let mut h = strict(); h.agent(1, "claude"); h.session.agent_request_control(1).unwrap();
        common::present(&mut h.session, vec!["$ ".into()]);
        h.session.agent_type(1, line).unwrap();
        let r = h.session.agent_send_key_with(1, "ENTER", Some("t".into())).unwrap();
        match r {
            KeyResult::Pending { approval_id, ref label, .. } => {
                assert!(label.starts_with("opaque execution"), "{line}: {label}");
                h.session.resolve_approval(&approval_id, conn_core::approval::Decision::Deny, "test").unwrap();
            }
            other => panic!("{line}: {other:?}"),
        }
    }
    common::present(&mut h.session, vec!["$ ".into()]);
    h.session.agent_type(1, "find . -name '*.log'").unwrap();
    assert!(matches!(h.session.agent_send_key_with(1, "ENTER", Some("t".into())).unwrap(), KeyResult::Executed { .. }));
}

#[test]
fn protected_paths_deny_even_when_the_rule_would_only_confirm() {
    let yaml = EXAMPLE_POLICY.replace("protected:\n  - '~/.ssh/**'\n  - '~/.conn/**'", "protected:\n  - '/srv/keep/**'");
    let mut h = Harness::with(&yaml, Duration::from_secs(60), Duration::from_secs(300));
    h.session.set_cwd_override(Some(PathBuf::from("/srv/keep/app")));
    h.agent(1, "claude");
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "rm -rf ./data").unwrap();
    let r = h.session.agent_send_key_with(1, "ENTER", Some("clean data".into())).unwrap();
    assert!(matches!(r, KeyResult::Denied { ref label, .. } if label == "protected path"), "{r:?}");
    let a = h.session.analyse_line("rm -rf ./data");
    assert!(a.segments[0].targets[0].protected);
    assert_eq!(Path::new(&a.segments[0].targets[0].path), Path::new("/srv/keep/app/data"));
}

#[test]
fn copilot_proposal_needs_intent_and_honours_isolation() {
    let mut h = strict();
    h.agent(1, "claude");
    h.session.set_mode(conn_core::session::AgentMode::Copilot).unwrap();
    h.session.agent_request_control(1).unwrap();
    h.session.agent_type(1, "cd / && rm -rf tmp").unwrap();
    assert!(matches!(h.session.agent_send_key(1, "ENTER"), Err(SessionError::IntentRequired)));
    let KeyResult::Proposed { proposal_id, .. } = h.session.agent_send_key_with(1, "ENTER", Some("clean tmp".into())).unwrap() else { panic!() };
    assert_eq!(h.session.proposal().unwrap().intent.as_deref(), Some("clean tmp"));
    let r = h.session.accept_proposal(&proposal_id).unwrap();
    assert!(matches!(r, KeyResult::Denied { .. }), "isolation applies to committed proposals too: {r:?}");
}

#[test]
fn redirect_rule_ignores_stderr_merge() {
    let p = conn_core::policy::Policy::parse(EXAMPLE_POLICY).unwrap();
    assert_eq!(p.evaluate("make 2>&1"), Decision::Allow);
    assert!(matches!(p.evaluate("echo x > file"), Decision::Confirm { .. }));
}
