# v0.9.0 validation

Implementation base: `744fa22f4396a3d4e762a29d6156ba4e12fcdb8b`, independently checked against upstream main before work. Existing checkouts were clean and were left untouched. All runtime tests below use disposable sessions; no existing user terminal, installed app or actual remote work environment was changed.

## Local evidence

- `cargo test --locked --offline`: 310 passed, one existing ignored test. Includes 16 new integration regressions and two monotonic/transport unit regressions.
- New coverage: default per-command review; explicit POSIX-only ordinary-command reduction; risk/deny/unsupported syntax; unknown dialects; distinct same-name connections; no retroactive approval or category promotion; expiry/stop/profile/policy/sharing queue cancellation; retained-input recovery; natural lease expiry versus human takeover; nonrenewing reads/duplicate clicks; owner-only mutation and connection-scoped public state.
- `npm run test:ui`: all nine test files passed.
- `npm run build -w conn-desktop`, `npm run build:web`, `npm run build -w conn-site`, `npm run check -w conn-demo`: passed. Svelte check: zero errors; existing warnings remain.
- `npm run test:screen -w @conn/ui`: 85 grid/cursor comparisons passed.
- `npm run test:checkpoint -w @conn/ui`: 984/984 checkpoint/xterm cases passed.
- `npm run test:app-lifetime -w @conn/ui`: passed app/language/request/host-link/disposal isolation.
- Production browser/MCP: ten existing collaboration scenarios passed. Five new environment-controls scenarios passed using actual local owner UI, server, PTY and persistent MCP: acknowledgement/close/re-entry, pending-card nonapproval, countdown/reload, stop/disconnect/same-name reconnect, and unknown-dialect review. The unknown-dialect fixture deliberately keeps per-command review; known explicit POSIX behavior is covered in core tests.
- `node scripts/check-ui-boundaries.mjs`, `node scripts/generate-owner-contract.mjs --check`: passed.
- `python3 scripts/release.py check --tag v0.9.0`, repository hygiene/relative links, and 81 release-tooling tests: passed.
- `cargo clippy --locked --offline --all-targets`: passed with existing warnings. Strict `-D warnings` fails on existing `backend.rs`, `analysis.rs`, `session.rs` and `human_input.rs` lint findings. `cargo fmt --all --check` reports existing repository formatting drift; the original base `session.rs` independently fails rustfmt. New Rust files were formatted. No repository-wide formatting churn was introduced.
- Local native debug build was attempted and stopped at missing `gdk-3.0.pc`/GTK development libraries. No system dependency installation was performed. CI native builds and release packaging remain distinct evidence.

## Limits

Linux Chromium evidence does not establish native macOS/Windows interaction, Wayland, installed-app upgrades or all nested remote transitions. Unknown final host changes remain undetectable. No live user grant was enabled. Audit additions contain only decision identifiers, duration/revision and end reason.

Release workflow, signing, checksums, exact-release package runtime and website publication are verified after the tagged pipeline; final status is recorded in the delivery report rather than inferred from local builds.
