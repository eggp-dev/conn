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
