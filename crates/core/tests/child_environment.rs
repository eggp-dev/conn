#![cfg(target_os = "linux")]
mod common;
use conn_core::{
    audit::Audit,
    backend::Profile,
    policy::{Policy, PolicyStore},
    Engine, EngineConfig,
};
use std::{
    process::Command,
    time::{Duration, Instant},
};

// Each case runs in its own process: never mutate global environment alongside
// other PTY tests. Check what an actual Engine child receives, including overrides.
#[test]
fn appimage_host_shell_environment_and_profile_overrides() {
    for case in ["inherited", "profile", "ordinary"] {
        let mut child = Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--exact",
                "child_environment_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("CONN_ENV_TEST_CASE", case)
            .env("APPDIR", "/tmp/conn-bundle")
            .env("APPIMAGE", "/opt/Conn.AppImage")
            .env(
                "LD_LIBRARY_PATH",
                "/tmp/conn-bundle/usr/lib:/opt/user-lib::/opt/another-lib",
            );
        if case == "ordinary" {
            child.env_remove("APPIMAGE");
        }
        let output = child.output().unwrap();
        assert!(
            output.status.success(),
            "{case}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[ignore = "launched by the parent with a dedicated environment"]
fn child_environment_fixture() {
    let case = std::env::var("CONN_ENV_TEST_CASE").unwrap();
    let mut profile = Profile::local("environment-test".into(), "/bin/sh".into());
    profile.args = vec![
        "-c".into(),
        "printf 'SHELL_LD=<%s>\\n' \"${LD_LIBRARY_PATH-UNSET}\"".into(),
    ];
    if case == "profile" {
        profile
            .env
            .insert("LD_LIBRARY_PATH".into(), "/opt/profile-lib".into());
    }
    let output: common::Buf = Default::default();
    let engine = Engine::spawn(EngineConfig {
        profile: Some(profile),
        audit: Some(Audit::null()),
        policy: Some(PolicyStore::from_policy(
            Policy::parse("default: deny\n").unwrap(),
        )),
        output_frame: Some(common::frame_sink(&output)),
        ..Default::default()
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !engine.has_exited() {
        assert!(Instant::now() < deadline, "child did not exit");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(engine.wait(), Some(0));
    let expected = match case.as_str() {
        "profile" => "/opt/profile-lib",
        "ordinary" => "/tmp/conn-bundle/usr/lib:/opt/user-lib::/opt/another-lib",
        _ => "/opt/user-lib::/opt/another-lib",
    };
    assert!(String::from_utf8_lossy(&output.lock().unwrap())
        .contains(&format!("SHELL_LD=<{expected}>")));
}
