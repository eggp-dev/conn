//! Guarded, atomic file replacement. The caller supplies before/after snapshots.
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

pub fn safe_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("unsafe_path".into());
    }
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(m) if m.file_type().is_symlink() => return Err("unsafe_path".into()),
            Ok(m) if ancestor != path && !m.is_dir() => return Err("unsafe_path".into()),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("file_io".into()),
        }
    }
    Ok(())
}
pub fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    safe_path(path)?;
    match fs::metadata(path) {
        Ok(m) if !m.is_file() || m.len() > 8 * 1024 * 1024 => Err("invalid_file".into()),
        Ok(_) => fs::read(path).map(Some).map_err(|_| "file_io".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("file_io".into()),
    }
}
pub struct Change {
    pub path: PathBuf,
    pub before: Option<Vec<u8>>,
    pub after: Option<Vec<u8>>,
}
fn replace(path: &Path, content: Option<&[u8]>) -> Result<(), String> {
    safe_path(path)?;
    let Some(bytes) = content else {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("file_io".into()),
        };
    };
    let parent = path.parent().ok_or("unsafe_path")?;
    fs::create_dir_all(parent).map_err(|_| "file_io")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|_| "file_io")?;
    temp.write_all(bytes).map_err(|_| "file_io")?;
    if let Ok(meta) = fs::metadata(path) {
        temp.as_file()
            .set_permissions(meta.permissions())
            .map_err(|_| "file_io")?;
    }
    temp.as_file().sync_all().map_err(|_| "file_io")?;
    temp.persist(path).map_err(|_| "file_io")?;
    Ok(())
}
pub fn apply(changes: &[Change], backup_dir: &Path) -> Result<(), String> {
    for c in changes {
        if read(&c.path)? != c.before {
            return Err("config_changed".into());
        }
    }
    // Recovery copies remain private, including Claude's config with account state.
    safe_path(backup_dir)?;
    fs::create_dir_all(backup_dir).map_err(|_| "file_io")?;
    for c in changes.iter().filter(|c| c.before != c.after) {
        if let Some(bytes) = &c.before {
            let mut backup = tempfile::Builder::new()
                .prefix(&format!(
                    "{}-before-",
                    c.path.file_name().unwrap_or_default().to_string_lossy()
                ))
                .suffix(".bak")
                .tempfile_in(backup_dir)
                .map_err(|_| "file_io")?;
            backup.write_all(bytes).map_err(|_| "file_io")?;
            backup.as_file().sync_all().map_err(|_| "file_io")?;
            backup.keep().map_err(|_| "file_io")?;
        }
    }
    let mut applied: Vec<&Change> = Vec::new();
    for c in changes.iter().filter(|c| c.before != c.after) {
        let result = match read(&c.path) {
            Ok(current) if current == c.before => replace(&c.path, c.after.as_deref()),
            Ok(_) => Err("config_changed".into()),
            Err(e) => Err(e),
        };
        if let Err(e) = result {
            let mut rollback_failed = false;
            for previous in applied.into_iter().rev() {
                if read(&previous.path).is_ok_and(|v| v == previous.after) {
                    rollback_failed |= replace(&previous.path, previous.before.as_deref()).is_err();
                } else {
                    rollback_failed = true;
                }
            }
            return Err(if rollback_failed {
                "recovery_needed".into()
            } else {
                e
            });
        }
        applied.push(c);
    }
    Ok(())
}
