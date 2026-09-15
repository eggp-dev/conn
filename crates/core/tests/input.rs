use conn_core::input::{resolve_command, resolve_command_prefer_vt, InputTracker, LineEvent};

fn enter(tracker: &mut InputTracker) -> LineEvent {
    let mut ev = tracker.feed(b"\r", 0);
    assert_eq!(ev.len(), 1);
    ev.pop().unwrap()
}

#[test]
fn type_then_enter_extracts_command() {
    let mut t = InputTracker::new();
    assert!(t.feed(b"kubectl get pods", 5).is_empty());
    assert_eq!(t.prompt_col(), Some(5));
    match enter(&mut t) {
        LineEvent::Enter { tracked, dirty, prompt_col } => {
            assert_eq!(tracked, "kubectl get pods");
            assert!(!dirty);
            assert_eq!(prompt_col, Some(5));
        }
        _ => panic!(),
    }
    assert_eq!(t.line(), "");
    assert_eq!(t.prompt_col(), None);
}

#[test]
fn backspace_and_ctrl_u() {
    let mut t = InputTracker::new();
    t.feed(b"lss\x7f", 0);
    assert_eq!(t.line(), "ls");
    t.feed(b" -la\x17", 0);
    assert_eq!(t.line(), "ls ");
    t.feed(b"\x15", 0);
    assert_eq!(t.line(), "");
    t.feed(b"abc", 0);
    let ev = t.feed(b"\x03", 0);
    assert_eq!(ev, vec![LineEvent::Interrupt]);
    assert_eq!(t.line(), "");
}

#[test]
fn escape_sequences_and_tab_mark_dirty() {
    let mut t = InputTracker::new();
    t.feed(b"git st", 2);
    assert!(!t.is_dirty());
    t.feed(b"\t", 2);
    assert!(t.is_dirty());
    let mut t = InputTracker::new();
    t.feed(b"\x1b[A", 2);
    assert!(t.is_dirty());
    assert_eq!(t.line(), "");
    assert_eq!(t.prompt_col(), Some(2));
}

#[test]
fn utf8_and_split_sequences() {
    let mut t = InputTracker::new();
    let s = "echo 한글".as_bytes();
    t.feed(&s[..7], 0);
    t.feed(&s[7..], 0);
    assert_eq!(t.line(), "echo 한글");
    t.feed(b"\x7f", 0);
    assert_eq!(t.line(), "echo 한");
}

#[test]
fn crlf_is_one_enter() {
    let mut t = InputTracker::new();
    let ev = t.feed(b"ls\r\n", 0);
    assert_eq!(ev.len(), 1);
}

#[test]
fn resolve_prefers_tracked_when_clean() {
    assert_eq!(resolve_command("ls", false, "$ l", Some(2)), "ls");
}

#[test]
fn resolve_uses_vt_when_dirty() {
    assert_eq!(resolve_command("ls sr", true, "$ ls src/", Some(2)), "ls src/");
    // VT yields nothing → fall back to tracked
    assert_eq!(resolve_command("ls sr", true, "", Some(2)), "ls sr");
    // no prompt col, tracked appears at end of line
    assert_eq!(resolve_command("ls", true, "dev$ ls", None), "ls");
}

#[test]
fn resolve_prefer_vt_trusts_vt_when_different() {
    assert_eq!(resolve_command_prefer_vt("rm -rf x", "$ rm -rf x", Some(2)), "rm -rf x");
    assert_eq!(resolve_command_prefer_vt("rm -rf x", "$ rm -rf ./tmp", Some(2)), "rm -rf ./tmp");
    assert_eq!(resolve_command_prefer_vt("echo hi", "", Some(2)), "echo hi");
}
