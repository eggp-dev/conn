//! Headless mode: no tty. Frontends attach over the socket, stream output, send input.

use std::path::PathBuf;

use conn_core::audit::Audit;
use conn_core::{Engine, EngineConfig, Pacing};

pub struct ServeOptions {
    pub command: Vec<String>,
    pub profile: Option<conn_core::backend::Profile>,
    pub rows: u16,
    pub cols: u16,
    pub policy_path: PathBuf,
    pub audit_path: PathBuf,
    pub socket_path: PathBuf,
    pub pacing: Pacing,
}

pub fn run(opts: ServeOptions) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
    let audit = Audit::open(&opts.audit_path)?;
    let policy = crate::proxy::load_policy(&opts.policy_path, &audit)?;
    let engine = Engine::spawn(EngineConfig {
        command: opts.command,
        profile: opts.profile,
        rows: opts.rows,
        cols: opts.cols,
        policy: Some(policy),
        audit: Some(audit),
        pacing: opts.pacing,
        output: None,
        render_prompt: false,
        ..EngineConfig::default()
    })?;
    let guard = conn_core::ipc::serve_in_background(opts.socket_path.clone(), conn_core::ipc::Hub::single("main", engine.session()))?;
    eprintln!("conn: serving {} (pid {:?}); attach a frontend with hello kind=frontend", guard.path().display(), engine.pid());
    let code = engine.wait();
    drop(guard);
    Ok(code.unwrap_or(0) as i32)
}
