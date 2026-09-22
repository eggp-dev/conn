# Browser testing with the native backend

Run from `frontends/tauri`:

```sh
npm ci
npm run test:browser
```

Open http://127.0.0.1:1421/ in one browser tab. Multiple shell tabs work inside it.

The browser loads the existing Svelte entry point, App, components and stores.
Only command/event transport changes: Tauri IPC in the desktop, a loopback
WebSocket in browser-test mode. Both adapters call `conn-frontend::Harness`,
which owns the existing native Engine, PTY, profiles, policy, audit and agent hub.
There is no WASM shell or separate web UI/state implementation.

The launcher builds the Rust test server and starts Vite. Rust and the normal
frontend Node dependencies are required; the test server does not need WebKit.
Ports 1421 and 1423 must be free. The WebSocket checks the exact frontend origin
and a per-run token. This is a local development harness, not a deployed service.

Profiles, policy, audit, defaults and the agent endpoint live in the printed
`/tmp/conn-browser-test-*` directory (the platform temporary directory on Windows).
Set `CONN_TEST_STATE` to an existing directory to reuse its settings.
The shell is a real native shell with the current user's filesystem access;
the separate settings directory is not an OS sandbox.

The desktop session is separate and remains running. Browser reload/disconnect
ends this harness's sessions. HMR is disabled to preserve shells while editing;
reload deliberately when ready to test frontend edits. Stop the launcher with
Ctrl-C. Never share the connection token or expose these ports publicly.

Native desktop builds use the same shared commands through the thin Tauri
adapter. Changes to shared UI therefore apply to both execution paths.

## Separate recording session

Use another state directory and pair of ports to record the same UI without
restarting an existing test session. Build from the repository root:

```sh
cargo build -p conn-browser-harness --locked
CONN_TEST_PORT=1433 CONN_TEST_ORIGIN=http://127.0.0.1:1431 \
  target/debug/conn-browser-harness /tmp/conn-film
```

In a second terminal, from `frontends/tauri`:

```sh
CONN_TEST_STATE=/tmp/conn-film \
  node node_modules/vite/bin/vite.js --mode browser-test --host 127.0.0.1 --port 1431
```

Open http://127.0.0.1:1431/. `CONN_TEST_PORT` defaults to `1423`;
`CONN_TEST_ORIGIN` defaults to `http://127.0.0.1:1421` and accepts only an exact
`http://127.0.0.1:<port>` origin. The backend always binds to loopback and retains
its per-run token check. These overrides apply to the manual launch above; the
`npm run test:browser` launcher continues to use its default ports.

Prepare `profiles.json` and `app.json` in the new state directory before opening
the page to select a demo working directory, clean shell prompt and pacing.
The new browser origin and state directory keep its timeline separate. Use a
disposable demo project; state separation still does not sandbox shell access.

## Agent setup isolation

The harness supplies `<test state>/agent-clients` as the client home for integration
setup. The Settings UI and native installer are unchanged; only path resolution
is injected. Clicking Set up, Update setup or Remove setup changes these disposable
files, never your real Codex/Claude/Cursor/Copilot configuration. Do not launch a
client against this temporary configuration unless you intend to connect it to
the test backend. CLI staging and backups remain under the test state directory.

## Pure state checks

Run `npm test` in `frontends/tauri` for the timeline, icon-state,
connection-status and terminal-input checks. They need neither a browser nor the native backend.

## Collaboration context and screen parity

Build `cargo build -p conn-browser-harness --locked` at the repository root, then run
`npm run test:context:harness` from `frontends/tauri`. This uses the same UI, Rust backend,
real local PTYs and persistent agent sockets. It covers background tab requests and notification
targets, human takeover, fresh connection admission, retained shell state, Observe navigation,
sessionless preparation, explicit sharing, EN/KO and a narrow viewport. It uses ports 1461/1463;
override `CONN_HARNESS_UI_PORT` and `CONN_HARNESS_PORT` if needed.
Screenshots, video, state and grid comparisons are written under the printed temporary directory.
`CONN_CONTEXT_OUTPUT` overrides the evidence location. Native attention delivery is not simulated.

`npm run test:screen` compares the installed xterm parser with a JSON-lines probe of the real Rust
`ScreenModel`. It checks rows and cursor positions after ASCII, Korean, combining marks, emoji,
soft wraps, scroll, height/width changes and alternate-screen transitions. The browser test separately
compares actual rendered rows against agent snapshots. Neither test proves every terminal program,
Unicode sequence, SSH connection or native window-manager behavior.

## SSH input, cursor and approval regression

After building the Rust harness, run `npm run test:input:harness` from
`frontends/tauri`. It uses ports 1471/1473 and writes screenshots plus rendered-grid
and cursor comparisons to the printed temporary directory (`CONN_INPUT_REVIEW_OUTPUT`
overrides it). The default uses a local Bash PTY.

To exercise real SSH, set `CONN_AUTH_FIXTURE_RUNTIME` to the disposable
`terminal-auth-fixtures/.runtime` directory before running it. The fixture must
already be listening on `127.0.0.1:22222`; the test accepts only its synthetic
password and pinned known-hosts file. It authenticates through the same visible PTY
and uses remote Bash with `LC_ALL=C.UTF-8` and `TERM=xterm-256color`.

Coverage includes long ASCII/Unicode input, short/wrapped JSON with no trailing newline versus
LF/CRLF (including prompt placement), settings modal open/close without PTY
resizing, intentionally delayed resizes to expose native IPC and output ordering, and fixed-window
approval open/deny with actual terminal motion and a delayed dock shrink. It also checks pane coverage
at several emulated pixel densities, a long risky command held for approval while resizing the window, the
session-allow button and its limited scope, a separate risk still requesting approval,
policy denial, human cursor movement/insertion, takeover, and return from SSH to the
original local shell. No privileged command is approved. This is Linux Chromium
coverage of the production UI and real Rust/PTY/SSH path, not macOS WebKit acceptance.
