//! Agent setup uses the shipped CLI directly; installing a command on PATH is optional.
#[cfg(any(target_os = "linux", test))]
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[cfg(any(target_os = "linux", test))]
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
#[cfg(any(target_os = "linux", test))]
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

fn executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn sidecar_next_to(exe: &Path) -> Option<PathBuf> {
    // Tauri strips the target-triple suffix when packaging externalBin. On all
    // three platforms the CLI lives beside the desktop executable, including
    // /usr/bin in a .deb and Contents/MacOS in a macOS app bundle.
    let path = exe
        .parent()?
        .join(if cfg!(windows) { "conn.exe" } else { "conn" });
    (executable_file(&path) && path.canonicalize().ok() != exe.canonicalize().ok()).then_some(path)
}

fn sidecar_path() -> Option<PathBuf> {
    sidecar_next_to(&std::env::current_exe().ok()?)
}

fn on_path() -> Option<PathBuf> {
    let path = conn_core::backend::executable("conn")?;
    Some(if path.is_absolute() {
        path
    } else {
        std::env::current_dir().ok()?.join(path)
    })
}

fn path_contains(dir: &Path, search_path: &OsStr) -> bool {
    std::env::split_paths(search_path).any(|p| {
        p == dir || matches!((p.canonicalize(), dir.canonicalize()), (Ok(a), Ok(b)) if a == b)
    })
}

fn install_directories() -> Vec<PathBuf> {
    let search_path = std::env::var_os("PATH").unwrap_or_default();
    let mut candidates = Vec::new();
    if let Some(home) = conn_core::profiles::home_dir() {
        candidates.push(home.join(".local/bin"));
    }
    candidates.push(PathBuf::from("/usr/local/bin"));
    candidates
        .into_iter()
        .filter(|p| path_contains(p, &search_path))
        .collect()
}

pub(crate) fn cli_status() -> Value {
    let bundled = sidecar_path();
    let on_path = on_path();
    json!({
        "canConfigure": bundled.is_some() || on_path.is_some(),
        "canInstall": cfg!(unix) && bundled.is_some() && on_path.is_none() && !install_directories().is_empty(),
        "bundled": bundled,
        "onPath": on_path,
    })
}

fn configuration_text(command: &str, endpoint: &str) -> Value {
    let args = json!(["--socket", endpoint, "mcp"]);
    let mcp = json!({ "mcpServers": { "conn": { "command": command, "args": args } } });
    // JSON strings/arrays also form valid TOML basic strings/arrays. In
    // particular Windows backslashes, spaces and quotes retain their values.
    let toml = format!(
        "[mcp_servers.conn]\ncommand = {}\nargs = {}\n",
        json!(command),
        args
    );
    json!({
        "command": command,
        "endpoint": endpoint,
        "mcpJson": serde_json::to_string_pretty(&mcp).expect("serializing configuration"),
        "codexToml": toml,
    })
}

pub(crate) fn configuration_command_path(config_dir: &Path) -> Option<PathBuf> {
    if let Some(bundled) = sidecar_path() {
        #[cfg(target_os = "linux")]
        if std::env::var_os("APPIMAGE").is_some() { return Some(config_dir.join("agent-bin/conn")); }
        let _ = config_dir;
        return Some(bundled);
    }
    on_path()
}

fn cli_for_configuration(config_dir: &Path) -> Result<PathBuf, String> {
    if let Some(bundled) = sidecar_path() {
        // AppImage mounts are temporary. Copy only on this explicit setup
        // action; opening Settings and cli_status remain read-only.
        #[cfg(target_os = "linux")]
        if std::env::var_os("APPIMAGE").is_some() {
            return stage_appimage_cli(config_dir, &bundled);
        }
        let _ = config_dir;
        return Ok(bundled);
    }
    on_path().ok_or_else(|| "The Conn CLI could not be found. Reinstall the desktop package or install the CLI release, then try again.".into())
}

pub(crate) fn configuration(config_dir: &Path, socket: &Path) -> Result<Value, String> {
    let command = cli_for_configuration(config_dir)?;
    let command = command
        .to_str()
        .ok_or("The CLI path contains unsupported characters")?;
    let socket = if socket.is_absolute() {
        socket.to_path_buf()
    } else {
        std::path::absolute(socket).map_err(|e| e.to_string())?
    };
    let endpoint = socket
        .to_str()
        .ok_or("The connection endpoint contains unsupported characters")?;
    Ok(configuration_text(command, endpoint))
}

#[cfg(any(target_os = "linux", test))]
#[derive(Serialize, Deserialize)]
struct StagedCli {
    owner: String,
    source: PathBuf,
    sha256: String,
}

#[cfg(any(target_os = "linux", test))]
fn file_hash(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(any(target_os = "linux", test))]
fn regular_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|m| m.file_type().is_file())
        .unwrap_or(false)
}

/// A stable copy for AppImage clients. Only files matching our ownership record
/// can be replaced. Never follow a recorded source path or a destination symlink.
#[cfg(any(target_os = "linux", test))]
fn stage_appimage_cli(config_dir: &Path, source: &Path) -> Result<PathBuf, String> {
    // Serialize repeated setup actions within this process, including API calls.
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK.lock().map_err(|e| e.to_string())?;
    let config_dir = std::path::absolute(config_dir).map_err(|e| e.to_string())?;
    let dir = config_dir.join("agent-bin");
    let binary = dir.join("conn");
    let record = dir.join("conn.json");
    match std::fs::symlink_metadata(&dir) {
        Ok(m) if !m.file_type().is_dir() => {
            return Err(format!(
                "Refusing to change {}: this is not a Conn setup directory",
                dir.display()
            ))
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
            std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
        }
        Err(e) => return Err(e.to_string()),
    }
    let exists = std::fs::symlink_metadata(&binary).is_ok();
    let record_exists = std::fs::symlink_metadata(&record).is_ok();
    if exists || record_exists {
        let owned = regular_file(&binary)
            && regular_file(&record)
            && std::fs::read(&record)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<StagedCli>(&bytes).ok())
                .is_some_and(|r| {
                    r.owner == "conn-agent-setup-v1"
                        && file_hash(&binary).is_ok_and(|h| h == r.sha256)
                });
        if !owned {
            return Err(format!(
                "Refusing to replace modified or unrecognized files in {}",
                dir.display()
            ));
        }
    }
    let hash = file_hash(source)?;
    if exists && file_hash(&binary)? == hash && executable_file(&binary) {
        return Ok(binary);
    }
    let mut staged = tempfile::NamedTempFile::new_in(&dir).map_err(|e| e.to_string())?;
    std::fs::copy(source, staged.path()).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        staged
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    staged.as_file_mut().flush().map_err(|e| e.to_string())?;
    staged.as_file().sync_all().map_err(|e| e.to_string())?;
    let mut manifest = tempfile::NamedTempFile::new_in(&dir).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(
        &mut manifest,
        &StagedCli {
            owner: "conn-agent-setup-v1".into(),
            source: source.into(),
            sha256: hash,
        },
    )
    .map_err(|e| e.to_string())?;
    manifest.as_file().sync_all().map_err(|e| e.to_string())?;
    if exists {
        staged.persist(&binary).map_err(|e| e.to_string())?;
        manifest.persist(&record).map_err(|e| e.to_string())?;
    } else {
        staged
            .persist_noclobber(&binary)
            .map_err(|e| e.to_string())?;
        manifest
            .persist_noclobber(&record)
            .map_err(|e| e.to_string())?;
    }
    Ok(binary)
}

#[cfg(unix)]
fn link_cli(source: &Path, candidates: &[PathBuf]) -> Result<String, String> {
    for dir in candidates {
        if std::fs::create_dir_all(dir).is_err() {
            continue;
        }
        let dst = dir.join("conn");
        if let Ok(meta) = std::fs::symlink_metadata(&dst) {
            if meta.file_type().is_symlink()
                && dst.canonicalize().ok() == source.canonicalize().ok()
            {
                return Ok(dst.display().to_string());
            }
            // An existing executable, directory or user symlink is never removed.
            continue;
        }
        if std::os::unix::fs::symlink(source, &dst).is_ok() {
            return Ok(dst.display().to_string());
        }
    }
    Err("No free, writable CLI location is already on PATH. Use Copy agent configuration in Settings → Agents instead; existing files were preserved.".into())
}

#[cfg(unix)]
pub(crate) fn install_cli(config_dir: &Path) -> Result<String, String> {
    link_cli(&cli_for_configuration(config_dir)?, &install_directories())
}

#[cfg(windows)]
pub(crate) fn install_cli(_config_dir: &Path) -> Result<String, String> {
    Err("Use Copy agent configuration in Settings → Agents. The bundled CLI works without PATH setup.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_preserves_windows_paths_and_current_named_pipe() {
        let command = r#"C:\Users\Sam Lee\AppData\Local\Conn\conn.exe"#;
        let endpoint = r#"\\.\pipe\conn-Sam-Lee"#;
        let config = configuration_text(command, endpoint);
        let parsed: Value = serde_json::from_str(config["mcpJson"].as_str().unwrap()).unwrap();
        assert_eq!(parsed["mcpServers"]["conn"]["command"], command);
        assert_eq!(
            parsed["mcpServers"]["conn"]["args"],
            json!(["--socket", endpoint, "mcp"])
        );
        let toml = config["codexToml"].as_str().unwrap();
        let mut lines = toml.lines();
        assert_eq!(lines.next(), Some("[mcp_servers.conn]"));
        let command_value: String =
            serde_json::from_str(lines.next().unwrap().strip_prefix("command = ").unwrap())
                .unwrap();
        let args_value: Value =
            serde_json::from_str(lines.next().unwrap().strip_prefix("args = ").unwrap()).unwrap();
        assert_eq!(command_value, command);
        assert_eq!(args_value, json!(["--socket", endpoint, "mcp"]));
    }

    #[test]
    fn configuration_keeps_custom_unix_socket_and_paths_with_quotes() {
        let config = configuration_text(
            "/Applications/Conn \"Preview\"/conn",
            "/tmp/conn custom/socket",
        );
        let parsed: Value = serde_json::from_str(config["mcpJson"].as_str().unwrap()).unwrap();
        assert_eq!(
            parsed["mcpServers"]["conn"]["command"],
            "/Applications/Conn \"Preview\"/conn"
        );
        assert_eq!(
            parsed["mcpServers"]["conn"]["args"][1],
            "/tmp/conn custom/socket"
        );
    }

    #[test]
    fn appimage_cli_path_stays_valid_after_mount_disappears_and_updates_if_owned() {
        let root = tempfile::tempdir().unwrap();
        let mount = tempfile::tempdir().unwrap();
        let source = mount.path().join("conn");
        std::fs::write(&source, b"first bundled CLI").unwrap();
        let path = stage_appimage_cli(root.path(), &source).unwrap();
        assert!(path.is_absolute());
        drop(mount);
        assert_eq!(std::fs::read(&path).unwrap(), b"first bundled CLI");
        let next_mount = tempfile::tempdir().unwrap();
        let next_source = next_mount.path().join("conn");
        std::fs::write(&next_source, b"updated bundled CLI").unwrap();
        assert_eq!(stage_appimage_cli(root.path(), &next_source).unwrap(), path);
        assert_eq!(std::fs::read(&path).unwrap(), b"updated bundled CLI");
        assert!(executable_file(&path));
    }

    #[test]
    fn appimage_cli_never_replaces_unrecognized_or_modified_files() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        std::fs::write(&source, b"bundled CLI").unwrap();
        let config = root.path().join("config");
        let target = stage_appimage_cli(&config, &source).unwrap();
        std::fs::write(&target, b"user replacement").unwrap();
        assert!(stage_appimage_cli(&config, &source).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"user replacement");
        std::fs::remove_file(config.join("agent-bin/conn.json")).unwrap();
        assert!(stage_appimage_cli(&config, &source).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"user replacement");
    }

    #[cfg(unix)]
    #[test]
    fn setup_preserves_foreign_symlinks_and_discovers_packaged_sidecar() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("usr/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let source = bin.join("conn");
        let desktop = bin.join("conn-desktop");
        std::fs::write(&source, b"bundled CLI").unwrap();
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(&desktop, b"desktop").unwrap();
        assert_eq!(sidecar_next_to(&desktop), Some(source.clone()));
        assert!(sidecar_next_to(&source).is_none());
        let destination = root.path().join("custom-bin");
        std::fs::create_dir(&destination).unwrap();
        let foreign = root.path().join("my-cli");
        std::fs::write(&foreign, b"user CLI").unwrap();
        symlink(&foreign, destination.join("conn")).unwrap();
        assert!(link_cli(&source, &[destination.clone()]).is_err());
        assert_eq!(
            std::fs::read_link(destination.join("conn")).unwrap(),
            foreign
        );
        assert!(path_contains(
            &bin,
            std::env::join_paths([&destination, &bin])
                .unwrap()
                .as_os_str()
        ));
        assert!(!path_contains(
            &root.path().join("not-on-path"),
            bin.as_os_str()
        ));
        let config = root.path().join("config");
        std::fs::create_dir(&config).unwrap();
        symlink(&destination, config.join("agent-bin")).unwrap();
        assert!(stage_appimage_cli(&config, &source).is_err());
        assert_eq!(
            std::fs::read_link(destination.join("conn")).unwrap(),
            foreign
        );
    }
}
