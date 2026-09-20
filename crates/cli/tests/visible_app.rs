use std::process::Command;
#[test]
fn bare_invocation_starts_nothing_and_points_to_the_app() {
    let output = Command::new(env!("CARGO_BIN_EXE_conn")).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Open the Conn app"));
}
#[test]
fn help_exposes_agent_adapter_and_profile_setup() {
    let out = Command::new(env!("CARGO_BIN_EXE_conn")).arg("--help").output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(out.status.success());
    assert!(text.contains("mcp") && text.contains("profiles"));
    assert!(!text.contains("entrust") && !text.contains("streamOutput"));
}
