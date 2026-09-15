//! Native Windows console frontend. ConPTY belongs to the engine; console events belong here.
use conn_core::{
    audit::Audit,
    policy::PolicyStore,
};
use conn_core::{Engine, EngineConfig, Pacing};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal,
};
use std::{
    io::{IsTerminal, Write},
    path::PathBuf,
    sync::Arc,
};

pub struct ProxyOptions {
    pub command: Vec<String>,
    pub profile: Option<conn_core::backend::Profile>,
    pub policy_path: PathBuf,
    pub audit_path: PathBuf,
    pub socket_path: PathBuf,
    pub pacing: Pacing,
}
pub fn load_policy(path: &PathBuf, audit: &Audit) -> Result<PolicyStore, Box<dyn std::error::Error + Send + Sync>> {
    match PolicyStore::open(path) {
        Ok(p) => Ok(p),
        Err(e) => {
            audit.record(
                "system",
                "policy_load_failed",
                serde_json::json!({"error":e.to_string()}),
            );
            Err(format!("Could not load policy {}: {e}. Fix the policy before starting a shell.", path.display()).into())
        }
    }
}
struct RawMode;
impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}
struct ConsoleOutput;
impl Write for ConsoleOutput {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        let mut out = std::io::stdout();
        out.write_all(b)?;
        out.flush()?;
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        std::io::stdout().flush()
    }
}
pub fn run(opts: ProxyOptions) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err("Run conn in a terminal, or use conn serve".into());
    }
    if std::env::var_os("CONN").is_some() {
        return Err("Already inside a conn session".into());
    }
    // ConPTY emits VT sequences; enable their interpretation in the host console.
    if !crossterm::ansi_support::supports_ansi() {
        return Err(
            "This console cannot render VT output. Use Windows Terminal or conn serve".into(),
        );
    }
    let audit = Audit::open(&opts.audit_path)?;
    let policy = load_policy(&opts.policy_path, &audit)?;
    let (cols, rows) = terminal::size().unwrap_or((80, 24));
    let engine = Arc::new(Engine::spawn(EngineConfig {
        profile: opts.profile,
        command: opts.command,
        rows,
        cols,
        policy: Some(policy),
        audit: Some(audit),
        pacing: opts.pacing,
        output: Some(Box::new(ConsoleOutput)),
        render_prompt: true,
        ..EngineConfig::default()
    })?);
    let _guard = conn_core::ipc::serve_in_background(
        opts.socket_path,
        conn_core::ipc::Hub::single("main", engine.session()),
    )?;
    terminal::enable_raw_mode()?;
    let _raw = RawMode;
    while !engine.has_exited() {
        if !event::poll(std::time::Duration::from_millis(100))? {
            continue;
        }
        match event::read()? {
            Event::Resize(cols, rows) => engine.resize(rows, cols),
            Event::Paste(s) => engine.write_input(s.as_bytes()),
            Event::Key(k) if k.kind != KeyEventKind::Release => {
                let bytes = key_bytes(k.code, k.modifiers);
                if !bytes.is_empty() {
                    engine.write_input(&bytes);
                }
            }
            _ => {}
        }
    }
    Ok(engine.wait().unwrap_or(0) as i32)
}
fn key_bytes(code: KeyCode, modifiers: KeyModifiers) -> Vec<u8> {
    let text = match code {
        KeyCode::Char(c) if modifiers.contains(KeyModifiers::CONTROL) && c.is_ascii() => {
            return vec![(c.to_ascii_uppercase() as u8) & 0x1f]
        }
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "\r".into(),
        KeyCode::Backspace => "\x7f".into(),
        KeyCode::Tab => "\t".into(),
        KeyCode::BackTab => "\x1b[Z".into(),
        KeyCode::Esc => "\x1b".into(),
        KeyCode::Up => "\x1b[A".into(),
        KeyCode::Down => "\x1b[B".into(),
        KeyCode::Right => "\x1b[C".into(),
        KeyCode::Left => "\x1b[D".into(),
        KeyCode::Home => "\x1b[H".into(),
        KeyCode::End => "\x1b[F".into(),
        KeyCode::Delete => "\x1b[3~".into(),
        KeyCode::Insert => "\x1b[2~".into(),
        KeyCode::PageUp => "\x1b[5~".into(),
        KeyCode::PageDown => "\x1b[6~".into(),
        KeyCode::F(n) => match n {
            1 => "\x1bOP".into(),
            2 => "\x1bOQ".into(),
            3 => "\x1bOR".into(),
            4 => "\x1bOS".into(),
            5..=12 => format!(
                "\x1b[{}~",
                [15, 17, 18, 19, 20, 21, 23, 24][(n - 5) as usize]
            ),
            _ => String::new(),
        },
        _ => String::new(),
    };
    let mut bytes = Vec::new();
    if modifiers.contains(KeyModifiers::ALT) {
        bytes.push(27);
    }
    bytes.extend(text.as_bytes());
    bytes
}
