# Getting started with Conn

English · [한국어](getting-started.ko.md) · [Back to Conn](../README.md)

Install Conn, connect your agent, and take turns in the same terminal. The desktop download includes the CLI needed for agent connections; Rust, Node.js and a development server are not required.

## 1. Install and open a session

One line, if you prefer:

```sh
brew install --cask eggp-dev/tap/conn                                                        # macOS, Apple Silicon
curl -fsSL https://raw.githubusercontent.com/eggp-dev/conn/main/scripts/install.sh | sh     # Linux x86_64
```

The cask installs the signed and notarized DMG and checks it against the SHA-256 published with the release. The Linux script downloads the AppImage, verifies it against the release's `SHA256SUMS`, and installs it for your user only (`~/.local/bin`, an app-menu entry, no sudo); it refuses to install on a checksum mismatch. `CONN_VERSION=v0.8.2` pins a release. AppImages need FUSE 2: on Ubuntu 24.04 and newer, `sudo apt install libfuse2t64`. Both keep updating from inside the app. Read the script before piping it to a shell if you like: [`scripts/install.sh`](../scripts/install.sh).

Or download a file yourself:

Download the [v0.8.2 preview](https://github.com/eggp-dev/conn/releases/tag/v0.8.2) for your OS and CPU. See the release notes for the exact signing and runtime verification results.

| Platform | Download | Install |
|---|---|---|
| Ubuntu x64 | [`.deb`](https://github.com/eggp-dev/conn/releases/download/v0.8.2/conn-v0.8.2-x86_64-unknown-linux-gnu-desktop.deb) | Run the command below, then open Conn from your applications. |
| Ubuntu x64 | [AppImage](https://github.com/eggp-dev/conn/releases/download/v0.8.2/conn-v0.8.2-x86_64-unknown-linux-gnu-desktop.AppImage) | Make the file executable, then open it. |
| macOS Apple Silicon | [`.dmg`](https://github.com/eggp-dev/conn/releases/download/v0.8.2/conn-v0.8.2-aarch64-apple-darwin-desktop.dmg) | Open the DMG, drag Conn into Applications, then launch it there. |
| Windows x64 | [Installer `.exe`](https://github.com/eggp-dev/conn/releases/download/v0.8.2/conn-v0.8.2-x86_64-pc-windows-msvc-setup.exe) | Run the installer, then open Conn from the Start menu. |

Intel Mac distribution is paused for future releases. Previously published Intel packages remain in [past releases](https://github.com/eggp-dev/conn/releases); do not install the Apple Silicon package on an Intel Mac.

For Ubuntu, run the matching command in your download directory:

```sh
sudo apt install ./conn-v0.8.2-x86_64-unknown-linux-gnu-desktop.deb
```

Or, for AppImage:

```sh
chmod +x ./conn-v0.8.2-x86_64-unknown-linux-gnu-desktop.AppImage
./conn-v0.8.2-x86_64-unknown-linux-gnu-desktop.AppImage
```

The Linux build targets Ubuntu 24.04 and 26.04 x64; AppImage still depends on system libraries. Windows previews are intentionally unsigned, so SmartScreen or an unknown-publisher prompt may appear; managed PCs may block installation. Public Mac assets must pass the release's Developer ID signing and notarization checks. See [platform support](platform-support.md) for the limits of each target.

Optional: download [SHA256SUMS](https://github.com/eggp-dev/conn/releases/download/v0.8.2/SHA256SUMS) alongside your file. On Linux, run `sha256sum --ignore-missing -c SHA256SUMS`; on macOS, compare `shasum -a 256 <file>` with its line; on Windows, use `Get-FileHash <file> -Algorithm SHA256`. Checksums detect corruption and are separate from code signing.

## 2. Set up your terminal

Open **the C icon at the top left → Settings**.

1. Under **Profiles**, choose a local shell, startup directory, and default profile. Use **+** to open a new tab with that profile; existing tabs keep theirs.
2. Under **Appearance → Language**, choose English or 한국어. Handoff effects can be disabled; the icon also respects the OS reduced-motion setting.
3. Close Settings and open the **top-right control indicator**. For your first request, choose **Autopilot**, enable **Ask before granting**, and set **Grace** to 2 seconds.

Autopilot allows agent execution within policy. **Co-pilot** lets you accept each proposed line with Enter; **Observe** allows reading without agent writes.

## Connect your agent

In **Settings → Agents → Connect your agent**:

1. Choose Codex / ChatGPT local tasks, Claude Code, Cursor, GitHub Copilot in VS Code, or Copilot CLI.
2. Click **Set up** to register the MCP server and collaboration skill.
3. Restart or reconnect your client following the card's hint. Keep Conn open and ask your agent to read the current terminal.
4. The first time an agent connects, Conn shows **wants to join**. Press **Allow**. The agent's waiting call continues by itself. Each new connection asks once; turn this off under **Settings → Agents → Ask before a new agent joins** if you prefer.

**Configured** means the files are saved; **Connected now** appears while the client is attached to Conn. Client trust prompts and command approvals remain separate. [Client paths, updates, removal and troubleshooting](agent-integrations.md).

The setup uses the bundled CLI's absolute path and current endpoint. Rust, Cargo, Node.js and PATH changes are not required; AppImage stages its CLI at a stable path. After moving or reinstalling Conn, use **Update setup**. Existing manually registered Conn entries are preserved for review.

For other clients, expand **Other MCP clients · manual setup** and copy MCP JSON or Codex TOML. Client formats differ; JSON is configuration data, not a shell command. This local connection does not provide a remote server for ChatGPT web/cloud tasks.

### Manual configuration and endpoints

For a separately installed CLI on `PATH`, Codex CLI can register it with:

```sh
codex mcp add conn -- conn mcp
```

The default endpoint is `~/.conn/conn.sock` on Unix or a local per-user named pipe on Windows. **Settings → Diagnostics** shows the current endpoint. For a nondefault session, use `conn --socket <endpoint> mcp` or `CONN_SOCKET`. The copied desktop configuration includes this automatically. The browser test adapter has its own endpoint.

## 3. Try the handover

Ask your agent:

> Use Conn for all shell work. Read the current screen, then request control to show the current directory. Include the exact command. Do not change files. If I deny the request or take control, stop and tell me what happened.

1. Inspect the control request. **View original request** shows its arguments and the planned command when supplied.
2. Choose **Allow**. Review any separate command approval too. During grace, **Enter** runs now and **Esc** cancels.
3. Read the result together. Ask the agent to read another snapshot rather than treating delivery of Enter as success.
4. Type a command yourself to take control back. When ready, ask the agent to read the changed screen and continue.
5. Open **Timeline** to review the commands and control changes. You can also deny a request and verify that it never runs; retry only when you explicitly ask for it.

**Taking control does not undo or stop a command already running.** It prevents further agent input. Use the terminal's normal interrupt, such as Ctrl-C, to stop a foreground process.

The open-C icon shows human control, pending requests, agent control and execution grace. It is a status cue, not an approval button.

## 4. Read the timeline

**All** combines commands and collaboration; **Commands** and **Collaboration** filter them. Expand **Details** for the event sequence and **View original request** for the recorded method and arguments. Older records or clients may lack a planned command.

Denied, blocked, cancelled, expired and executed are separate outcomes. **Executed** means input reached the shell, not that the command succeeded. The timeline does not save terminal output or scrollback.

## CLI and MCP (shared-surface development version)

Open the **Conn desktop app** first. The CLI connects an agent to that visible
session; it no longer launches a terminal proxy or a headless collaboration shell.
Choose the mode, control gate and pacing in Conn's control centre or Settings.

```sh
conn --version
conn mcp --agent-id my-client
```

Agent clients normally launch `conn mcp` themselves using the configuration from
Settings. Keep that connection alive when the user selects it as a participant;
a different CLI process receives a different connection ID, even with the same name.

For a one-off read-only request in an ordinary shared tab:

```sh
conn agent --agent-id demo run "pwd" --reason "Show the current directory without changing files"
```

Use `cd` for cmd.exe. Human takeover, approval and mode changes belong in the app.
`conn log -n 30` explicitly reads the local audit file using your OS permissions;
it is not an agent observation endpoint. Remote and non-POSIX execution still
requires command review. These CLI changes take effect in v0.7.0. Restart the app and MCP clients together.

## Common questions

| Symptom | What to check |
|---|---|
| Agent cannot find the CLI | Copy the configuration from Settings again. The desktop connection uses an absolute path and needs no `PATH` change. |
| Agent cannot connect | Keep Conn open and use its current endpoint. Copy the configuration again after moving the app. |
| `admission_pending` | Press **Allow** on the "wants to join" card in the Conn window, then let the agent retry. |
| Agent reads but cannot type | Check Observe mode, tool permissions and control ownership. Your typing takes control back; the agent must request it again. |
| Request stays pending | Look for a control request, command approval, or Co-pilot proposal. |
| Agent works in a tab you are not viewing | Expected: access follows sharing, mode and control, not window focus or the visible tab. Stop sharing or use Observe mode to restrict it. |
| `surface_unavailable` | The terminal changed after the agent last read it. Take a fresh snapshot and retry. |
| Compound command is blocked | With `isolate_dangerous: true`, submit navigation, the dangerous action, and verification separately. |
| Command is still running | Wait and read another snapshot; a submission result is not an exit status. |

Policy lives under **Settings → Policy** or `~/.conn/policy.yaml`. Approval cannot override a policy block. Configuration and audit data normally live in `~/.conn` (`%USERPROFILE%\.conn` on Windows). Requests and profile environment values may contain sensitive text; review before sharing.

## From source

Source builds are for development or custom changes. Install Rust through rustup and a native compiler/linker; desktop builds also need **Node.js 24** and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). The repository pins the Rust toolchain.

```sh
git clone https://github.com/eggp-dev/conn.git
cd conn
cargo install --path crates/cli --locked
cd frontends/tauri
npm ci
npm run tauri dev
```

The shared-surface development version requires a visible Conn frontend. For browser testing with the same native backend, see [browser testing](browser-testing.md).

Next: [backends](backends.md), [policy](policy.md), [trust model](security.md), or [contributing](../CONTRIBUTING.md).

## Updates (v0.6.0 and later)

Conn checks in the background and downloads signed updates. The Conn menu shows **Install and restart** when an update is ready. You choose when to install; the confirmation explains that all shell sessions will close. Settings and stored timeline records remain.

Use **Check for updates → Update preferences** to disable automatic checks/downloads or change the preview channel. Preview builds include previews by default; stable builds do not. A channel change never downgrades your app. No GitHub account or token is needed.

In-app installation supports Apple Silicon macOS, Windows x64 and Linux AppImage. `.deb` installations use your package manager or a new installer from Releases. Existing 0.5.x installations need one manual installation of 0.7.0 to receive the updater. Failed checks/downloads leave your running shell untouched; use Retry or the release page.

## Same-session sharing and extensions (v0.7.0)

External launches start private. Use the sharing control in that window to choose
connected agents and share its current viewport. The existing external writer is
revoked; the shell and SSH connection continue. Stop sharing to continue alone.
Earlier private input/output is not added to history.

**Settings → Extensions** contains themes and opt-in command suggestions. Choose a
model and save an API key in the OS credential store. The current shared viewport
may be sent to the provider. Suggestions insert text only after acceptance; Enter
is a separate action. [Extension guide](extensions.md)
