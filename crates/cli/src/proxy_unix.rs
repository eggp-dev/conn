//! Terminal mode: the transparent proxy. stdin → engine, engine output → stdout.
//! This is what Terminal.app runs (`conn -- CMD`).

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use conn_core::audit::Audit;
use conn_core::policy::PolicyStore;
use conn_core::{Engine, EngineConfig, Pacing};

pub struct ProxyOptions {
    pub command: Vec<String>,
    pub profile: Option<conn_core::backend::Profile>,
    pub policy_path: PathBuf,
    pub audit_path: PathBuf,
    pub socket_path: PathBuf,
    pub pacing: Pacing,
}

/// Puts a tty into raw mode and restores it on drop.
pub struct RawMode {
    fd: i32,
    orig: libc::termios,
}

impl RawMode {
    pub fn enable(fd: i32) -> std::io::Result<Self> {
        let mut orig: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut orig) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut raw = orig;
        unsafe { libc::cfmakeraw(&mut raw) };
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self { fd, orig })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        unsafe { libc::tcsetattr(self.fd, libc::TCSANOW, &self.orig) };
    }
}

pub fn window_size(fd: i32) -> (u16, u16) {
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    if unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) } == 0 && ws.ws_row > 0 && ws.ws_col > 0 {
        (ws.ws_row, ws.ws_col)
    } else {
        (24, 80)
    }
}

fn is_tty(fd: i32) -> bool {
    unsafe { libc::isatty(fd) == 1 }
}

struct RawStdout;
impl Write for RawStdout {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut off = 0;
        while off < buf.len() {
            let n = unsafe { libc::write(1, buf[off..].as_ptr() as *const _, buf.len() - off) };
            if n < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
            off += n as usize;
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn load_policy(path: &PathBuf, audit: &Audit) -> Result<PolicyStore, Box<dyn std::error::Error + Send + Sync>> {
    match PolicyStore::open(path) {
        Ok(p) => Ok(p),
        Err(e) => {
            audit.record("system", "policy_load_failed", serde_json::json!({ "error": e.to_string() }));
            Err(format!("Could not load policy {}: {e}. Fix the policy before starting a shell.", path.display()).into())
        }
    }
}

/// Run the proxy until the child exits. Returns the child's exit code.
pub fn run(opts: ProxyOptions) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
    if !is_tty(0) || !is_tty(1) {
        return Err("conn must be run from a terminal (stdin/stdout are not a tty); use `conn serve` for headless mode".into());
    }
    if std::env::var_os("CONN").is_some() {
        return Err("already inside a conn session (CONN is set)".into());
    }
    let audit = Audit::open(&opts.audit_path)?;
    let policy = load_policy(&opts.policy_path, &audit)?;
    let (rows, cols) = window_size(1);

    let engine = Arc::new(Engine::spawn(EngineConfig {
        command: opts.command,
        profile: opts.profile,
        rows,
        cols,
        policy: Some(policy),
        audit: Some(audit),
        pacing: opts.pacing,
        output: Some(Box::new(RawStdout)),
        render_prompt: true,
        ..EngineConfig::default()
    })?);

    let raw = RawMode::enable(0)?;

    // stdin → engine (human path)
    {
        let engine = engine.clone();
        std::thread::Builder::new().name("stdin".into()).spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                let n = unsafe { libc::read(0, buf.as_mut_ptr() as *mut _, buf.len()) };
                if n < 0 {
                    if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    break;
                }
                if n == 0 {
                    break;
                }
                engine.write_input(&buf[..n as usize]);
            }
        })?;
    }

    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?;
    let exit_code = rt.block_on(async {
        use tokio::signal::unix::{signal, SignalKind};
        let server = tokio::spawn(conn_core::ipc::serve(opts.socket_path.clone(), conn_core::ipc::Hub::single("main", engine.session())));
        let mut winch = signal(SignalKind::window_change()).ok();
        let mut hup = signal(SignalKind::hangup()).ok();
        let mut term = signal(SignalKind::terminate()).ok();
        let mut poll = tokio::time::interval(std::time::Duration::from_millis(100));
        let code = loop {
            tokio::select! {
                _ = async { match winch.as_mut() { Some(s) => { s.recv().await; } None => std::future::pending::<()>().await } } => {
                    let (r, c) = window_size(1);
                    engine.resize(r, c);
                }
                _ = async { match hup.as_mut() { Some(s) => { s.recv().await; } None => std::future::pending::<()>().await } } => {
                    engine.session().lock().process_exited(None);
                    break 129;
                }
                _ = async { match term.as_mut() { Some(s) => { s.recv().await; } None => std::future::pending::<()>().await } } => {
                    engine.session().lock().process_exited(None);
                    break 143;
                }
                _ = poll.tick() => {
                    if let Some(code) = engine.try_wait() {
                        break code.unwrap_or(0) as i32;
                    }
                }
            }
        };
        server.abort();
        code
    });

    let _ = std::fs::remove_file(&opts.socket_path);
    drop(raw);
    Ok(exit_code)
}
