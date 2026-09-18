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

## Shared-surface rendering fixtures

The unreleased renderer checks use `tests/browser/surface.html` and
`tests/browser/completion.html` in the frontend development server. The first mounts
real xterm DOM output to exercise masking, conceal, Unicode, scrolling, alternate
screens and raster capture. The second mounts the real completion component with a
mock command transport to test prompt gating and insertion. Neither contacts a model.
Run `npm test` for the corresponding pure surface/state checks, and use an actual
browser for the renderer fixtures. The main app still uses the native backend.
