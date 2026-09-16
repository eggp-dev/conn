# Contributing to Conn

English · [한국어](CONTRIBUTING.ko.md) · [README](README.md)

Conn is an early project. A clear reproduction, a careful platform test, or a better sentence can be as useful as a new feature. Issues and pull requests in **English or Korean** are welcome.

## Pick a useful first contribution

- Test a real macOS or Windows session: launch, type, resize, request control, deny, take over, and close the tab.
- Report a shell or backend incompatibility with the smallest command that reproduces it.
- Improve keyboard navigation, text wrapping, focus, screen-reader labels, or reduced-motion behavior.
- Keep English and Korean interface strings and documentation aligned.
- Add a regression test for a control, policy, request-recording, or session-lifetime bug.

For a larger feature or a new backend, open an issue describing the user's workflow before changing the architecture. Search existing issues first. For security problems, use [SECURITY.md](SECURITY.md) instead of posting sensitive details publicly.

## Set up the project

Install rustup, **Node.js 24**, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for native desktop work. Rust is pinned by `rust-toolchain.toml`. CI also uses Python 3.12 for release tooling.

```sh
git clone https://github.com/eggplantiny/conn.git
cd conn
```

If you are contributing through a fork, clone your fork and create a focused branch. Keep local profiles, audit logs, shell history, credentials, build output, and test files out of commits.

### Run the desktop

```sh
cd frontends/tauri
npm ci
npm run tauri dev
```

The wrapper builds the native CLI sidecar before starting Tauri. A native desktop build needs the platform libraries even when the Rust core builds successfully without them.

### Run the shared UI in a browser

From `frontends/tauri`:

```sh
npm ci
npm run test:browser
```

Open `http://127.0.0.1:1421/`. This uses the **same App and Rust frontend harness** through a local WebSocket adapter. It is a development aid, not a separate web product or a simulated shell. Shell commands run as your local user. The launcher prints a separate settings directory and agent endpoint.

Reloading or disconnecting the browser ends its live sessions. Test with disposable files and deliberate reloads. See [browser testing](docs/browser-testing.md) for lifecycle and endpoint details.

## Know where the change belongs

| Path | Responsibility |
|---|---|
| `crates/core` | PTY, session control, policy, audit, IPC, shell/backend profiles |
| `crates/cli` | Terminal frontend, headless serve command, MCP adapter, human/agent CLI |
| `crates/frontend` | Shared native frontend harness and command dispatch |
| `crates/browser-harness` | Local browser-test transport to the shared harness |
| `frontends/tauri/src` | Shared Svelte UI, terminal renderer, timeline, settings, i18n |
| `frontends/tauri/src-tauri` | Tauri adapter, native configuration, packaging |
| `plugin` | Agent procedure and MCP server definition |
| `docs` | User guides, protocol, architecture, validation, releases |

Read the relevant [architecture](docs/architecture.md), [protocol](docs/protocol.md), [policy](docs/policy.md), or [backend](docs/backends.md) guide before changing its contract.

## Preserve the collaboration contract

- Human input takes priority over agent writes. Control approval does not automatically approve every command.
- Keep request metadata distinct from execution evidence. A denied request may have a planned command; it must never appear as executed.
- Preserve the actual method and arguments when recording an original request. Do not infer missing payloads from a reason or from later commands.
- A policy block, a user's denial, expiry, and cancellation are different outcomes. Keep that distinction through events, persistence, and UI.
- Profiles belong in settings. New tabs use the saved default; existing sessions keep their current configuration.
- Keep one shared UI and native engine. Browser testing should replace transport, not fork behavior.
- Remote targets must not resolve file paths against the host. Backend changes need argument construction, process lifetime, and policy-boundary tests.
- Use existing i18n keys for interface text and add both English and Korean strings for new keys. Avoid hard-coded user-facing text in components.
- Keep state readable without animation or color alone. Verify keyboard focus and `prefers-reduced-motion` for motion changes.

## Validate your change

From the repository root:

```sh
cargo test --workspace --locked
```

From `frontends/tauri`:

```sh
npm ci
npm test
npm run build
```

For native adapter or packaging changes, also run this from `frontends/tauri` on the platform being changed:

```sh
npm run tauri build -- --debug --no-bundle
```

For a UI change, verify the real flow in the native app or the shared browser harness. Capture a screenshot when layout or motion is relevant. Cover approval, denial, and cancellation when touching those controls. Add meaningful regression tests for changed behavior rather than tests that repeat the implementation.

Use the [platform checklist](docs/platform-support.md#release-verification-checklist) for installer/runtime reports. Include the exact asset and checksum, OS version, CPU architecture, shell and Linux display session where applicable. Preview targets are Ubuntu 24.04/26.04 x64, Windows x64, and Apple Silicon Macs; Windows signing credentials are not required to contribute or test.

State **which platform you tested** and **what you observed**. Compilation, automated tests, installer creation, and native interaction are separate evidence. Do not call a platform supported based only on a cross-compile check.

Before submitting:

```sh
git diff --check
git status --short
```

Inspect every staged file, especially logs, screenshots, endpoint paths, and local configuration. Redact sensitive data before attaching diagnostics.

## Write a reviewable pull request

Describe the concrete problem, the resulting behavior, and how it was verified. A short before/after scenario helps:

> A denied control request lost its command in the timeline after reload. This change saves the original request with its resolution and restores it in the detail view. Tested a denial, reload, and a legacy record without a payload.

Include relevant commands and their results, screenshots for UI changes, and any platform checks still pending. Update both language versions when you change user-facing instructions. Avoid unrelated formatting or generated-file churn.

There is no separate contributor license agreement. Contributions are made under the repository's [MIT license](LICENSE).

## Releases

Contributors do not need signing credentials or release permissions. CI validates changes; release preparation and public publication follow the maintainer steps in [the release guide](docs/releasing.md). Do not add secrets, publish packages, or change production domains as part of a normal pull request.
