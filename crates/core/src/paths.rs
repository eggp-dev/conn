//! Well-known file locations.

use std::path::PathBuf;

/// `~/.conn`, created on demand.
pub fn config_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = home.join(".conn");
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
        // One-time migration from the pre-rename location.
        let old = home.join(".shared-shell");
        for f in ["policy.yaml", "audit.jsonl"] {
            if old.join(f).exists() && !dir.join(f).exists() {
                let _ = std::fs::copy(old.join(f), dir.join(f));
            }
        }
    }
    dir
}

pub fn policy_path() -> PathBuf {
    config_dir().join("policy.yaml")
}

pub fn audit_path() -> PathBuf {
    config_dir().join("audit.jsonl")
}

/// `$CONN_SOCKET`, else `~/.conn/conn.sock`.
///
/// Deliberately not under `$TMPDIR`: agent harnesses often launch MCP servers with a
/// scrubbed environment, and the socket must resolve to the same place from every
/// process of the same user.
pub fn socket_path() -> PathBuf {
    if let Some(p) = std::env::var_os("CONN_SOCKET").filter(|s| !s.is_empty()) {
        return PathBuf::from(p);
    }
    let dir = config_dir();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    #[cfg(unix)] { dir.join("conn.sock") }
    #[cfg(windows)] { crate::transport::pipe_name(&dir.join("conn.sock")).into() }
}
