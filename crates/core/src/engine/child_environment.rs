//! AppImage's runtime paths belong to the desktop, not its host-side PTY child.
//! Restore inherited paths before applying explicit engine/profile overrides.
use portable_pty::CommandBuilder;

pub(super) fn restore_host_environment(cmd: &mut CommandBuilder) {
    #[cfg(target_os = "linux")]
    restore_appimage_paths(cmd);
    #[cfg(not(target_os = "linux"))]
    let _ = cmd;
}

#[cfg(target_os = "linux")]
fn restore_appimage_paths(cmd: &mut CommandBuilder) {
    use std::path::{Component, Path, PathBuf};
    fn normalized(path: &Path) -> PathBuf {
        let mut result = PathBuf::new();
        for part in path.components() {
            match part {
                Component::ParentDir => {
                    result.pop();
                }
                Component::CurDir => {}
                _ => result.push(part.as_os_str()),
            }
        }
        result
    }
    if cmd.get_env("APPIMAGE").is_none_or(|value| value.is_empty()) {
        return;
    }
    let Some(appdir) = cmd
        .get_env("APPDIR")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    else {
        return;
    };
    let appdir = normalized(&appdir);
    // A malformed root must never strip the host's entire environment.
    if appdir.parent().is_none() {
        return;
    }
    let bundled = |path: &Path| path.is_absolute() && normalized(path).starts_with(&appdir);
    for key in [
        "LD_LIBRARY_PATH",
        "PATH",
        "XDG_DATA_DIRS",
        "GI_TYPELIB_PATH",
        "GTK_PATH",
        "GIO_EXTRA_MODULES",
    ] {
        let Some(value) = cmd.get_env(key) else {
            continue;
        };
        let paths: Vec<_> = std::env::split_paths(value).collect();
        if !paths.iter().any(|path| bundled(path)) {
            continue;
        }
        let retained: Vec<_> = paths.into_iter().filter(|path| !bundled(path)).collect();
        if retained.is_empty() {
            cmd.env_remove(key);
        } else if let Ok(value) = std::env::join_paths(retained) {
            cmd.env(key, value);
        }
    }
    for key in [
        "GTK_DATA_PREFIX",
        "GTK_EXE_PREFIX",
        "GSETTINGS_SCHEMA_DIR",
        "GTK_IM_MODULE_FILE",
        "GDK_PIXBUF_MODULE_FILE",
        "GDK_PIXBUF_MODULEDIR",
    ] {
        if cmd
            .get_env(key)
            .is_some_and(|value| bundled(Path::new(value)))
        {
            cmd.env_remove(key);
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    fn appimage() -> CommandBuilder {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.env_clear();
        cmd.env("APPIMAGE", "/opt/Conn.AppImage");
        cmd.env("APPDIR", "/tmp/conn-bundle");
        cmd
    }
    #[test]
    fn keeps_host_paths_empty_segments_and_similar_prefixes() {
        let mut cmd = appimage();
        cmd.env("LD_LIBRARY_PATH", "/tmp/conn-bundle/usr/lib:/opt/user/lib::/tmp/conn-bundle-other/lib:/tmp/conn-bundle/../host/lib");
        restore_host_environment(&mut cmd);
        assert_eq!(
            cmd.get_env("LD_LIBRARY_PATH").unwrap(),
            "/opt/user/lib::/tmp/conn-bundle-other/lib:/tmp/conn-bundle/../host/lib"
        );
    }
    #[test]
    fn removes_bundle_only_runtime_settings() {
        let mut cmd = appimage();
        cmd.env(
            "LD_LIBRARY_PATH",
            "/tmp/conn-bundle//usr/lib:/tmp/conn-bundle/usr/lib/../lib64",
        );
        cmd.env("GIO_EXTRA_MODULES", "/tmp/conn-bundle/usr/lib/gio/modules");
        cmd.env(
            "GTK_IM_MODULE_FILE",
            "/tmp/conn-bundle/usr/lib/immodules.cache",
        );
        cmd.env("GSETTINGS_SCHEMA_DIR", "/opt/user/schemas");
        cmd.env("PATH", "/tmp/conn-bundle/usr/bin:/usr/local/bin:/usr/bin");
        restore_host_environment(&mut cmd);
        assert!(cmd.get_env("LD_LIBRARY_PATH").is_none());
        assert!(cmd.get_env("GIO_EXTRA_MODULES").is_none());
        assert!(cmd.get_env("GTK_IM_MODULE_FILE").is_none());
        assert_eq!(
            cmd.get_env("GSETTINGS_SCHEMA_DIR").unwrap(),
            "/opt/user/schemas"
        );
        assert_eq!(cmd.get_env("PATH").unwrap(), "/usr/local/bin:/usr/bin");
    }
    #[test]
    fn ordinary_launches_and_malformed_appdirs_are_unchanged() {
        for appdir in [None, Some("/"), Some("relative/bundle")] {
            let mut cmd = appimage();
            cmd.env("LD_LIBRARY_PATH", "/tmp/conn-bundle/usr/lib:/opt/user/lib");
            if let Some(path) = appdir {
                cmd.env("APPDIR", path);
            } else {
                cmd.env_remove("APPIMAGE");
            }
            restore_host_environment(&mut cmd);
            assert_eq!(
                cmd.get_env("LD_LIBRARY_PATH").unwrap(),
                "/tmp/conn-bundle/usr/lib:/opt/user/lib"
            );
        }
    }
}
