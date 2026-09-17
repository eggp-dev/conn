mod cli;
mod mcp;
mod profiles;

use std::{path::PathBuf, process::ExitCode};
use clap::{Parser, Subcommand};
use conn_core::paths;

/// Conn — share the shell you see. Start the Conn app before connecting an agent.
#[derive(Parser)]
#[command(name = "conn", version, about)]
struct Args {
    #[arg(long, global = true)]
    socket: Option<PathBuf>,
    #[arg(long, global = true)]
    profiles_file: Option<PathBuf>,
    /// Local audit file; reading it uses this process's filesystem permissions.
    #[arg(long, global = true)]
    audit: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Manage saved execution profiles
    Profiles { #[command(subcommand)] command: profiles::ProfileCommand },
    /// MCP stdio adapter, launched by the agent client
    Mcp {
        /// Display label only; permissions bind to the actual connection
        #[arg(long)]
        agent_id: Option<String>,
        #[arg(long, default_value = "auto")]
        tools: String,
    },
    /// Show sessions available to this connection (owner settings stay in the app)
    Status,
    /// List available sessions
    Sessions,
    /// Print the collaboration guide
    Guide,
    /// Agent operations; use MCP for a persistent connection
    Agent {
        #[arg(long, default_value = "cli-agent")]
        agent_id: String,
        action: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Read a local audit file using this process's filesystem permissions
    Log {
        #[arg(short = 'n', long, default_value_t = 50)]
        lines: usize,
        #[arg(long)]
        actor: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    let args = Args::parse();
    let socket = args.socket.unwrap_or_else(paths::socket_path);
    let audit = args.audit.unwrap_or_else(paths::audit_path);
    let profiles_path = args.profiles_file.unwrap_or_else(conn_core::profiles::path);
    let result: Result<i32, String> = (|| match args.cmd {
        None => Err("Open the Conn app to start a visible shell, then connect with `conn mcp`. Headless and terminal-proxy sessions no longer provide agent access.".into()),
        Some(Cmd::Mcp { agent_id, tools }) => {
            let mode = match tools.as_str() {
                "dynamic" => mcp::ToolMode::Dynamic,
                "static" => mcp::ToolMode::Static,
                "auto" => mcp::ToolMode::Auto,
                _ => return Err("tools must be auto, dynamic or static".into()),
            };
            mcp::run(socket, agent_id, mode).map(|_| 0).map_err(|e| e.to_string())
        }
        Some(Cmd::Profiles { command }) => profiles::run(command, &profiles_path),
        Some(Cmd::Status | Cmd::Sessions) => cli::sessions(&socket).map(|_| 0).map_err(|e| e.to_string()),
        Some(Cmd::Guide) => { cli::guide(); Ok(0) }
        Some(Cmd::Agent { agent_id, action, args }) => {
            if action == "run" {
                let reason = args.iter().position(|a| a == "--reason").and_then(|i| args.get(i + 1)).cloned();
                if reason.is_none() { return Err("agent run requires --reason describing the command and its effects".into()); }
                let keep = args.iter().any(|a| a == "--keep");
                let text: Vec<String> = { let mut skip = false; args.iter().filter(|a| { if skip { skip = false; return false; } if *a == "--reason" { skip = true; return false; } *a != "--keep" }).cloned().collect() };
                cli::agent_run(&socket, &agent_id, reason, &text.join(" "), !keep).map(|_| 0).map_err(|e| e.to_string())
            } else {
                cli::agent(&socket, &agent_id, &action, &args).map(|_| 0).map_err(|e| e.to_string())
            }
        }
        Some(Cmd::Log { lines, actor, json }) => cli::log(&audit, lines, actor, json).map(|_| 0).map_err(|e| e.to_string()),
    })();
    match result {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(e) => { eprintln!("conn: {e}"); ExitCode::FAILURE }
    }
}
