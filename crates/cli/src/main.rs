mod cli;
mod mcp;
mod proxy;
mod serve;
mod profiles;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args as ClapArgs, Parser, Subcommand};
use conn_core::{paths, Pacing};

/// Conn — one shell, one hand on it. You have the conn.
#[derive(Parser)]
#[command(name = "conn", version, about, long_about = None)]
struct Args {
    /// Profile to use for a new shell (default: saved default profile)
    #[arg(long, global = true)]
    profile: Option<String>,
    /// Profiles JSON file (default: ~/.conn/profiles.json)
    #[arg(long, global = true)]
    profiles_file: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Option<Cmd>,

    /// Policy file (default: ~/.conn/policy.yaml)
    #[arg(long, global = true)]
    policy: Option<PathBuf>,

    /// Audit log (default: ~/.conn/audit.jsonl)
    #[arg(long, global = true)]
    audit: Option<PathBuf>,

    /// Control socket (default: $CONN_SOCKET or ~/.conn/conn.sock)
    #[arg(long, global = true)]
    socket: Option<PathBuf>,

    #[command(flatten)]
    pacing: PacingArgs,

    /// Command to run before the login shell: `conn -- ssh host`
    #[arg(last = true)]
    command: Vec<String>,
}

#[derive(ClapArgs, Clone)]
struct PacingArgs {
    /// Minimum interval between agent writes, ms (0 = unlimited)
    #[arg(long, global = true, default_value_t = 0)]
    min_write_interval_ms: u64,
    /// Delay between an allowed ENTER and its execution, ms; human input cancels
    #[arg(long, global = true, default_value_t = 0)]
    enter_grace_ms: u64,
    /// Agent lease TTL, seconds
    #[arg(long, global = true, default_value_t = 60)]
    lease_ttl_secs: u64,
    /// Approval request TTL, seconds
    #[arg(long, global = true, default_value_t = 300)]
    approval_ttl_secs: u64,
}

impl From<PacingArgs> for Pacing {
    fn from(p: PacingArgs) -> Self {
        Pacing {
            min_write_interval_ms: p.min_write_interval_ms,
            enter_grace_ms: p.enter_grace_ms,
            lease_ttl_secs: p.lease_ttl_secs,
            approval_ttl_secs: p.approval_ttl_secs,
        }
    }
}

#[derive(Subcommand)]
enum Cmd {
    /// List, discover, import, select and test shell/backend profiles
    Profiles { #[command(subcommand)] command: profiles::ProfileCommand },
    /// Headless backend: no tty; frontends attach over the socket
    Serve {
        #[arg(long, default_value_t = 24)]
        rows: u16,
        #[arg(long, default_value_t = 80)]
        cols: u16,
        /// Command to run before the login shell
        #[arg(last = true)]
        command: Vec<String>,
    },
    /// MCP stdio server (run by the agent harness)
    Mcp {
        /// Agent identity recorded in the audit log (default: derived from the MCP client's name)
        #[arg(long)]
        agent_id: Option<String>,
        /// Tool list: dynamic (follows affordances, needs list_changed support), static
        /// (always all tools; unavailable ones error), auto (static for Codex)
        #[arg(long, default_value = "auto")]
        tools: String,
    },
    /// Show controller, pending approvals and connected clients
    Status,
    /// Take the conn: revoke the agent's lease without typing anything
    Take,
    /// Decide a pending approval (oldest one if no id is given)
    Approve {
        id: Option<String>,
        #[arg(long, short = 'd', conflicts_with = "allow_session")]
        deny: bool,
        /// Grant, and allow this label for the rest of the session
        #[arg(long, short = 'A')]
        allow_session: bool,
    },
    /// Show or change pacing of the running session
    Pacing {
        #[arg(long)]
        min_write_interval_ms: Option<u64>,
        #[arg(long)]
        enter_grace_ms: Option<u64>,
        #[arg(long)]
        lease_ttl_secs: Option<u64>,
        #[arg(long)]
        approval_ttl_secs: Option<u64>,
    },
    /// Restrict agent affordances for the running session (e.g. `snapshot request_control`); no args = show; `--clear` = lift
    Affordances {
        allow: Vec<String>,
        #[arg(long)]
        clear: bool,
    },
    /// Show or set the agent mode: observe | copilot | autopilot
    Mode { mode: Option<String> },
    /// Control gate: --ask makes every request_control wait for `conn decide`
    Gate {
        #[arg(long, conflicts_with = "auto")]
        ask: bool,
        #[arg(long)]
        auto: bool,
    },
    /// Decide a pending control request (oldest if no id)
    Decide {
        id: Option<String>,
        #[arg(long, short = 'd')]
        deny: bool,
    },
    /// Give the conn back to the agent that last held it
    Handback,
    /// List sessions (tabs) behind the socket; ▶ marks the attended one
    Sessions,
    /// Mark a session as the one you are looking at
    Attend { id: String },
    /// Leave the attended session to its agent while you are away (policy `unattended` cap applies)
    Entrust {
        #[arg(long)]
        session: Option<String>,
    },
    /// Print the agent guide (same content as the Claude Code skill)
    Guide,
    /// Agent actions over the socket, for harnesses without MCP.
    /// `agent run "<cmd>"` does request → type → enter → wait in one go.
    Agent {
        #[arg(long, default_value = "cli-agent")]
        agent_id: String,
        /// snapshot | request [--reason R] [--command COMMAND] | type <text> | enter | key <KEY> | interrupt | check <id> | release | tabs | open-tab [--reason R] | switch-tab <n|id> | run <cmd> [--reason R] [--keep]
        action: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Print the audit log
    Log {
        #[arg(short = 'n', long, default_value_t = 50)]
        lines: usize,
        #[arg(long)]
        actor: Option<String>,
        /// Print raw JSONL
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    let args = Args::parse();
    let socket = args.socket.clone().unwrap_or_else(paths::socket_path);
    let audit = args.audit.clone().unwrap_or_else(paths::audit_path);
    let policy = args.policy.clone().unwrap_or_else(paths::policy_path);
    let pacing: Pacing = args.pacing.clone().into();

    let profiles_path = args.profiles_file.unwrap_or_else(conn_core::profiles::path);
    let selected = || conn_core::profiles::Profiles::load(&profiles_path)?.select(args.profile.as_deref()).map(Some);
    let result: Result<i32, String> = (|| { match args.cmd {
        None => proxy::run(proxy::ProxyOptions { profile: selected()?, command: args.command, policy_path: policy, audit_path: audit, socket_path: socket, pacing })
            .map_err(|e| e.to_string()),
        Some(Cmd::Serve { rows, cols, command }) => {
            serve::run(serve::ServeOptions { profile: selected()?, command, rows, cols, policy_path: policy, audit_path: audit, socket_path: socket, pacing })
                .map_err(|e| e.to_string())
        }
        Some(Cmd::Mcp { agent_id, tools }) => {
            let mode = match tools.as_str() {
                "dynamic" => mcp::ToolMode::Dynamic,
                "static" => mcp::ToolMode::Static,
                _ => mcp::ToolMode::Auto,
            };
            mcp::run(socket, agent_id, mode).map(|_| 0).map_err(|e| e.to_string())
        }
        Some(Cmd::Profiles {command}) => profiles::run(command, &profiles_path),
        Some(Cmd::Status) => cli::status(&socket).map(|_| 0).map_err(|e| e.to_string()),
        Some(Cmd::Take) => cli::take(&socket).map(|_| 0).map_err(|e| e.to_string()),
        Some(Cmd::Approve { id, deny, allow_session }) => {
            cli::approve(&socket, id, deny, allow_session).map(|_| 0).map_err(|e| e.to_string())
        }
        Some(Cmd::Pacing { min_write_interval_ms, enter_grace_ms, lease_ttl_secs, approval_ttl_secs }) => {
            cli::pacing(&socket, min_write_interval_ms, enter_grace_ms, lease_ttl_secs, approval_ttl_secs).map(|_| 0).map_err(|e| e.to_string())
        }
        Some(Cmd::Affordances { allow, clear }) => cli::affordances(&socket, allow, clear).map(|_| 0).map_err(|e| e.to_string()),
        Some(Cmd::Mode { mode }) => cli::mode(&socket, mode).map(|_| 0).map_err(|e| e.to_string()),
        Some(Cmd::Gate { ask, auto }) => cli::gate(&socket, if ask { Some(true) } else if auto { Some(false) } else { None }).map(|_| 0).map_err(|e| e.to_string()),
        Some(Cmd::Decide { id, deny }) => cli::decide(&socket, id, deny).map(|_| 0).map_err(|e| e.to_string()),
        Some(Cmd::Handback) => cli::handback(&socket).map(|_| 0).map_err(|e| e.to_string()),
        Some(Cmd::Sessions) => cli::sessions(&socket).map(|_| 0).map_err(|e| e.to_string()),
        Some(Cmd::Attend { id }) => cli::attend(&socket, &id).map(|_| 0).map_err(|e| e.to_string()),
        Some(Cmd::Entrust { session }) => cli::entrust(&socket, session).map(|_| 0).map_err(|e| e.to_string()),
        Some(Cmd::Guide) => { cli::guide(); Ok(0) }
        Some(Cmd::Agent { agent_id, action, args }) => {
            if action == "run" {
                let reason = args.iter().position(|a| a == "--reason").and_then(|i| args.get(i + 1)).cloned();
                if reason.is_none() {
                    eprintln!("conn: `agent run` needs --reason \"what this command does and changes\" (it is shown to the human and required as the ENTER intent)");
                    return Err("agent run requires --reason".into());
                }
                let keep = args.iter().any(|a| a == "--keep");
                let text: Vec<String> = { let mut skip = false; args.iter().filter(|a| { if skip { skip = false; return false; } if *a == "--reason" { skip = true; return false; } *a != "--keep" }).cloned().collect() };
                cli::agent_run(&socket, &agent_id, reason, &text.join(" "), !keep).map(|_| 0).map_err(|e| e.to_string())
            } else {
                cli::agent(&socket, &agent_id, &action, &args).map(|_| 0).map_err(|e| e.to_string())
            }
        }
        Some(Cmd::Log { lines, actor, json }) => cli::log(&audit, lines, actor, json).map(|_| 0).map_err(|e| e.to_string()),
    } })();
    match result {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(e) => {
            eprintln!("conn: {e}");
            ExitCode::from(1)
        }
    }
}
