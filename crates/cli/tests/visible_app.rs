use std::process::Command;
#[test]
fn removed_owner_and_headless_paths_cannot_be_launched() {
    for args in [vec![], vec!["serve"], vec!["take"], vec!["approve", "any"], vec!["entrust"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_conn")).args(&args).output().unwrap();
        assert!(!output.status.success(), "legacy path {args:?} unexpectedly ran");
        assert!(!output.stderr.is_empty());
    }
}
#[test]
fn help_exposes_agent_adapter_and_profile_setup() {
    let out = Command::new(env!("CARGO_BIN_EXE_conn")).arg("--help").output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(out.status.success());
    assert!(text.contains("mcp") && text.contains("profiles"));
    assert!(!text.contains("entrust") && !text.contains("streamOutput"));
}
