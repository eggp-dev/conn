# Getting started with Conn

English · [한국어](getting-started.ko.md) · [Back to Conn](../README.md)

Install Conn, connect your agent, and take turns in the same terminal. The desktop download includes the CLI needed for agent connections; Rust, Node.js and a development server are not required.

## 1. Install and open a session

Download the [v0.4.1 preview](https://github.com/eggplantiny/conn/releases/tag/v0.4.1) for your OS and CPU. See the release notes for the exact signing and runtime verification results.

| Platform | Download | Install |
|---|---|---|
| Ubuntu x64 | [`.deb`](https://github.com/eggplantiny/conn/releases/download/v0.4.1/conn-v0.4.1-x86_64-unknown-linux-gnu-desktop.deb) | Run the command below, then open Conn from your applications. |
| Ubuntu x64 | [AppImage](https://github.com/eggplantiny/conn/releases/download/v0.4.1/conn-v0.4.1-x86_64-unknown-linux-gnu-desktop.AppImage) | Make the file executable, then open it. |
| macOS Apple Silicon | [`.dmg`](https://github.com/eggplantiny/conn/releases/download/v0.4.1/conn-v0.4.1-aarch64-apple-darwin-desktop.dmg) | Open the DMG, drag Conn into Applications, then launch it there. |
| macOS Intel | [`.dmg`](https://github.com/eggplantiny/conn/releases/download/v0.4.1/conn-v0.4.1-x86_64-apple-darwin-desktop.dmg) | Open the DMG, drag Conn into Applications, then launch it there. |
| Windows x64 | [Installer `.exe`](https://github.com/eggplantiny/conn/releases/download/v0.4.1/conn-v0.4.1-x86_64-pc-windows-msvc-setup.exe) | Run the installer, then open Conn from the Start menu. |

For Ubuntu, run the matching command in your download directory:

```sh
sudo apt install ./conn-v0.4.1-x86_64-unknown-linux-gnu-desktop.deb
```

Or, for AppImage:

```sh
chmod +x ./conn-v0.4.1-x86_64-unknown-linux-gnu-desktop.AppImage
./conn-v0.4.1-x86_64-unknown-linux-gnu-desktop.AppImage
```

The Linux build targets Ubuntu 24.04 and 26.04 x64; AppImage still depends on system libraries. Windows previews are intentionally unsigned, so SmartScreen or an unknown-publisher prompt may appear; managed PCs may block installation. Public Mac assets must pass the release's Developer ID signing and notarization checks. See [platform support](platform-support.md) for the limits of each target.

Optional: download [SHA256SUMS](https://github.com/eggplantiny/conn/releases/download/v0.4.1/SHA256SUMS) alongside your file. On Linux, run `sha256sum --ignore-missing -c SHA256SUMS`; on macOS, compare `shasum -a 256 <file>` with its line; on Windows, use `Get-FileHash <file> -Algorithm SHA256`. Checksums detect corruption and are separate from code signing.

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

## Existing-terminal and CLI use

Download the CLI archive for [Linux x64](https://github.com/eggplantiny/conn/releases/download/v0.4.1/conn-v0.4.1-x86_64-unknown-linux-gnu-cli.tar.gz), [Mac Apple Silicon](https://github.com/eggplantiny/conn/releases/download/v0.4.1/conn-v0.4.1-aarch64-apple-darwin-cli.tar.gz), [Mac Intel](https://github.com/eggplantiny/conn/releases/download/v0.4.1/conn-v0.4.1-x86_64-apple-darwin-cli.tar.gz), or [Windows x64](https://github.com/eggplantiny/conn/releases/download/v0.4.1/conn-v0.4.1-x86_64-pc-windows-msvc-cli.zip). Extract `conn` (`conn.exe` on Windows) to a directory on your user `PATH` and reopen your terminal.

```sh
conn --version
conn
```

Keep that session open. In another terminal, configure the walkthrough and send a read-only request:

```sh
conn mode autopilot
conn gate --ask
conn pacing --enter-grace-ms 2000
conn agent --agent-id demo run "pwd" --reason "Show the current directory without changing files"
```

Use `cd` instead of `pwd` for cmd.exe. `conn take` reclaims control; `conn log -n 30` reads recent audit entries. `agent run` prints a snapshot after submission and does not track shell exit status. Non-POSIX and remote profiles require command review.

## Common questions

| Symptom | What to check |
|---|---|
| Agent cannot find the CLI | Copy the configuration from Settings again. The desktop connection uses an absolute path and needs no `PATH` change. |
| Agent cannot connect | Keep Conn open and use its current endpoint. Copy the configuration again after moving the app. |
| Agent reads but cannot type | Check Observe mode, tool permissions, control ownership, and whether its tab is visible. |
| Request stays pending | Look for a control request, command approval, or Co-pilot proposal. |
| `unattended` or `suspended` | Return to that tab or explicitly entrust it. The agent cannot change your visible tab. |
| Compound command is blocked | With `isolate_dangerous: true`, submit navigation, the dangerous action, and verification separately. |
| Command is still running | Wait and read another snapshot; a submission result is not an exit status. |

Policy lives under **Settings → Policy** or `~/.conn/policy.yaml`. Approval cannot override a policy block. Configuration and audit data normally live in `~/.conn` (`%USERPROFILE%\.conn` on Windows). Requests and profile environment values may contain sensitive text; review before sharing.

## From source

Source builds are for development or custom changes. Install Rust through rustup and a native compiler/linker; desktop builds also need **Node.js 24** and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). The repository pins the Rust toolchain.

```sh
git clone https://github.com/eggplantiny/conn.git
cd conn
cargo install --path crates/cli --locked
cd frontends/tauri
npm ci
npm run tauri dev
```

For the existing-terminal frontend, run `conn` after the Cargo installation; Node.js and Tauri are unnecessary for that path.

Next: [backends](backends.md), [policy](policy.md), [trust model](security.md), or [contributing](../CONTRIBUTING.md).

## Check for updates (v0.4.1+)

Open the Conn icon menu → **Check for Updates**. The app checks public GitHub releases when requested, compares versions, and shows release notes and a download button for your platform. **Include preview releases** is on by default for this preview and remembers your choice. Network failures offer retry; you can also open the releases page. No account or token is required.

The download opens in your system browser. Finish terminal work and quit Conn before replacing the app. On Linux the shortcut downloads the AppImage; `.deb` packages are also available on the releases page. This does not install updates or restart Conn automatically. Existing v0.4.0 installations need a manual download to receive this feature.
