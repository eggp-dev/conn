//! Versioned, atomically written user profiles, shared by CLI and desktop.
use crate::backend::{executable, Availability, BackendKind, Profile};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profiles {
    pub version: u32,
    #[serde(default)]
    pub revision: u64,
    pub default_profile: String,
    pub profiles: Vec<Profile>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub config: Profiles,
    pub availability: std::collections::BTreeMap<String, Availability>,
}
pub fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}
pub fn path() -> PathBuf {
    crate::paths::config_dir().join("profiles.json")
}

pub fn discover() -> Vec<Profile> {
    let mut programs = vec![crate::engine::default_shell()];
    if cfg!(windows) {
        programs.extend(["pwsh.exe", "powershell.exe", "cmd.exe", "bash.exe"].map(String::from));
    } else {
        programs.extend(["bash", "zsh", "fish", "sh", "pwsh"].map(String::from));
    }
    #[cfg(windows)]
    for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base) = std::env::var_os(variable) {
            programs.push(
                PathBuf::from(base)
                    .join("Git/bin/bash.exe")
                    .to_string_lossy()
                    .into(),
            );
        }
    }
    let mut seen = HashSet::new();
    let mut found = vec![];
    for program in programs {
        if let Some(p) = executable(&program) {
            let canonical = p.canonicalize().unwrap_or_else(|_| p.clone());
            if !seen.insert(canonical) {
                continue;
            }
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            let id = format!(
                "local-{}",
                name.trim_end_matches(".exe")
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                    .collect::<String>()
            );
            if found.iter().any(|p: &Profile| p.id == id) {
                continue;
            }
            found.push(Profile::local(id, p.to_string_lossy().into()));
        }
    }
    #[cfg(windows)]
    if executable("wsl.exe").is_some() {
        if let Ok(out) = run_bounded("wsl.exe", &["--list".into(), "--quiet".into()], 5) {
            let text = if out.contains(&0) {
                String::from_utf16_lossy(
                    &out.chunks_exact(2)
                        .map(|b| u16::from_le_bytes([b[0], b[1]]))
                        .collect::<Vec<_>>(),
                )
            } else {
                String::from_utf8_lossy(&out).into()
            };
            for name in text
                .lines()
                .map(|s| s.trim().trim_start_matches('\u{feff}'))
                .filter(|s| !s.is_empty())
            {
                let id = name.as_bytes().iter().fold(0xcbf29ce484222325u64, |h, b| {
                    (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
                });
                let mut p = Profile::local(format!("wsl-{id:016x}"), "/bin/bash".into());
                p.name = format!("WSL · {name}");
                p.backend = BackendKind::Wsl;
                p.target = Some(name.into());
                found.push(p);
            }
        }
    }
    found
}
impl Profiles {
    pub fn detected() -> Self {
        let mut profiles = discover();
        if profiles.is_empty() {
            profiles.push(Profile::local(
                "local-default".into(),
                crate::engine::default_shell(),
            ));
        }
        Self {
            version: 1,
            revision: 0,
            default_profile: profiles[0].id.clone(),
            profiles,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported profiles version".into());
        }
        if self.profiles.len() > 128 {
            return Err("At most 128 profiles are supported".into());
        }
        let mut ids = HashSet::new();
        for p in &self.profiles {
            p.validate()?;
            if !ids.insert(&p.id) {
                return Err(format!("Duplicate profile id: {}", p.id));
            }
        }
        let default = self
            .profiles
            .iter()
            .find(|p| p.id == self.default_profile)
            .ok_or("Choose a default profile")?;
        if !default.enabled {
            return Err("The default profile must be enabled".into());
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read(path) {
            Ok(bytes) => {
                let p: Self = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("Invalid profiles file: {e}"))?;
                p.validate()?;
                Ok(p)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::detected()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn save(&self, path: &Path) -> Result<Self, String> {
        self.validate()?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        // Lock the file across CLI and desktop processes, rather than only a Rust mutex.
        let lock_path = path.with_extension("lock");
        let _lock = ConfigLock::acquire(lock_path)?;
        let previous = Self::load(path)?;
        if previous.revision != self.revision {
            return Err("Profiles changed in another window or CLI. Reload before saving.".into());
        }
        let mut next = self.clone();
        next.revision += 1;
        let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tmp.as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
        }
        tmp.write_all(&serde_json::to_vec_pretty(&next).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        tmp.as_file().sync_all().map_err(|e| e.to_string())?;
        tmp.persist(path).map_err(|e| e.to_string())?;
        Ok(next)
    }
    pub fn select(&self, id: Option<&str>) -> Result<Profile, String> {
        let id = id.unwrap_or(&self.default_profile);
        let p = self
            .profiles
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("Unknown profile: {id}"))?;
        let a = p.availability();
        if !a.available {
            return Err(format!("{}: {}", p.name, a.message));
        }
        Ok(p.clone())
    }
    pub fn catalog(self) -> Catalog {
        let availability = self
            .profiles
            .iter()
            .map(|p| (p.id.clone(), p.availability()))
            .collect();
        Catalog {
            config: self,
            availability,
        }
    }
}
struct ConfigLock {
    file: std::fs::File,
}
impl ConfigLock {
    fn acquire(path: PathBuf) -> Result<Self, String> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|e| e.to_string())?;
        file.try_lock()
            .map_err(|_| "Profiles are being saved by another process. Try again.".to_string())?;
        Ok(Self { file })
    }
}
impl Drop for ConfigLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// Bounded subprocess execution for explicit connection tests and WSL discovery.
pub fn run_bounded(program: &str, args: &[String], timeout_secs: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let read = |mut s: Box<dyn Read + Send>| {
        let mut out = Vec::new();
        let mut b = [0; 4096];
        while let Ok(n) = s.read(&mut b) {
            if n == 0 {
                break;
            }
            if out.len() < 65536 {
                out.extend_from_slice(&b[..n.min(65536 - out.len())]);
            }
        }
        out
    };
    let (out_tx, out_rx) = std::sync::mpsc::channel();
    let (err_tx, err_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = out_tx.send(read(Box::new(stdout)));
    });
    std::thread::spawn(move || {
        let _ = err_tx.send(read(Box::new(stderr)));
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Ok(s),
            Ok(None) => {}
            Err(e) => break Err(e.to_string()),
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            break Err("Connection test timed out".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    };
    // A descendant may retain the pipes even after the launcher has exited.
    // Keep the same deadline for process exit and output collection.
    let status = status?;
    let out = out_rx
        .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
        .map_err(|_| "Connection test timed out while reading output")?;
    let err = err_rx
        .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
        .map_err(|_| "Connection test timed out while reading output")?;
    if !status.success() {
        return Err(format!(
            "Connection test failed ({status}): {}",
            String::from_utf8_lossy(&err)
                .chars()
                .take(1000)
                .collect::<String>()
        ));
    }
    Ok(out)
}

pub fn test(profile: &Profile) -> Result<Availability, String> {
    let a = profile.availability();
    if !a.available {
        return Ok(a);
    }
    let plan = profile.prepare()?;
    let args = match profile.backend {
        BackendKind::Local => {
            return Ok(Availability {
                available: true,
                message:
                    "Executable and working directory verified. Open a tab to start this shell."
                        .into(),
            })
        }
        BackendKind::Ssh => {
            let mut a = vec![
                "-T".into(),
                "-o".into(),
                "BatchMode=yes".into(),
                "-o".into(),
                "ConnectTimeout=5".into(),
            ];
            if let Some(p) = profile.port {
                a.extend(["-p".into(), p.to_string()]);
            }
            a.extend([
                profile.target.clone().unwrap(),
                "echo CONN_CONNECTION_OK".into(),
            ]);
            a
        }
        BackendKind::Wsl => vec![
            "-d".into(),
            profile.target.clone().unwrap(),
            "--exec".into(),
            "/bin/echo".into(),
            "CONN_CONNECTION_OK".into(),
        ],
        BackendKind::Docker => vec![
            "exec".into(),
            profile.target.clone().unwrap(),
            "echo".into(),
            "CONN_CONNECTION_OK".into(),
        ],
    };
    let out = run_bounded(&plan.program, &args, 8)?;
    if !String::from_utf8_lossy(&out).contains("CONN_CONNECTION_OK") {
        return Err("Target did not return the connection marker".into());
    }
    Ok(Availability {
        available: true,
        message: "Target connection verified. Open a tab to test the configured shell.".into(),
    })
}
