use super::*;
use adapters::Format;

struct Fixture {
    _temp: tempfile::TempDir,
    locations: Locations,
    state: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().canonicalize().unwrap().join("a user");
        fs::create_dir(&home).unwrap();
        let locations = Locations {
            codex: home.join(".codex"),
            claude: home.join(".claude"),
            claude_config: home.join(".claude.json"),
            copilot: home.join(".copilot"),
            vscode: home.join(".config/Code/User"),
            home: home.clone(),
        };
        Self {
            _temp: temp,
            locations,
            state: home.join(".conn"),
        }
    }
    fn entry(&self, id: &str) -> Value {
        adapter(id)
            .unwrap()
            .entry("C:\\Program Files\\Conn\\conn.exe", r"\\.\pipe\conn-test")
    }
    fn install(&self, id: &str) -> Result<(), String> {
        update(
            &self.state,
            &self.locations,
            adapter(id)?,
            Some(self.entry(id)),
        )
    }
    fn remove(&self, id: &str) -> Result<(), String> {
        update(&self.state, &self.locations, adapter(id)?, None)
    }
    fn paths(&self, id: &str) -> (PathBuf, PathBuf) {
        self.locations.paths(adapter(id).unwrap())
    }
}
fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn all_clients_install_idempotently_and_remove_only_conn() {
    for a in ADAPTERS {
        let f = Fixture::new();
        let paths = f.paths(a.id);
        let original = match a.format {
            Format::Toml => "# personal\nmodel = 'my-model'\n[mcp_servers.other]\ncommand = 'keep-me'\n",
            Format::Json("servers") => "{\n // personal\n \"inputs\": [],\n \"servers\": {\"other\": {\"command\": \"keep-me\"}},\n}\n",
            _ => "{\n // personal\n \"theme\": \"dark\",\n \"mcpServers\": {\"other\": {\"command\": \"keep-me\"}},\n}\n",
        };
        write(&paths.0, original);
        f.install(a.id).unwrap();
        let installed = fs::read_to_string(&paths.0).unwrap();
        assert_eq!(
            config::read(&installed, a.format).unwrap(),
            Some(f.entry(a.id))
        );
        assert!(installed.contains("personal") && installed.contains("keep-me"));
        assert_eq!(fs::read_to_string(&paths.1).unwrap(), SKILL);
        f.install(a.id).unwrap();
        assert_eq!(
            installed,
            fs::read_to_string(&paths.0).unwrap(),
            "idempotent: {}",
            a.id
        );
        f.remove(a.id).unwrap();
        let removed = fs::read_to_string(&paths.0).unwrap();
        assert_eq!(config::read(&removed, a.format).unwrap(), None);
        assert!(removed.contains("personal") && removed.contains("keep-me"));
        assert!(!paths.1.exists());
    }
}
#[test]
fn refuses_foreign_or_modified_entries_without_touching_other_files() {
    let f = Fixture::new();
    let paths = f.paths("cursor");
    let foreign = r#"{"mcpServers":{"conn":{"command":"custom"}}}"#;
    write(&paths.0, foreign);
    assert_eq!(f.install("cursor"), Err("entry_conflict".into()));
    assert_eq!(fs::read_to_string(&paths.0).unwrap(), foreign);
    assert!(!paths.1.exists());
    fs::remove_file(&paths.0).unwrap();
    f.install("cursor").unwrap();
    write(&paths.0, foreign);
    assert_eq!(f.install("cursor"), Err("entry_conflict".into()));
    assert_eq!(f.remove("cursor"), Err("entry_conflict".into()));
    assert_eq!(fs::read_to_string(&paths.1).unwrap(), SKILL);
}
#[test]
fn edited_skill_is_never_overwritten_or_deleted() {
    let f = Fixture::new();
    f.install("codex").unwrap();
    let paths = f.paths("codex");
    let config = fs::read(&paths.0).unwrap();
    write(&paths.1, "my version");
    assert_eq!(f.install("codex"), Err("skill_conflict".into()));
    assert_eq!(f.remove("codex"), Err("skill_conflict".into()));
    assert_eq!(fs::read(&paths.0).unwrap(), config);
}
#[test]
fn shared_copilot_skill_survives_until_both_clients_are_removed() {
    let f = Fixture::new();
    f.install("copilot-vscode").unwrap();
    f.install("copilot-cli").unwrap();
    let skill = f.paths("copilot-cli").1;
    f.remove("copilot-vscode").unwrap();
    assert!(skill.exists());
    f.remove("copilot-cli").unwrap();
    assert!(!skill.exists());
}
#[test]
fn preexisting_identical_skill_is_not_claimed_by_installer() {
    let f = Fixture::new();
    let skill = f.paths("copilot-cli").1;
    write(&skill, SKILL);
    f.install("copilot-cli").unwrap();
    f.install("copilot-vscode").unwrap();
    f.remove("copilot-cli").unwrap();
    f.remove("copilot-vscode").unwrap();
    assert_eq!(fs::read_to_string(skill).unwrap(), SKILL);
}
#[test]
fn owned_setup_can_repair_missing_files_and_changed_binary_location() {
    let f = Fixture::new();
    f.install("claude-code").unwrap();
    let a = adapter("claude-code").unwrap();
    let paths = f.paths(a.id);
    fs::remove_file(&paths.1).unwrap();
    let new = a.entry(
        "/Applications/Conn.app/Contents/MacOS/conn",
        "/Users/me/.conn/conn.sock",
    );
    update(&f.state, &f.locations, a, Some(new.clone())).unwrap();
    assert_eq!(
        config::read(&fs::read_to_string(&paths.0).unwrap(), a.format).unwrap(),
        Some(new)
    );
    assert!(paths.1.exists());
    fs::remove_file(&paths.0).unwrap();
    f.install(a.id).unwrap();
}
#[test]
fn malformed_and_ambiguous_configs_are_preserved() {
    for source in [
        "{bad",
        "[]",
        r#"{"mcpServers":null}"#,
        r#"{"mcpServers":{},"mcpServers":{}}"#,
        r#"{"mcpServers":{"conn":{},"conn":{}}}"#,
    ] {
        let f = Fixture::new();
        let paths = f.paths("cursor");
        write(&paths.0, source);
        assert_eq!(f.install("cursor"), Err("invalid_config".into()));
        assert_eq!(fs::read_to_string(&paths.0).unwrap(), source);
    }
}
#[test]
fn toml_inline_tables_preserve_existing_server() {
    let source = "mcp_servers = { other = { command = 'keep' } }\n";
    let entry = json!({"command":"/bin/conn", "args":["mcp"]});
    let updated = config::edit(source, Format::Toml, Some(&entry)).unwrap();
    assert_eq!(config::read(&updated, Format::Toml).unwrap(), Some(entry));
    assert!(updated.contains("keep"));
}
#[test]
fn changed_config_is_detected_before_transaction() {
    let f = Fixture::new();
    let path = f.state.join("test.json");
    write(&path, "newer");
    let result = storage::apply(
        &[Change {
            path: path.clone(),
            before: Some(b"old".to_vec()),
            after: Some(b"replace".to_vec()),
        }],
        &f.state.join("backups"),
    );
    assert_eq!(result, Err("config_changed".into()));
    assert_eq!(fs::read_to_string(path).unwrap(), "newer");
}
#[test]
fn invalid_later_path_is_rejected_before_any_change() {
    let f = Fixture::new();
    let first = f.state.join("first");
    write(&first, "original");
    // A path with a missing parent below a regular file fails validation before any changes.
    let bad = first.join("child");
    let result = storage::apply(
        &[
            Change {
                path: first.clone(),
                before: Some(b"original".to_vec()),
                after: Some(b"updated".to_vec()),
            },
            Change {
                path: bad,
                before: None,
                after: Some(vec![]),
            },
        ],
        &f.state.join("backups"),
    );
    assert!(result.is_err());
    assert_eq!(fs::read_to_string(first).unwrap(), "original");
}
#[test]
fn changed_manifest_paths_cannot_redirect_deletion() {
    let f = Fixture::new();
    f.install("cursor").unwrap();
    let manifest = f.state.join("integrations.json");
    let mut r = registry(Some(&fs::read(&manifest).unwrap())).unwrap();
    r.clients.get_mut("cursor").unwrap().skill = f.state.join("unrelated.md");
    fs::write(manifest, serde_json::to_vec(&r).unwrap()).unwrap();
    assert_eq!(f.remove("cursor"), Err("location_changed".into()));
    assert!(f.paths("cursor").1.exists());
}
#[test]
fn concurrent_setup_lock_is_respected() {
    let f = Fixture::new();
    fs::create_dir(&f.state).unwrap();
    let lock = fs::File::create(f.state.join("integration-setup.lock")).unwrap();
    lock.lock().unwrap();
    assert_eq!(f.install("codex"), Err("setup_busy".into()));
    assert!(!f.paths("codex").0.exists());
}
#[cfg(unix)]
#[test]
fn setup_unlocks_even_while_a_duplicated_descriptor_is_open() {
    let f = Fixture::new();
    fs::create_dir(&f.state).unwrap();
    let path = f.state.join("integration-setup.lock");
    let file = fs::File::create(&path).unwrap();
    file.try_lock().unwrap();
    let lock = SetupLock(file);
    // try_clone shares the open-file description, as a forked PTY child does.
    let inherited = lock.0.try_clone().unwrap();
    let next = fs::OpenOptions::new().write(true).open(path).unwrap();
    assert!(next.try_lock().is_err(), "active setup still excludes other callers");
    drop(lock);
    next.try_lock().unwrap();
    drop(inherited);
}
#[cfg(unix)]
#[test]
fn refuses_symlink_files_and_parent_directories() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let paths = f.paths("cursor");
    let target = f.locations.home.join("target");
    write(&target, "original");
    fs::create_dir_all(paths.0.parent().unwrap()).unwrap();
    symlink(&target, &paths.0).unwrap();
    assert_eq!(f.install("cursor"), Err("unsafe_path".into()));
    fs::remove_file(&paths.0).unwrap();
    fs::remove_dir(paths.0.parent().unwrap()).unwrap();
    symlink(&f.state, paths.0.parent().unwrap()).unwrap();
    assert_eq!(f.install("cursor"), Err("unsafe_path".into()));
    assert_eq!(fs::read_to_string(target).unwrap(), "original");
}
#[cfg(unix)]
#[test]
fn private_permissions_and_backups_survive_setup() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let path = f.paths("claude-code").0;
    write(&path, "{\"account\":\"private\"}");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    f.install("claude-code").unwrap();
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let backups: Vec<_> = fs::read_dir(f.state.join("integration-backups"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert!(!backups.is_empty());
    for path in backups {
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[cfg(unix)]
#[test]
fn later_write_failure_really_rolls_back_previous_write() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let first = f.state.join("first");
    write(&first, "original");
    let blocked = f.state.join("read-only");
    fs::create_dir(&blocked).unwrap();
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o500)).unwrap();
    let result = storage::apply(
        &[
            Change {
                path: first.clone(),
                before: Some(b"original".to_vec()),
                after: Some(b"updated".to_vec()),
            },
            Change {
                path: blocked.join("second"),
                before: None,
                after: Some(b"new".to_vec()),
            },
        ],
        &f.state.join("backups"),
    );
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(result, Err("file_io".into()));
    assert_eq!(fs::read_to_string(first).unwrap(), "original");
    assert!(!blocked.join("second").exists());
}
#[test]
fn catalog_is_read_only_and_separates_registration_from_connection() {
    let f = Fixture::new();
    let socket = f.state.join("conn.sock");
    let read_catalog =
        |ids: &[String]| catalog(&f.state, &socket, Some(&f.locations.home), ids).unwrap();
    let initial = read_catalog(&[]);
    assert_eq!(initial["clients"].as_array().unwrap().len(), 5);
    assert!(!f.state.exists());
    assert!(!f.paths("cursor").0.exists());
    f.install("cursor").unwrap();
    let configured = read_catalog(&[]);
    let client = configured["clients"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "cursor")
        .unwrap();
    assert_eq!(client["managed"], true);
    assert_eq!(client["connected"], false);
    let live = read_catalog(&["cursor".into()]);
    assert_eq!(
        live["clients"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["id"] == "cursor")
            .unwrap()["connected"],
        true
    );
    fs::remove_file(f.paths("cursor").1).unwrap();
    let broken = read_catalog(&[]);
    assert_eq!(
        broken["clients"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["id"] == "cursor")
            .unwrap()["state"],
        "needs_update"
    );
}

#[test]
fn catalog_surfaces_foreign_skill_before_installation() {
    let f = Fixture::new();
    write(&f.paths("cursor").1, "personal skill");
    let result = catalog(
        &f.state,
        &f.state.join("conn.sock"),
        Some(&f.locations.home),
        &[],
    )
    .unwrap();
    let client = result["clients"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "cursor")
        .unwrap();
    assert_eq!(client["state"], "conflict");
    assert_eq!(client["error"], "skill_conflict");
    assert!(!f.paths("cursor").0.exists());
}
