//! Client-specific capabilities and paths. Keep the installer independent of clients.
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Clone, Copy)]
pub enum Format {
    Toml,
    Json(&'static str),
}
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Adapter {
    pub id: &'static str,
    pub name: &'static str,
    #[serde(skip)]
    pub format: Format,
}
pub const ADAPTERS: &[Adapter] = &[
    Adapter {
        id: "codex",
        name: "Codex / ChatGPT",
        format: Format::Toml,
    },
    Adapter {
        id: "claude-code",
        name: "Claude Code",
        format: Format::Json("mcpServers"),
    },
    Adapter {
        id: "cursor",
        name: "Cursor",
        format: Format::Json("mcpServers"),
    },
    Adapter {
        id: "copilot-vscode",
        name: "GitHub Copilot · VS Code",
        format: Format::Json("servers"),
    },
    Adapter {
        id: "copilot-cli",
        name: "GitHub Copilot · CLI",
        format: Format::Json("mcpServers"),
    },
];
pub fn adapter(id: &str) -> Result<Adapter, String> {
    ADAPTERS
        .iter()
        .find(|a| a.id == id)
        .copied()
        .ok_or_else(|| "unknown_client".into())
}

pub struct Locations {
    pub home: PathBuf,
    pub codex: PathBuf,
    pub claude: PathBuf,
    pub claude_config: PathBuf,
    pub copilot: PathBuf,
    pub vscode: PathBuf,
}
impl Locations {
    pub fn isolated(home: PathBuf) -> Self {
        Self {
            codex: home.join(".codex"),
            claude: home.join(".claude"),
            claude_config: home.join(".claude.json"),
            copilot: home.join(".copilot"),
            vscode: home.join(".config/Code/User"),
            home,
        }
    }
    pub fn system() -> Result<Self, String> {
        let home = conn_core::profiles::home_dir().ok_or("home_unavailable")?;
        let env_path = |key: &str| {
            std::env::var_os(key)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        let claude_override = env_path("CLAUDE_CONFIG_DIR");
        let claude = claude_override
            .clone()
            .unwrap_or_else(|| home.join(".claude"));
        let claude_config = if claude_override.is_some() {
            claude.join(".claude.json")
        } else {
            home.join(".claude.json")
        };
        let vscode = match std::env::consts::OS {
            "macos" => home.join("Library/Application Support/Code/User"),
            "windows" => env_path("APPDATA")
                .unwrap_or_else(|| home.join("AppData/Roaming"))
                .join("Code/User"),
            _ => env_path("XDG_CONFIG_HOME")
                .unwrap_or_else(|| home.join(".config"))
                .join("Code/User"),
        };
        Ok(Self {
            codex: env_path("CODEX_HOME").unwrap_or_else(|| home.join(".codex")),
            copilot: env_path("COPILOT_HOME").unwrap_or_else(|| home.join(".copilot")),
            home,
            claude,
            claude_config,
            vscode,
        })
    }
    pub fn paths(&self, a: Adapter) -> (PathBuf, PathBuf) {
        let (config, skills) = match a.id {
            "codex" => (
                self.codex.join("config.toml"),
                self.home.join(".agents/skills"),
            ),
            "claude-code" => (self.claude_config.clone(), self.claude.join("skills")),
            "cursor" => (
                self.home.join(".cursor/mcp.json"),
                self.home.join(".cursor/skills"),
            ),
            "copilot-vscode" => (
                self.vscode.join("mcp.json"),
                self.home.join(".copilot/skills"),
            ),
            "copilot-cli" => (
                self.copilot.join("mcp-config.json"),
                self.copilot.join("skills"),
            ),
            _ => unreachable!("registered adapter"),
        };
        (config, skills.join("conn/SKILL.md"))
    }
}
impl Adapter {
    pub fn entry(self, command: &str, endpoint: &str) -> Value {
        // A fixed identity lets the UI match a real IPC connection to this setup.
        // Static tools also work in clients without tools/list_changed support.
        let mut entry = json!({"command": command, "args": ["--socket", endpoint, "mcp", "--agent-id", self.id, "--tools", "static"]});
        match self.id {
            "claude-code" | "copilot-vscode" => {
                entry["type"] = json!("stdio");
            }
            "copilot-cli" => {
                entry["type"] = json!("local");
                entry["tools"] = json!(["*"]);
            }
            _ => {}
        }
        entry
    }
}
