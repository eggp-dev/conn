# Getting started with Conn

English · [한국어](getting-started.ko.md) · [Back to Conn](../README.md)

The first goal is simple: open a terminal, let an agent request one read-only command, and see your decision in the timeline.

## 1. Install and open a session

### From a release

Use the files attached to a [published release](https://github.com/eggplantiny/conn/releases). The release workflow targets Linux `.deb`/`.AppImage`, macOS `.dmg`, Windows NSIS `.exe`, and separate CLI archives. If no release has been published yet, build from source below.

Windows previews are intentionally unsigned: SmartScreen or unknown-publisher prompts may appear, and managed PCs may block installation. Public Mac downloads are planned after Developer ID signing and notarization; current ad-hoc Mac artifacts are for testing. Check each release's actual signing and validation notes. Checksums detect download corruption, not publisher identity. See [platform support](platform-support.md); no published release means no completed installer validation claim.

Choose the asset for your OS and CPU architecture:

| Platform | Desktop installation |
|---|---|
| Ubuntu 24.04 / 26.04 x64 | Download the `.deb`, then run `sudo apt install ./conn-v0.3.0-x86_64-unknown-linux-gnu-desktop.deb` in its directory. |
| Ubuntu x64 AppImage | Run `chmod +x ./conn-v0.3.0-x86_64-unknown-linux-gnu-desktop.AppImage`, then launch that file. Your system needs the runtime libraries required by the AppImage. |
| macOS | Choose Apple Silicon (`aarch64`) or Intel (`x86_64`), open the `.dmg`, and drag Conn into Applications. Check the release notes for the signed/notarized package; ad-hoc draft builds remain test-only. |
| Windows x64 | Run the `-setup.exe` installer and launch Conn from the Start menu. The preview has no verified publisher certificate. |

Linux release files are built on Ubuntu 24.04 and must be tested on both 24.04 and 26.04. Other distributions and older Ubuntu releases are not yet validated; an AppImage is not universal Linux compatibility.

For another version, replace `v0.3.0` with the version you downloaded. Keep the checksum file from the same release. The desktop contains its matching CLI, while the separate CLI archive is useful for configuring an agent client on `PATH`.

For a CLI archive, extract `conn` (`conn.exe` on Windows) to a directory on your user `PATH`. Reopen your terminal and verify:

```sh
conn --version
```

On Unix, the desktop's command palette also offers **Install the conn CLI on PATH** when a bundled CLI is available. Settings → Diagnostics shows the bundled executable and the executable found on `PATH`. On Windows, use the CLI archive or Cargo installation.

### From source

For the CLI, install Rust with rustup and your platform's native compiler/linker. For the desktop, also install **Node.js 24** and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). `rust-toolchain.toml` selects Rust 1.95.0.

```sh
git clone https://github.com/eggplantiny/conn.git
cd conn
cargo install --path crates/cli --locked
```

Choose one frontend:

**Desktop** — from the repository:

```sh
cd frontends/tauri
npm ci
npm run tauri dev
```

**Existing terminal** — run:

```sh
conn
```

Keep that session open. `conn mcp` connects agents to an existing session; it does not open a terminal for you. For a first run, use one Conn frontend on the default endpoint. Separate instances need separate endpoints.

## 2. Set up your terminal

In the desktop, click the **C icon at the top left → Settings**.

1. Under **Profiles**, choose a local shell and startup directory. Detect installed shells if needed, then select your default profile.
2. Under **Appearance → Language**, choose English or 한국어. Turn off handoff effects if you prefer less motion; the icon also respects the operating system's reduced-motion preference.
3. Close settings. The **+** button opens a new tab with the default profile. Existing tabs keep their original profile.
4. Click the **top-right control indicator**. Select **Autopilot**, enable **Ask before granting**, and set **Grace** to 2 seconds for this walkthrough.

Autopilot permits agent execution within policy; it does not override command approvals. For a workflow where you accept every proposed line with Enter, select **Co-pilot** instead. **Observe** allows reading without agent writes.

If using the terminal frontend, set the walkthrough options from another terminal:

```sh
conn mode autopilot
conn gate --ask
conn pacing --enter-grace-ms 2000
conn status
```

These commands change the running session's options. They do not start another shell.

## Connect your agent

Register this command as an MCP stdio server in your agent client:

```text
Executable: conn
Arguments:  mcp
```

A client that accepts an `mcpServers` JSON object can use:

```json
{
  "mcpServers": {
    "conn": {
      "command": "conn",
      "args": ["mcp"]
    }
  }
}
```

Configuration formats and locations differ by client. This is a server definition, not a command to paste into your terminal. Use an absolute path for `command` when the agent's environment cannot find your CLI. On Windows, use the full `conn.exe` path with JSON-escaped backslashes if needed.

Conn derives the agent name from the MCP client's initialization. To choose a stable name, use arguments `mcp --agent-id my-agent`. Use `mcp --tools static` if your client cannot refresh a changing tool list. Automatic mode selects the static list for client names recognized as Codex; permission checks still run for every call.

The adapter provides instructions during MCP initialization. [The companion skill](../plugin/skills/conn/SKILL.md) explains the workflow, and `conn guide` prints it. The [plugin directory](../plugin/README.md) contains both the MCP definition and the skill for clients that can load them.

### Choose the right endpoint

The default endpoint is `~/.conn/conn.sock` on Unix, or a local per-user named pipe on Windows. Check **Settings → Diagnostics** for the endpoint used by the desktop.

For a nondefault endpoint, pass the displayed value explicitly:

```text
Executable: conn
Arguments:  --socket <endpoint-from-diagnostics> mcp
```

`CONN_SOCKET` also selects the endpoint. The local browser test harness prints its own endpoint; it does not share the desktop's session automatically.

## 3. Try the handover

Ask your agent:

> Use Conn for all shell work. Read the current screen, then request control to show the current directory. Include the exact command in the request. Do not change files. If I deny or interrupt, stop and tell me what happened.

For a CLI-only demonstration, run this in **another terminal**, not inside the shared session:

```sh
conn agent --agent-id demo run "pwd" --reason "Show the current directory without changing files"
```

Use `cd` instead of `pwd` for cmd.exe. Non-POSIX and remote profiles require command review even if the normal policy would allow the command.

In the desktop:

1. The control request appears. Expand **View original request** to inspect `request_control`, its arguments, and the planned command.
2. Choose **Deny** for your first attempt. The command must not run. Open the bottom **Timeline** and inspect the denied request.
3. Explicitly ask the agent to try again, or run the CLI demonstration again yourself. This time choose **Allow**.
4. If a command approval appears, inspect and decide that separately. During execution grace, **Enter** runs now and **Esc** cancels.
5. Read the terminal output. The agent should also take another snapshot and report what it actually sees.

The open-C icon reflects the active tab's state. It moves from human control to a waiting request, agent control, and execution grace; a policy block, pause, or disconnected shell has its own appearance. It is a status cue, not an approval button.

**Taking control does not undo a running command.** Human terminal input stops further agent writes; use the terminal's normal interrupt behavior, such as Ctrl-C, when you also need to interrupt the foreground process. From another terminal, `conn take` reclaims control without inserting characters into the shared shell.

## 4. Read the timeline

The desktop timeline combines command history and collaboration activity:

- **All** groups related control events and command execution.
- **Commands** focuses on command input.
- **Collaboration** shows control requests and handovers.
- **Details** shows the event sequence, intent, and available policy information.
- **View original request** shows the recorded method and arguments. A planned command is metadata; it is not proof that the command was executed.

User denial, policy blocking, cancellation, expiry, and execution are separate outcomes. Older records may have no original request, and a client may omit the planned command. Conn says when this information is unavailable rather than inventing it.

An **executed** record means Conn sent the command to the shell. It is not an exit code or a completion result. The timeline is not a terminal recording; output and scrollback are not saved in the audit log.

From a terminal, inspect the audit with:

```sh
conn log -n 30
conn log --json --actor demo
```

## Common questions

| Symptom | What to check |
|---|---|
| `conn` is not found | Reopen the terminal after updating `PATH`, or use the full executable path in MCP configuration. |
| The agent cannot connect | Keep a Conn session open. Compare the endpoint with Diagnostics; browser tests use a separate one. |
| The agent can read but not type | Check Observe mode, the agent tool mask, control ownership, and whether you are viewing its tab. |
| A request stays pending | There may be a control request, a command approval, or a Co-pilot proposal waiting for your decision. |
| `unattended` or `suspended` | Return to that tab or explicitly entrust it. The agent cannot change your visible tab. |
| A compound command is blocked | With `isolate_dangerous: true`, a dangerous action must be alone. Run navigation, the action, and verification as separate requests. |
| A denied command returns CLI output | Read `result:` and the timeline. `agent run` prints a screen snapshot; its successful process exit is not proof the requested shell command ran. |
| A long command has not finished | `agent run` takes a snapshot shortly after submission. Wait and request another snapshot; it does not track shell exit status. |
| Reloading the browser ends the shell | Expected for the development harness. Reload deliberately; files and saved audit records remain, the live session does not. |

Policy is configurable under **Settings → Policy** or in `~/.conn/policy.yaml`. A policy block cannot be lifted with an approval. Read the [policy reference](policy.md) before changing rules.

## Local data and next steps

Configuration and audit data normally live under `~/.conn` (`%USERPROFILE%\.conn` on Windows). They include profiles, policy, defaults, and audit records. The frontend also keeps appearance preferences locally. Profile environment values and request arguments may contain sensitive text; inspect and redact them before sharing a report.

Next: [backends and profiles](backends.md), [trust model](security.md), [browser testing](browser-testing.md), or [contributing](../CONTRIBUTING.md).
