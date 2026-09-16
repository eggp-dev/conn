# Agent integrations

[한국어](agent-integrations.ko.md)

Available in Conn v0.4.0 and newer. Upgrade from v0.3.0 to use the setup buttons.

## One setup per client

1. Install Conn and open a terminal.
2. Open **Settings → Agents** and choose **Set up** on your client.
3. Restart or reconnect the client using the hint on its card. Keep Conn running.
4. Ask: “Use Conn to read my current terminal. Do not run a command yet.”

Conn registers the bundled executable as an MCP stdio server and installs the
collaboration skill in the same action. Rust, Node.js and PATH changes are not
required. The skill is embedded in the app; setup does not download or execute
an installer from the internet.

**Configured** means the files are registered. **Connected now** means a client
using that setup's identity is currently attached to one of this Conn instance's
terminals. It is a connection indicator, not client authentication, a successful
model task, or permission to control the shell. Client trust prompts and Conn's
approval policy still apply. A client without skills can use the MCP server's
built-in instructions independently.

| Client | Default personal MCP configuration | Skill | After setup |
|---|---|---|---|
| Codex / ChatGPT local desktop tasks | `~/.codex/config.toml` → `mcp_servers.conn` | `~/.agents/skills/conn/SKILL.md` | Restart the local client / start a new task; inspect MCP tools |
| Claude Code | `~/.claude.json` → `mcpServers.conn` | `~/.claude/skills/conn/SKILL.md` | New session; `/mcp`, `/skills` |
| Cursor | `~/.cursor/mcp.json` → `mcpServers.conn` | `~/.cursor/skills/conn/SKILL.md` | Restart; enable Conn in MCP settings |
| GitHub Copilot in VS Code | Default user profile `mcp.json` → `servers.conn` | `~/.copilot/skills/conn/SKILL.md` | Reload; **MCP: List Servers** → start Conn |
| GitHub Copilot CLI | `~/.copilot/mcp-config.json` → `mcpServers.conn` | `~/.copilot/skills/conn/SKILL.md` | New session; `/mcp` |

VS Code's default user directory is `~/Library/Application Support/Code/User` on
macOS, `%APPDATA%/Code/User` on Windows, and `$XDG_CONFIG_HOME/Code/User` or
`~/.config/Code/User` on Linux. Named/portable VS Code profiles and remote
containers need manual configuration in that profile. Use the card's **Setup
details → Copy configuration for manual setup**; its wrapper is client-specific.

`CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `COPILOT_HOME`, `XDG_CONFIG_HOME` and `APPDATA`
are respected when present in Conn's own environment. For a custom Claude
configuration directory, MCP settings live in `$CLAUDE_CONFIG_DIR/.claude.json`.
The card displays the actual paths before you install. Shell-specific environment
variables might not be inherited when launching an app from the Dock or Start menu.
Project configurations, policy controls, other profiles and already-installed
plugins may take precedence; a saved personal configuration alone does not prove
the client has loaded it.

ChatGPT web/cloud tasks and remote agents cannot reach this local stdio server
through this setup. This release path does not expose a network shell service.

## Update, conflicts and removal

- **Update setup** refreshes the bundled executable path, endpoint and skill.
  Use it after moving/reinstalling Conn. AppImage stages its CLI at a stable path.
- JSONC and TOML edits preserve other entries and comments. Existing `conn`
  entries from another installer are left for manual review, even if they happen
  to match. Configurations and skills edited after setup are also preserved.
- **Remove setup** removes only the unchanged Conn entry and its managed skill.
  A pre-existing identical skill is not claimed by the installer. Copilot's shared
  skill remains until its last managed user is removed. Restart clients to unload
  a running MCP process; removing configuration does not disconnect it immediately.
- Symbolic links, redirected parent paths, invalid or ambiguous configurations,
  and a changed setup location use the manual path rather than being overwritten.
- The ownership record is `<Conn config dir>/integrations.json`. Recovery copies
  are in `integration-backups/` under that directory. Treat them as private: they
  may contain other client settings. Unix backups have mode `0600`; Windows uses
  the user's directory ACL. Files are replaced atomically; detected failures roll
  back completed writes. A power loss or simultaneous external writer can still
  require manual recovery. Close a client that keeps rewriting its config and retry.

The browser test harness runs the same installer against
`<test state>/agent-clients`, never the real client settings. Its configured MCP
process still talks to the native test backend.

## Adapter contract

`crates/frontend/src/integrations` is shared by Tauri and the browser harness:

- `adapters.rs`: stable client IDs, config formats, path resolution and MCP entry
  capabilities. Unknown client IDs fail closed; callers cannot supply arbitrary paths.
- `config.rs`: lossless edits to one Conn entry, independent of client UI.
- `storage.rs`: path checks, compare-before-write, backups, atomic replacement and rollback.
- `mod.rs`: catalog, install/update, removal, manual export and ownership receipts.
- `AgentConnections.svelte`: data-driven cards using the catalog, with EN/KO copy.

To add a client, register its adapter, verified paths/schema and localized hint;
add preservation/lifecycle fixtures. New transports or plugin packaging can be
added behind this boundary without changing terminal, policy or approval code.
Do not infer connection success from a successful file write, auto-approve a
client's tools, or use an agent-provided config path.

## Compatibility evidence

The implementation is based on these official client references (checked
2026-09-16): [Codex MCP](https://developers.openai.com/codex/mcp/),
[Codex skills](https://developers.openai.com/codex/skills/),
[Claude Code MCP](https://code.claude.com/docs/en/mcp),
[Claude skills](https://code.claude.com/docs/en/skills),
[Cursor MCP](https://prod.cursor.com/help/customization/mcp),
[Cursor skills](https://cursor.com/docs/skills),
[VS Code MCP](https://code.visualstudio.com/docs/agent-customization/mcp-servers),
[VS Code skills](https://code.visualstudio.com/docs/agent-customization/agent-skills),
[Copilot CLI MCP](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-mcp-servers),
[Copilot CLI directories](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference).

Automated fixtures exercise all five adapters and Windows-style command/pipe
escaping. Local browser checks use the native backend and an actual MCP process.
A native GUI smoke test with every client on every OS remains a release check;
file-schema tests are not a substitute for it.
