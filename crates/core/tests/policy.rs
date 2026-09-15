use conn_core::policy::{Decision, Policy, PolicyStore, Reload, EXAMPLE_POLICY};

#[test]
fn example_policy_decisions() {
    let p = Policy::parse(EXAMPLE_POLICY).unwrap();
    assert_eq!(p.evaluate("rm -rf /"), Decision::Deny { label: "delete root".into() });
    assert_eq!(p.evaluate("rm -rf ./tmp"), Decision::Confirm { label: "recursive delete".into() });
    assert_eq!(p.evaluate("kubectl delete pod x -n prod"), Decision::Confirm { label: "delete resource".into() });
    assert_eq!(p.evaluate("echo hi > out.txt"), Decision::Confirm { label: "overwrite file".into() });
    assert_eq!(p.evaluate("kubectl get pods"), Decision::Allow);
    assert_eq!(p.evaluate("ls -la"), Decision::Allow);
}

#[test]
fn deny_wins_over_confirm() {
    let p = Policy::parse("deny:\n  - pattern: 'x'\n    label: d\nconfirm:\n  - pattern: 'x'\n    label: c\n").unwrap();
    assert_eq!(p.evaluate("x"), Decision::Deny { label: "d".into() });
}

#[test]
fn default_can_be_confirm() {
    let p = Policy::parse("default: confirm\n").unwrap();
    assert!(matches!(p.evaluate("anything"), Decision::Confirm { .. }));
}

#[test]
fn bad_regex_is_an_error() {
    assert!(Policy::parse("confirm:\n  - pattern: '('\n    label: x\n").is_err());
    assert!(Policy::parse("confirm: [\n").is_err());
}

#[test]
fn reload_keeps_previous_policy_on_parse_failure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("policy.yaml");
    let mut store = PolicyStore::open(&path).unwrap(); // writes the example file
    assert!(matches!(store.evaluate("rm -rf ./x"), Decision::Confirm { .. }));

    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&path, "confirm: [\n").unwrap();
    bump_mtime(&path);
    assert!(matches!(store.maybe_reload(), Reload::Failed(_)));
    assert!(matches!(store.evaluate("rm -rf ./x"), Decision::Confirm { .. }), "old policy retained");

    std::fs::write(&path, "default: allow\n").unwrap();
    bump_mtime(&path);
    assert!(matches!(store.maybe_reload(), Reload::Reloaded));
    assert_eq!(store.evaluate("rm -rf ./x"), Decision::Allow);
    assert!(matches!(store.maybe_reload(), Reload::Unchanged));
}

fn bump_mtime(path: &std::path::Path) {
    // Make sure the mtime differs even on coarse filesystems.
    let t = std::time::SystemTime::now() + std::time::Duration::from_secs(2);
    let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    f.set_modified(t).unwrap();
}

#[test]
fn session_allow_lifts_confirm_but_never_deny() {
    let mut store = PolicyStore::from_policy(Policy::parse(EXAMPLE_POLICY).unwrap());
    store.allow_for_session("recursive delete");
    assert_eq!(store.evaluate("rm -rf ./tmp"), Decision::Allow);
    store.allow_for_session("delete root");
    assert!(matches!(store.evaluate("rm -rf /"), Decision::Deny { .. }));
    assert_eq!(store.session_allows(), vec!["delete root".to_string(), "recursive delete".to_string()]);
}
