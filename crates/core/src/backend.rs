//! Execution adapters. Profiles describe a target; launch plans contain argv, never a local shell wrapper.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    #[default]
    Local,
    Wsl,
    Ssh,
    Docker,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellKind {
    #[default]
    Posix,
    Fish,
    PowerShell,
    Cmd,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub backend: BackendKind,
    #[serde(default)]
    pub shell: ShellKind,
    /// Executable on the target. Empty for the SSH server's default shell.
    #[serde(default)]
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// SSH host/alias, WSL distribution, or Docker container.
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
}
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Availability {
    pub available: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct LaunchPlan {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
}

/// An adapter can add a new target without taking over Session's authority or audit logic.
pub trait Backend {
    fn prepare(&self, profile: &Profile) -> Result<LaunchPlan, String>;
}
pub struct LocalBackend;
pub struct WslBackend;
pub struct SshBackend;
pub struct DockerBackend;

pub fn shell_kind(program: &str) -> ShellKind {
    let name = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase();
    match name.trim_end_matches(".exe") {
        "sh" | "bash" | "zsh" | "dash" | "ksh" => ShellKind::Posix,
        "fish" => ShellKind::Fish,
        "pwsh" | "powershell" => ShellKind::PowerShell,
        "cmd" => ShellKind::Cmd,
        _ => ShellKind::Custom,
    }
}
pub fn default_args(shell: ShellKind) -> Vec<String> {
    match shell {
        ShellKind::Posix | ShellKind::Fish => vec!["-l".into()],
        ShellKind::PowerShell => vec!["-NoLogo".into()],
        ShellKind::Cmd => vec!["/D".into()],
        ShellKind::Custom => vec![],
    }
}
pub fn expand_local(path: &str) -> PathBuf {
    if path == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}
pub fn executable(program: &str) -> Option<PathBuf> {
    executable_in(program, std::env::var_os("PATH").as_deref(), None)
}
fn executable_in(
    program: &str,
    search_path: Option<&std::ffi::OsStr>,
    cwd: Option<&Path>,
) -> Option<PathBuf> {
    fn is_executable(p: &Path) -> bool {
        if !p.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            return p
                .metadata()
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false);
        }
        #[cfg(not(unix))]
        {
            true
        }
    }
    let p = expand_local(program);
    if p.is_absolute() || p.components().count() > 1 {
        let p = if p.is_relative() {
            cwd.map(|d| d.join(&p)).unwrap_or(p)
        } else {
            p
        };
        return is_executable(&p)
            .then(|| std::path::absolute(p).ok())
            .flatten();
    }
    let mut names = vec![program.to_string()];
    #[cfg(windows)]
    if !program.to_ascii_lowercase().ends_with(".exe") {
        names.push(format!("{program}.exe"));
    }
    #[cfg(not(windows))]
    let _ = &mut names;
    for dir in std::env::split_paths(search_path?) {
        let dir = if dir.is_relative() {
            cwd.map(|d| d.join(&dir)).unwrap_or(dir)
        } else {
            dir
        };
        for name in &names {
            let p = dir.join(name);
            if is_executable(&p) {
                return std::path::absolute(p).ok();
            }
        }
    }
    None
}

impl Profile {
    pub fn local(id: String, program: String) -> Self {
        let shell = shell_kind(&program);
        Self {
            name: program
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(&program)
                .to_string(),
            id,
            enabled: true,
            backend: BackendKind::Local,
            shell,
            program,
            args: default_args(shell),
            cwd: None,
            env: BTreeMap::new(),
            target: None,
            port: None,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || self.id.len() > 64
            || !self
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
        {
            return Err(
                "Profile id must contain 1–64 letters, digits, hyphens or underscores".into(),
            );
        }
        if self.name.trim().is_empty() || self.name.len() > 128 {
            return Err("Profile name must contain 1–128 characters".into());
        }
        if self.program.contains(['\0', '\r', '\n']) || self.args.iter().any(|a| a.contains('\0')) {
            return Err("Invalid executable or argument".into());
        }
        if self.backend != BackendKind::Ssh && self.program.trim().is_empty() {
            return Err("Choose a shell executable".into());
        }
        if self.env.iter().any(|(k, v)| {
            k.is_empty()
                || k.starts_with('-')
                || k.contains(['=', '\0', '\r', '\n'])
                || v.contains('\0')
        }) {
            return Err(
                "Environment names must be nonempty, not start with -, and cannot contain = or control characters"
                    .into(),
            );
        }
        if self.cwd.as_ref().is_some_and(|c| c.contains('\0')) {
            return Err("Invalid working directory".into());
        }
        if self.backend != BackendKind::Local {
            if self.program.starts_with('-') {
                return Err("The target executable cannot be an option".into());
            }
            let target = self.target.as_deref().unwrap_or("");
            if target.trim().is_empty()
                || target.starts_with('-')
                || target.chars().any(char::is_control)
            {
                return Err("Choose a valid target (not an option)".into());
            }
            if self.backend == BackendKind::Ssh
                && (target.chars().any(char::is_whitespace)
                    || !target
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "._-@:%[]".contains(c)))
            {
                return Err("Use an SSH host, user@host, or SSH config alias".into());
            }
        }
        if self.port == Some(0) || (self.port.is_some() && self.backend != BackendKind::Ssh) {
            return Err("Port is only valid for SSH and must be 1–65535".into());
        }
        if self.backend == BackendKind::Ssh {
            if self.program.is_empty() {
                if !self.args.is_empty()
                    || !self.env.is_empty()
                    || self.cwd.as_ref().is_some_and(|s| !s.is_empty())
                {
                    return Err("For the SSH server's default shell, configure its startup directory/environment on the server".into());
                }
            } else if self.shell != ShellKind::Posix {
                return Err("An explicit SSH startup command currently requires a POSIX login shell; leave executable empty to use any server's default shell".into());
            }
        }
        Ok(())
    }
    pub fn prepare(&self) -> Result<LaunchPlan, String> {
        self.validate()?;
        if !self.enabled {
            return Err(format!("Profile '{}' is disabled", self.name));
        }
        match self.backend {
            BackendKind::Local => LocalBackend.prepare(self),
            BackendKind::Wsl => WslBackend.prepare(self),
            BackendKind::Ssh => SshBackend.prepare(self),
            BackendKind::Docker => DockerBackend.prepare(self),
        }
    }
    /// Discovery only checks local prerequisites. A successful probe checks the target.
    pub fn availability(&self) -> Availability {
        let result = self.prepare().and_then(|p| {
            if executable(&p.program).is_none() {
                return Err(format!("Executable not found: {}", p.program));
            }
            if let Some(cwd) = p.cwd {
                if !cwd.is_dir() {
                    return Err(format!("Working directory not found: {}", cwd.display()));
                }
            }
            Ok(())
        });
        match result {
            Ok(()) => Availability {
                available: true,
                message: if self.backend == BackendKind::Local {
                    "Ready"
                } else {
                    "Launcher available; test the target connection"
                }
                .into(),
            },
            Err(message) => Availability {
                available: false,
                message,
            },
        }
    }
    pub fn needs_review(&self) -> bool {
        self.backend != BackendKind::Local || self.shell != ShellKind::Posix
    }
}

impl Backend for LocalBackend {
    fn prepare(&self, p: &Profile) -> Result<LaunchPlan, String> {
        let cwd = p.cwd.as_deref().filter(|s| !s.is_empty()).map(expand_local);
        let inherited_path = std::env::var_os("PATH");
        let configured_path = p
            .env
            .iter()
            .find(|(k, _)| {
                if cfg!(windows) {
                    k.eq_ignore_ascii_case("PATH")
                } else {
                    k.as_str() == "PATH"
                }
            })
            .map(|(_, v)| std::ffi::OsStr::new(v));
        let program = executable_in(
            &p.program,
            configured_path.or(inherited_path.as_deref()),
            cwd.as_deref(),
        )
        .ok_or_else(|| format!("Executable not found: {}", p.program))?;
        Ok(LaunchPlan {
            program: program.to_string_lossy().into(),
            args: p.args.clone(),
            cwd,
            env: p.env.clone(),
        })
    }
}
impl Backend for WslBackend {
    fn prepare(&self, p: &Profile) -> Result<LaunchPlan, String> {
        if !cfg!(windows) {
            return Err("WSL profiles can be launched on Windows only".into());
        }
        let mut args = vec![
            "--distribution".into(),
            p.target.clone().unwrap_or_default(),
        ];
        if let Some(cwd) = p.cwd.as_ref().filter(|s| !s.is_empty()) {
            args.extend(["--cd".into(), cwd.clone()]);
        }
        args.push("--exec".into());
        if !p.env.is_empty() {
            args.push("/usr/bin/env".into());
            for (k, v) in &p.env {
                args.push(format!("{k}={v}"));
            }
        }
        args.push(p.program.clone());
        args.extend(p.args.clone());
        Ok(LaunchPlan {
            program: "wsl.exe".into(),
            args,
            cwd: None,
            env: BTreeMap::new(),
        })
    }
}
pub fn quote_posix(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
impl Backend for SshBackend {
    fn prepare(&self, p: &Profile) -> Result<LaunchPlan, String> {
        let mut args = vec!["-tt".into(), "-o".into(), "ServerAliveInterval=30".into()];
        if let Some(port) = p.port {
            args.extend(["-p".into(), port.to_string()]);
        }
        args.push(p.target.clone().unwrap_or_default());
        if !p.program.is_empty() {
            let mut words = vec!["exec".into(), "env".into()];
            for (k, v) in &p.env {
                words.push(quote_posix(&format!("{k}={v}")));
            }
            words.push(quote_posix(&p.program));
            words.extend(p.args.iter().map(|s| quote_posix(s)));
            let prefix = p
                .cwd
                .as_ref()
                .filter(|s| !s.is_empty())
                .map(|d| format!("cd -- {} && ", quote_posix(d)))
                .unwrap_or_default();
            args.push(format!("{prefix}{}", words.join(" ")));
        }
        Ok(LaunchPlan {
            program: "ssh".into(),
            args,
            cwd: None,
            env: BTreeMap::new(),
        })
    }
}
impl Backend for DockerBackend {
    fn prepare(&self, p: &Profile) -> Result<LaunchPlan, String> {
        let mut args = vec!["exec".into(), "-it".into()];
        if let Some(cwd) = p.cwd.as_ref().filter(|s| !s.is_empty()) {
            args.extend(["--workdir".into(), cwd.clone()]);
        }
        for (k, v) in &p.env {
            args.extend(["--env".into(), format!("{k}={v}")]);
        }
        args.push(p.target.clone().unwrap_or_default());
        args.push(p.program.clone());
        args.extend(p.args.clone());
        Ok(LaunchPlan {
            program: "docker".into(),
            args,
            cwd: None,
            env: BTreeMap::new(),
        })
    }
}
