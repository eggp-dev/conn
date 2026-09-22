# Conn web host and browser validation

[한국어](browser-testing.ko.md) · [Architecture](architecture.md) · [Boundary design](web-harness-design.md)

The web frontend is a workspace package, `@conn/web`. It mounts the same `@conn/ui` ConnApp, components, runes, terminal queues and collaboration actions as the native app. `conn-web` and Tauri call the same Rust `AppRuntime`. Only host transport, native attention, links and updates differ.

## Run locally

From the repository root, with the pinned Rust toolchain and Node 24 or newer:

```sh
npm ci
npm run dev:web
```

Open `http://127.0.0.1:1429`. Enter `bootstrapToken` from the private connection file printed by the server. The browser receives an HttpOnly, SameSite cookie; the token is not a WebSocket query parameter or a localStorage value. The dev launcher builds both `conn-web` and the matching `conn` MCP binary. It keeps state in `~/.config/conn-web-dev` unless `CONN_WEB_STATE` is set. `CONN_WEB_PORT` and `CONN_WEB_BACKEND_PORT` override the frontend/backend ports (1429/1430). Use `CONN_WEB_SETUP_HOME` to isolate agent setup files during experiments.

For an MCP client, use the matching binary and `agentSocket` from that connection file:

```sh
target/debug/conn --socket /path/to/state/conn.sock mcp --agent-id your-agent
```

Keep the MCP process alive across steps. Starting a new process creates a new connection and correctly requires fresh admission. The connection code belongs to the human owner, not the agent endpoint.

## Operate a built package

```sh
npm run package:web
```

The printed directory contains `conn-web`, matching `conn`, `ui/`, a manifest, checksums and startup instructions. Node and Vite are build tools; the assembled package runs without them:

```sh
./conn-web serve --state-dir /path/to/conn-state --port 1423
```

Shells run as the host user. Only loopback binding is supported in this first implementation. The adapter boundary allows future remote operation, but remote authentication, TLS, deployment and multiple users are not implemented by this local package.

## Lifetime and control

- The server process owns PTYs. Browser reload or disconnection detaches the renderer and retains running shells. Stopping the server ends them; restarting a server is not shell recovery.
- A shell set has one active human input/size renderer. A second view asks to continue there; accepting it fences the previous view. It does not grant an agent admission or control.
- A renderer receives a checkpoint and subsequent sequenced output. Until the checkpoint is parsed, input is disabled. Gaps request a fresh checkpoint, never replay user input.
- `input`, `terminal_response` and `resize` share a per-session FIFO in both clients and the Rust owner. Slow unrelated work does not serialize terminal delivery.
- Admission, selected sharing, a control lease and command review remain separate. Reconnecting an agent by the same name inherits none of the old connection's permissions.

## Reproduce verification

```sh
cargo test --locked
cargo build --locked -p conn-web -p conn
npm run build:web
npm run test:ui
npm run test:screen -w @conn/ui
npm run test:checkpoint -w @conn/ui
npm run test:app-lifetime -w @conn/ui
npm run test:collaboration
node scripts/generate-owner-contract.mjs --check
node scripts/check-ui-boundaries.mjs
```

Install the pinned Chromium browser first with `npm exec -w @conn/collaboration-tests -- playwright install chromium` if needed. Collaboration tests serve the production web bundle with the real Rust backend on temporary ports, then use a persistent real MCP process and owner UI clicks/keys. They do not import app state, replace the app or synthesize its approvals. Evidence is written to a printed temporary directory.

The separate app-lifetime test mounts actual ConnApp instances with controlled host ports to test isolation, error paths and disposal. It is not evidence of a real shell or native IPC. Parser/checkpoint tests compare the real Rust terminal model and installed xterm; their fixtures do not establish compatibility with every terminal extension.

## SSH input, cursor and approval regression

```sh
npm run test:input -w @conn/collaboration-tests
```

The default is a local Bash PTY. For the disposable SSH fixture, set `CONN_AUTH_FIXTURE_RUNTIME` to its `.runtime` directory. The fixture must already listen on `127.0.0.1:22222` with a synthetic password and pinned known-hosts file. Authentication is typed into the visible PTY. The test uses real MCP, the production UI and Rust host, and stores rendered row/cursor comparisons and screenshots outside the checkout.

Browser results are Linux Chromium evidence. They do not establish macOS WebKit, native window-manager notifications, native installation or release completion. Previous native results remain documented separately in [SSH input review results](ssh-input-review-results.md).
