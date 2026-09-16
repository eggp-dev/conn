//! The engine: a spawned shell in a PTY plus the threads that keep a `Session`
//! alive. This is what an embedding frontend (Tauri, etc.) links against.
//!
//! ```no_run
//! use conn_core::{Engine, EngineConfig};
//! let engine = Engine::spawn(EngineConfig::default()).unwrap();
//! engine.subscribe("ui", Box::new(|ev| println!("{ev:?}")), true);
//! engine.write_input(b"ls\r");
//! ```

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde_json::json;

use crate::audit::Audit;
use crate::authority::ConnId;
use crate::config::Pacing;
use crate::policy::PolicyStore;
use crate::session::{EventSink, Session, SessionConfig, SharedSession};

mod child_killer;
use child_killer::ChildKiller;

/// Move-owned startup specification for trusted native callers. Never serialized.
#[derive(Default)]
pub enum LaunchSpec {
    #[default]
    ProfileDefault,
    Program { executable: String, argv: Vec<String> },
}

pub struct EngineConfig {
    /// Immutable private origin, established before opening any log or spawning.
    pub external_private: bool,
    /// Direct process launch; an override is restricted to private local sessions.
    pub launch: LaunchSpec,
    /// Resolved execution profile. None preserves the embedders' default shell.
    pub profile: Option<crate::backend::Profile>,
    /// Shell to spawn. Default: `$SHELL`, else `/bin/zsh`.
    pub shell: Option<String>,
    /// Extra command to run before the login shell (`shell -l -c "CMD; exec shell -l"`).
    pub command: Vec<String>,
    pub rows: u16,
    pub cols: u16,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub policy: Option<PolicyStore>,
    pub audit: Option<Audit>,
    pub pacing: Pacing,
    /// Where PTY output goes for the human. `None` = only streamed to subscribers.
    pub output: Option<Box<dyn Write + Send>>,
    /// Draw the approval prompt into `output`. Frontends set false.
    pub render_prompt: bool,
    /// How often the session tick runs (lease/approval expiry, grace timers, screen events).
    pub tick: Duration,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            external_private: false,
            launch: LaunchSpec::ProfileDefault,
            profile: None,
            shell: None,
            command: vec![],
            rows: 24,
            cols: 80,
            cwd: None,
            env: vec![],
            policy: None,
            audit: None,
            pacing: Pacing::default(),
            output: None,
            render_prompt: false,
            tick: Duration::from_millis(50),
        }
    }
}

pub struct Engine {
    killer: parking_lot::Mutex<ChildKiller>,
    session: SharedSession,
    exit_rx: parking_lot::Mutex<Option<mpsc::Receiver<Option<u32>>>>,
    exited: Arc<AtomicBool>,
    exit_code: parking_lot::Mutex<Option<Option<u32>>>,
    shell: String,
    pid: Option<u32>,
}

pub fn default_shell() -> String {
    #[cfg(windows)] {
        if crate::backend::executable("pwsh.exe").is_some() { return "pwsh.exe".into(); }
        return std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".into());
    }
    #[cfg(unix)] {
        std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| {
            if cfg!(target_os="macos") { "/bin/zsh".into() } else { "/bin/sh".into() }
        })
    }
}

impl Engine {
    pub fn spawn(cfg: EngineConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let private = cfg.external_private;
        Self::spawn_inner(cfg).map_err(|e| if private { "External session could not start".into() } else { e })
    }

    fn spawn_inner(cfg: EngineConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let audit = if cfg.external_private {
            Audit::null()
        } else {
            match cfg.audit {
                Some(a) => a,
                None => Audit::open(&crate::paths::audit_path())?,
            }
        };
        let policy = match cfg.policy {
            Some(p) => p,
            None => match PolicyStore::open(&crate::paths::policy_path()) {
                Ok(p) => p,
                Err(e) => {
                    audit.record("system", "policy_load_failed", json!({ "error": e.to_string() }));
                    return Err(format!("Could not load policy {}: {e}. Fix the policy before starting a shell.", crate::paths::policy_path().display()).into());
                }
            },
        };

        let (rows, cols) = (cfg.rows.max(1), cfg.cols.max(1));
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;

        let profile = cfg.profile.clone().unwrap_or_else(|| crate::backend::Profile::local("local-default".into(), cfg.shell.clone().unwrap_or_else(default_shell)));
        let mut plan = profile.prepare()?;
        if cfg.external_private && !cfg.command.is_empty() {
            return Err("External sessions require a direct launch specification".into());
        }
        if let LaunchSpec::Program { executable, argv } = cfg.launch {
            if !cfg.external_private || profile.backend != crate::backend::BackendKind::Local {
                return Err("Direct startup requires a private local session".into());
            }
            if executable.is_empty() || executable.chars().any(char::is_control)
                || argv.iter().any(|arg| arg.chars().any(char::is_control))
                || executable.len() + argv.iter().map(String::len).sum::<usize>() > 16384
            {
                return Err("Invalid startup program".into());
            }
            plan.program = executable;
            plan.args = argv;
        }
        if !cfg.command.is_empty() {
            if profile.backend != crate::backend::BackendKind::Local { return Err("Startup commands with -- are supported for local profiles only".into()); }
            use crate::backend::{ShellKind,quote_posix};
            plan.args = match profile.shell {
                ShellKind::Posix | ShellKind::Fish => {
                    let joined = cfg.command.join(" "); // Preserve the documented -- shell-command semantics.
                    vec!["-l".into(), "-c".into(), format!("{joined}; exec {} -l",quote_posix(&plan.program))]
                }
                ShellKind::PowerShell => {
                    let joined = cfg.command.iter().map(|s| format!("'{}'",s.replace('\'',"''"))).collect::<Vec<_>>().join(" ");
                    vec!["-NoLogo".into(),"-NoExit".into(),"-Command".into(),format!("& {joined}")]
                }
                ShellKind::Cmd => {
                    if cfg.command.iter().any(|s|s.contains(['"','%','!','^','&','|','<','>','\r','\n'])) { return Err("cmd startup arguments contain shell operators; enter this command interactively".into()); }
                    vec!["/D".into(),"/K".into(),cfg.command.iter().map(|s|format!("\"{s}\"")).collect::<Vec<_>>().join(" ")]
                }
                ShellKind::Custom => return Err("Startup commands require a known shell dialect".into()),
            };
        }
        #[cfg(unix)]
        let integration = if !cfg.external_private && cfg.command.is_empty() && profile.backend == crate::backend::BackendKind::Local {
            crate::shell_integration::Integration::prepare(&mut plan, &cfg.env).ok().flatten()
        } else { None };
        let shell = profile.program.clone();
        let mut cmd = CommandBuilder::new(&plan.program);
        cmd.args(&plan.args);
        cmd.env("CONN", "1");
        cmd.env("TERM", std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".into()));
        for (k, v) in &cfg.env { cmd.env(k, v); }
        for (k, v) in &plan.env { cmd.env(k, v); }
        if let Some(d) = plan.cwd.as_ref().or(cfg.cwd.as_ref()) { cmd.cwd(d); }
        else if let Ok(d) = std::env::current_dir() { cmd.cwd(d); }
        let mut child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);
        let mut pty_reader = pair.master.try_clone_reader()?;
        let pty_writer = pair.master.take_writer()?;
        let pid = child.process_id();
        let killer = parking_lot::Mutex::new(ChildKiller::new(child.as_ref())?);

        if !cfg.external_private { audit.record(
            "system",
            "session_start",
            json!({ "pid": pid, "shell": shell, "profileId": profile.id, "backend": profile.backend, "command": cfg.command, "rows": rows, "cols": cols }),
        ); }
        if !cfg.command.is_empty() {
            audit.record("human", "exec", json!({ "cmd": cfg.command.join(" "), "via": "argv" }));
        }

        let session_cfg = SessionConfig {
            rows,
            cols,
            audit,
            policy,
            pty_writer,
            output: cfg.output,
            master: Some(pair.master),
            pacing: cfg.pacing,
            render_prompt: cfg.render_prompt,
            shell_pid: pid,
        };
        let session: SharedSession = Arc::new(parking_lot::Mutex::new(if cfg.external_private {
            Session::new_external_private(session_cfg)
        } else {
            Session::new(session_cfg)
        }));

        session.lock().set_execution_profile(profile);
        #[cfg(unix)]
        let integration = Arc::new(parking_lot::Mutex::new(integration));
        #[cfg(unix)]
        if integration.lock().is_some() { session.lock().shell_integration_starting(); }

        let exited = Arc::new(AtomicBool::new(false));

        // PTY → session
        let pty_done = Arc::new(AtomicBool::new(false));
        {
            let session = session.clone();
            let pty_done = pty_done.clone();
            std::thread::Builder::new().name("ss-pty-read".into()).spawn(move || {
                let mut buf = [0u8; 16384];
                loop {
                    match pty_reader.read(&mut buf) {
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(1)),
                        Ok(0) | Err(_) => break,
                        Ok(n) => session.lock().pty_output(&buf[..n]),
                    }
                }
                pty_done.store(true, Ordering::SeqCst);
            })?;
        }

        // tick
        {
            let session = session.clone();
            let exited = exited.clone();
            #[cfg(unix)]
            let integration = integration.clone();
            let tick = cfg.tick.max(Duration::from_millis(5));
            std::thread::Builder::new().name("ss-tick".into()).spawn(move || {
                while !exited.load(Ordering::SeqCst) {
                    std::thread::sleep(tick);
                    #[cfg(unix)]
                    if let Some(hook) = integration.lock().as_mut() { hook.drain(&mut session.lock(), pid); }
                    session.lock().tick(Instant::now());
                }
            })?;
        }

        // child exit
        let (exit_tx, exit_rx) = mpsc::channel();
        {
            let session = session.clone();
            let exited = exited.clone();
            std::thread::Builder::new().name("ss-child-wait".into()).spawn(move || {
                let code = child.wait().ok().map(|s| s.exit_code());
                let deadline = Instant::now() + Duration::from_millis(300);
                while !pty_done.load(Ordering::SeqCst) && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(10));
                }
                #[cfg(unix)]
                { let mut integration = integration.lock();
                  if let Some(hook) = integration.as_mut() { hook.drain(&mut session.lock(), pid); }
                  *integration = None;
                }
                session.lock().process_exited(code);
                exited.store(true, Ordering::SeqCst);
                let _ = exit_tx.send(code);
            })?;
        }

        Ok(Self {
            killer,
            session,
            exit_rx: parking_lot::Mutex::new(Some(exit_rx)),
            exited,
            exit_code: parking_lot::Mutex::new(None),
            shell,
            pid,
        })
    }

    /// Stop the backend process, independent of the shell's EOF convention.
    pub fn terminate(&self) -> std::io::Result<()> {
        if !self.has_exited() { self.killer.lock().kill()?; }
        Ok(())
    }

    pub fn session(&self) -> SharedSession {
        self.session.clone()
    }

    pub fn shell(&self) -> &str {
        &self.shell
    }

    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Human keyboard input. Revokes any agent lease first.
    pub fn write_input(&self, bytes: &[u8]) {
        self.session.lock().human_input(bytes);
    }

    /// Deliver one bounded native external-input chunk. No command tracking.
    pub fn write_external(&self, bytes: &[u8]) -> Result<(), crate::session::SessionError> {
        self.session.lock().write_external(bytes)
    }

    pub fn resize(&self, rows: u16, cols: u16) {
        self.session.lock().resize(rows, cols);
    }

    /// Register an in-process frontend. Returns an id for `unsubscribe`.
    pub fn subscribe(&self, name: &str, sink: Box<dyn EventSink>, stream_output: bool) -> ConnId {
        self.session.lock().subscribe(name, sink, stream_output)
    }

    pub fn unsubscribe(&self, id: ConnId) {
        self.session.lock().unsubscribe(id);
    }

    pub fn has_exited(&self) -> bool {
        self.exited.load(Ordering::SeqCst)
    }

    /// Block until the shell exits. Returns its exit code (None if unknown).
    pub fn wait(&self) -> Option<u32> {
        if let Some(c) = *self.exit_code.lock() {
            return c;
        }
        let rx = self.exit_rx.lock().take();
        let code = rx.and_then(|rx| rx.recv().ok()).flatten();
        *self.exit_code.lock() = Some(code);
        code
    }

    /// Non-blocking variant of `wait`.
    pub fn try_wait(&self) -> Option<Option<u32>> {
        if let Some(c) = *self.exit_code.lock() {
            return Some(c);
        }
        let guard = self.exit_rx.lock();
        let rx = guard.as_ref()?;
        match rx.try_recv() {
            Ok(code) => {
                *self.exit_code.lock() = Some(code);
                Some(code)
            }
            Err(_) => None,
        }
    }
}

impl Drop for Engine { fn drop(&mut self) { let _ = self.terminate(); } }
