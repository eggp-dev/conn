# v0.9.0 validation

Implementation base: `744fa22f4396a3d4e762a29d6156ba4e12fcdb8b`, independently checked against upstream main before work. Existing checkouts were clean and were left untouched. All runtime tests below use disposable sessions; no existing user terminal, installed app or actual remote work environment was changed.

## Local evidence

- `cargo test --locked --offline`: 315 passed, one existing ignored test. Includes 18 new integration regressions and five monotonic/transport/queue unit regressions.
- New coverage: default per-command review; explicit POSIX-only ordinary-command reduction; risk/deny/unsupported syntax; unknown dialects; distinct same-name connections; no retroactive approval or category promotion; expiry/stop/profile/policy/sharing queue cancellation; retained-input recovery; natural lease expiry versus human takeover; nonrenewing reads/duplicate clicks; owner-only mutation and connection-scoped public state.
- `npm run test:ui`: all nine test files passed.
- `npm run build -w conn-desktop`, `npm run build:web`, `npm run build -w conn-site`, `npm run check -w conn-demo`: passed. Svelte check: zero errors; existing warnings remain.
- `npm run test:screen -w @conn/ui`: 85 grid/cursor comparisons passed.
- `npm run test:checkpoint -w @conn/ui`: 984/984 checkpoint/xterm cases passed.
- `npm run test:app-lifetime -w @conn/ui`: passed app/language/request/host-link/disposal isolation.
- Retained-input/header/connection production regressions: all 15 cases passed twice after keeping inactive environment controls inside a pending-review flow. This prevents a transient shell-status update from changing idle terminal geometry.
- Production browser/MCP: ten existing collaboration scenarios passed. Five new environment-controls scenarios passed using actual local owner UI, server, PTY and persistent MCP: acknowledgement/close/re-entry, pending-card nonapproval, countdown/reload, stop/disconnect/same-name reconnect, and unknown-dialect review. The unknown-dialect fixture deliberately keeps per-command review; known explicit POSIX behavior is covered in core tests.
- `node scripts/check-ui-boundaries.mjs`, `node scripts/generate-owner-contract.mjs --check`: passed.
- `python3 scripts/release.py check --tag v0.9.0`, repository hygiene/relative links, and 81 release-tooling tests: passed.
- `cargo clippy --locked --offline --all-targets`: passed with existing warnings. Strict `-D warnings` fails on existing `backend.rs`, `analysis.rs`, `session.rs` and `human_input.rs` lint findings. `cargo fmt --all --check` reports existing repository formatting drift; the original base `session.rs` independently fails rustfmt. New Rust files were formatted. No repository-wide formatting churn was introduced.
- Local native debug build was attempted and stopped at missing `gdk-3.0.pc`/GTK development libraries. No system dependency installation was performed. CI native builds and release packaging remain distinct evidence.

## Limits

Linux Chromium evidence does not establish native macOS/Windows interaction, Wayland, installed-app upgrades or all nested remote transitions. Unknown final host changes remain undetectable. No live user grant was enabled. Audit additions contain only decision identifiers, duration/revision and end reason.

Release workflow, signing, checksums, exact-release package runtime and website publication are verified after the tagged pipeline; final status is recorded in the delivery report rather than inferred from local builds.

## Tagged-run corrections

The first tagged run `40354039491` failed and created no public release. Windows exposed that mtime-only policy reload could miss a fast policy edit before queued Enter. Policy reload now compares source bytes, including equal-length edits with unchanged timestamps, and refuses environment grants while the source is invalid or unreadable. The queue regression preserves the timestamp deliberately. Last-good policy fallback otherwise remains intact.

The header regression failed only on extra trailing empty viewport rows during dock layout. The assertion now checks identical non-padding contents, cursor and column count, retaining both rejected-input checks. This does not waive input or cursor changes.

macOS received notarization Accepted and completed stapling, but validation exited 68. An Apple service connection failure is a possible cause, not a confirmed diagnosis from the sanitized log. Ticket validation retries exit 68 at most twice; other failures and exhausted retries still block packaging. No signature, ticket, Gatekeeper or contained-DMG check is removed. Native success must still be established by the corrected release pipeline.

Local correction checks: portable `cargo test` passed; 82 release-tool tests passed; all 15 header/input browser regressions passed. A full workspace native attempt remains blocked by missing local GTK development libraries. The original unpublished v0.9.0 tag is retained pending an explicit retag decision; no public artifacts were replaced.
