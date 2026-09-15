use conn_core::{backend::Profile, profiles::Profiles};
use std::process::Command;

#[test]
fn cli_import_select_and_enable_disable_are_persistent_and_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("profiles.json");
    let source = dir.path().join("import.json");
    let shell = if cfg!(windows) { "cmd.exe" } else { "/bin/sh" };
    let c = Profiles {
        version: 1,
        revision: 0,
        default_profile: "one".into(),
        profiles: vec![
            Profile::local("one".into(), shell.into()),
            Profile::local("two".into(), shell.into()),
        ],
    };
    std::fs::write(&source, serde_json::to_vec(&c).unwrap()).unwrap();
    let run = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_conn"))
            .arg("--profiles-file")
            .arg(&file)
            .arg("--socket")
            .arg(dir.path().join("unused.sock"))
            .arg("--policy")
            .arg(dir.path().join("policy.yaml"))
            .arg("--audit")
            .arg(dir.path().join("audit.jsonl"))
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    };
    run(&["profiles", "import", source.to_str().unwrap()]);
    run(&["profiles", "disable", "two"]);
    run(&["profiles", "disable", "two"]);
    assert!(!Profiles::load(&file).unwrap().profiles[1].enabled);
    run(&["profiles", "enable", "two"]);
    run(&["profiles", "enable", "two"]);
    run(&["profiles", "set-default", "two"]);
    assert_eq!(Profiles::load(&file).unwrap().default_profile, "two");
    let out = run(&["profiles", "test", "two"]);
    let result: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["available"], true);
}
