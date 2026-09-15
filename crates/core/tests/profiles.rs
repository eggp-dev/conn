use conn_core::backend::{Backend, BackendKind, DockerBackend, Profile, ShellKind, SshBackend};
use conn_core::policy::{Decision, Policy, PolicyStore};
use conn_core::profiles::Profiles;
use std::collections::BTreeMap;

fn local() -> Profile {
    Profile::local(
        "test".into(),
        if cfg!(windows) { "cmd.exe" } else { "/bin/sh" }.into(),
    )
}
fn config() -> Profiles {
    Profiles {
        version: 1,
        revision: 0,
        default_profile: "test".into(),
        profiles: vec![local()],
    }
}

#[test]
fn atomic_save_detects_stale_writers_and_retains_last_good_file() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("profiles.json");
    let initial = config();
    let saved = initial.save(&path).unwrap();
    assert_eq!(saved.revision, 1);
    assert!(initial.save(&path).unwrap_err().contains("another window"));
    let mut invalid = saved.clone();
    invalid.default_profile = "missing".into();
    assert!(invalid.save(&path).is_err());
    assert_eq!(Profiles::load(&path).unwrap().revision, 1);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
#[test]
fn missing_disabled_duplicate_and_bad_default_are_explicit_errors() {
    let mut c = config();
    c.profiles[0].enabled = false;
    assert!(c.validate().is_err());
    assert!(c.select(Some("test")).is_err());
    c.profiles[0].enabled = true;
    c.profiles.push(c.profiles[0].clone());
    assert!(c.validate().is_err());
    assert!(c.select(Some("unknown")).is_err());
    let mut p = local();
    p.program = "/definitely/not/a/conn-shell".into();
    assert!(!p.availability().available);
}
#[test]
fn local_argv_env_and_cwd_are_not_combined_as_shell_source() {
    let mut p = local();
    p.args = vec!["a b".into(), "$(touch should-not-run)".into()];
    p.env.insert("EXAMPLE".into(), "x; echo nope".into());
    p.cwd = Some("folder with spaces".into());
    let plan = p.prepare().unwrap();
    assert_eq!(plan.args, p.args);
    assert_eq!(plan.env, p.env);
    assert_eq!(
        plan.cwd.unwrap(),
        std::path::PathBuf::from("folder with spaces")
    );
}
#[cfg(unix)]
#[test]
fn local_shell_resolution_uses_profile_path_and_working_directory() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    symlink("/bin/sh", temp.path().join("my-shell")).unwrap();
    let mut p = local();
    p.program = "my-shell".into();
    p.env
        .insert("PATH".into(), temp.path().display().to_string());
    assert_eq!(
        p.prepare().unwrap().program,
        temp.path().join("my-shell").display().to_string()
    );
    p.program = "./my-shell".into();
    p.env.clear();
    p.cwd = Some(temp.path().display().to_string());
    assert!(p.availability().available);
}
#[test]
fn ssh_quotes_remote_arguments_and_never_uses_remote_paths_as_host_paths() {
    let mut p = local();
    p.backend = BackendKind::Ssh;
    p.shell = ShellKind::Posix;
    p.program = "/bin/bash".into();
    p.target = Some("dev@host".into());
    p.cwd = Some("/remote/it's here".into());
    p.env = BTreeMap::from([("KEY".into(), "a'; touch nope; '".into())]);
    p.args = vec!["-l".into()];
    let plan = SshBackend.prepare(&p).unwrap();
    assert!(plan.cwd.is_none());
    assert!(plan.env.is_empty());
    let command = plan.args.last().unwrap();
    assert!(command.contains("'\\''"));
    assert!(command.contains("exec env"));
    p.target = Some("-oProxyCommand=bad".into());
    assert!(p.validate().is_err());
    p.target = Some("host".into());
    p.program.clear();
    assert!(p.validate().is_err()); // cannot silently discard cwd/env
    p.cwd = None;
    p.env.clear();
    p.args.clear();
    p.shell = ShellKind::Custom;
    assert!(p.validate().is_ok());
    assert_eq!(p.prepare().unwrap().args.last().unwrap(), "host");
    p.env.insert("--chdir".into(), "/tmp".into());
    assert!(p.validate().is_err());
    p.env.clear();
    p.program = "--split-string".into();
    p.shell = ShellKind::Posix;
    assert!(p.validate().is_err());
}
#[test]
fn docker_arguments_keep_remote_environment_and_directory_on_target() {
    let mut p = local();
    p.backend = BackendKind::Docker;
    p.target = Some("test-container".into());
    p.cwd = Some("/app with spaces".into());
    p.env.insert("KEY".into(), "value with spaces".into());
    let plan = DockerBackend.prepare(&p).unwrap();
    assert!(plan.cwd.is_none());
    assert!(plan.env.is_empty());
    assert_eq!(
        &plan.args[..6],
        [
            "exec",
            "-it",
            "--workdir",
            "/app with spaces",
            "--env",
            "KEY=value with spaces"
        ]
    );
}
#[test]
fn wsl_is_explicitly_unavailable_on_other_hosts() {
    let mut p = local();
    p.backend = BackendKind::Wsl;
    p.target = Some("Ubuntu".into());
    if !cfg!(windows) {
        assert!(p.prepare().unwrap_err().contains("Windows only"));
    }
}
#[test]
fn external_review_cannot_be_lifted_for_later_commands_and_deny_survives() {
    let mut store=PolicyStore::from_policy(Policy::parse("deny:\n  - command: Remove-Item\n    label: keep files\n  - pattern: FORBIDDEN\n    label: explicit deny\ndefault: allow\n").unwrap());
    let a = store.analyse_external("Write-Output 'hello'", ShellKind::PowerShell);
    assert!(a.cwd.is_none());
    assert!(a.segments.iter().all(|s| s.targets.is_empty()));
    let Decision::Confirm { label } = a.decision else {
        panic!()
    };
    store.allow_for_session(&label);
    assert!(matches!(
        store
            .analyse_external("Write-Output 'next'", ShellKind::PowerShell)
            .decision,
        Decision::Confirm { .. }
    ));
    assert!(matches!(
        store
            .analyse_external("REMOVE-ITEM C:\\data", ShellKind::PowerShell)
            .decision,
        Decision::Deny { .. }
    ));
    assert!(matches!(
        store
            .analyse_external("echo FORBIDDEN", ShellKind::Cmd)
            .decision,
        Decision::Deny { .. }
    ));
    assert_eq!(
        store
            .analyse_external("Get-Item 'C:\\a b'", ShellKind::PowerShell)
            .segments[0]
            .command
            .as_deref(),
        Some("Get-Item")
    );
}

#[test]
fn endpoint_bind_errors_surface_without_replacing_a_live_server() {
    use conn_core::ipc::{serve_in_background, Client, Hub};
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("s.sock");
    let guard = serve_in_background(path.clone(), Hub::new()).unwrap();
    assert!(serve_in_background(path.clone(), Hub::new()).is_err());
    assert!(Client::connect(&path).is_ok());
    drop(guard);
}

#[test]
fn native_profile_launches_with_environment_and_cleans_up_on_termination() {
    use conn_core::{affordance::Actor, audit::Audit};
    use conn_core::{Engine, EngineConfig};
    let temp = tempfile::tempdir().unwrap();
    let mut p = local();
    p.args = vec![];
    p.cwd = Some(temp.path().display().to_string());
    p.env.insert("CONN_TEST_VALUE".into(), "42".into());
    let engine = Engine::spawn(EngineConfig {
        profile: Some(p),
        policy: Some(PolicyStore::from_policy(Policy::allow_all())),
        audit: Some(Audit::null()),
        ..EngineConfig::default()
    })
    .unwrap();
    let command = if cfg!(windows) {
        "echo CONN_RESULT_%CONN_TEST_VALUE%\r"
    } else {
        "printf 'CONN_RESULT_%s\\n' \"$CONN_TEST_VALUE\"\n"
    };
    engine.write_input(command.as_bytes());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    let mut seen = false;
    while std::time::Instant::now() < deadline {
        let snapshot = engine.session().lock().snapshot(Actor::Human).unwrap();
        if serde_json::to_string(&snapshot)
            .unwrap()
            .contains("CONN_RESULT_42")
        {
            seen = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    engine.terminate().unwrap();
    // Windows can report access denied once termination has completed, before
    // the engine's wait thread has published the exit. Closing twice is safe.
    #[cfg(windows)]
    engine.terminate().unwrap();
    assert!(
        seen,
        "Configured environment did not reach the native shell"
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !engine.has_exited() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(engine.has_exited());
    engine.terminate().unwrap();
}

#[cfg(unix)]
#[test]
fn connection_probe_deadline_includes_inherited_output_pipes() {
    let start = std::time::Instant::now();
    let result = conn_core::profiles::run_bounded(
        "/bin/sh",
        &["-c".into(), "sleep 3 & printf done".into()],
        1,
    );
    assert!(result.unwrap_err().contains("timed out"));
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
}
