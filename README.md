<p align="center">
  <img src="frontends/tauri/public/conn-icon.svg" width="88" height="88" alt="Conn icon: an open C with a control cursor">
</p>

<h1 align="center">Conn</h1>
<p align="center"><strong>Keep your agent. Share your terminal.</strong></p>
<p align="center">English · <a href="README.ko.md">한국어</a></p>
<p align="center">
  <a href="docs/getting-started.md">Get started</a> ·
  <a href="https://github.com/eggplantiny/conn/releases">Releases</a> ·
  <a href="CONTRIBUTING.md">Contribute</a> ·
  <a href="LICENSE">MIT license</a>
</p>

Conn connects your AI agent to a terminal you can both use. Keep the conversation in your agent client, let it work in the shared shell, and step in by typing. When you are ready, ask it to read the current screen and continue from your changes.

Use Conn as a desktop app or inside your existing terminal. Agents connect through **MCP or CLI**; Conn does not run a model.

[![Watch Codex CLI and Conn: an agent fixes a test, a person adds a case, and the agent continues from the changed terminal.](docs/assets/conn-demo-poster.webp)](docs/assets/conn-demo-en.mp4)

**[Watch the demo](docs/assets/conn-demo-en.mp4)** · [한국어 영상](docs/assets/conn-demo-ko.mp4) · [How it was recorded](docs/demo.md)

*An actual Codex CLI session connected through Conn MCP. The demo project is prepared, user-role input is automated, and waiting time is edited.*

## Work together, take turns

- **Start with your agent.** Connect an MCP client such as Codex CLI. It can read the shared screen and request commands in the same live shell you use.
- **Step in when you need to.** Review a request, or type directly to reclaim control and stop further agent input. Taking control does not cancel a command already running.
- **Continue from what changed.** After your turn, ask the agent to read the screen again and continue. Commands and control changes share a timeline, with the original request available to inspect.

## Get started

### 1. Open Conn

Install Rust through rustup, Node.js 24, and your platform's [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). The repository pins its Rust toolchain.

```sh
git clone https://github.com/eggplantiny/conn.git
cd conn
cargo install --path crates/cli --locked
cd frontends/tauri
npm ci
npm run tauri dev
```

Prefer your existing terminal? After installing the CLI, run `conn` there. That path needs Rust and a native compiler/linker, without Node.js or Tauri. Keep the session open while connecting your agent.

### 2. Connect your agent

For Codex CLI, register Conn and start a new Codex session:

```sh
codex mcp add conn -- conn mcp
```

Keep Conn open while your agent connects. Other clients can register `conn mcp` as an MCP **stdio server**. For clients using JSON configuration:

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

Configuration format and location depend on the client. Use an absolute executable path if it cannot find `conn`. See [agent setup](docs/getting-started.md#connect-your-agent) for endpoints and client options.

### 3. Try one request

In the desktop's top-right controls, select **Autopilot**, turn on **Ask before granting**, and set **Grace** to 2 seconds. Then ask your agent:

> Use Conn for all shell work. Read the current screen, then request control to show the current directory. Include the exact command. Do not change files. Stop if I deny the request or take control back.

Inspect the request and choose **Allow** or **Deny**. After allowing it, type a command yourself to take control. Ask the agent to read the screen again before proposing its next step. Open **Timeline** to review who did what.

Follow the [first collaboration walkthrough](docs/getting-started.md#3-try-the-handover) for command approvals, Co-pilot mode, and CLI-only use.

## Explore

| Task | Guide |
|---|---|
| Install, connect, and troubleshoot | [Getting started](docs/getting-started.md) |
| Configure local shells, WSL, SSH, or Docker | [Backends and profiles](docs/backends.md) |
| Choose approval and execution rules | [Policy](docs/policy.md) |
| Integrate an agent or frontend | [Protocol](docs/protocol.md) · [Architecture](docs/architecture.md) |
| Test or contribute | [Browser testing](docs/browser-testing.md) · [Contributing](CONTRIBUTING.md) |
| Check packages and release requirements | [Platform support](docs/platform-support.md) · [Releasing](docs/releasing.md) |

## Preview status

**v0.3.0 preview.** Start with the source setup above. Linux, macOS, and Windows packaging targets and validation status are tracked in [platform support](docs/platform-support.md); a passing build does not establish installer or interactive compatibility.

Conn is a collaboration and mistake-prevention layer, **not a security sandbox**. Its timeline covers actions routed through Conn; an `executed` record means input reached the shell, not that a command succeeded. Requests and screen snapshots can contain sensitive text. See the [trust model](docs/security.md) and [security reporting policy](SECURITY.md).
