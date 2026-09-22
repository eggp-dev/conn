# Shared runtime — follow-up after Mac validation

> Historical handoff baseline. The remaining implementation is now included in the 0.8.7 release candidate. See [release validation](release-0.8.7-validation.md) for current results and limits.

[한국어](shared-runtime-follow-up.ko.md) · [Mac report](shared-runtime-macos-validation-results.ko.md)
· [Control UX direction](control-experience.md) · [Product backlog](product-backlog.md)

2026-09-23 · version remains `0.8.6` · **branch development validation, unreleased**.

## Incoming changes

Fast-forwarded `codex/shared-runtime-macos-validation` from `7fa5723` to
`5df5e3667ebb6a820fc8ac5e6b544beb7b68fe52`. No other branch merge, main merge,
tag or deployment was performed. All seven existing uncommitted design documents
retained identical SHA-256 hashes across the update; follow-up links were added afterward.

| Commit | Change |
| --- | --- |
| `1d30c87` | Drain PTY output independently of the input writer's session lock; ordered FIFO consumer and two long-input regressions |
| `5df5e36` | Native macOS/web validation, test-script improvements and twelve evidence images |

All twelve image hashes match the Mac evidence manifest. This verifies file
integrity, not a new Mac validation run on this Linux host.

## Linux test portability correction

The first unmodified `cargo test --locked` failed the two new regression tests.
Long input, its complete output and the following command all completed. Linux
Bash/readline inserted a bracketed-paste mode escape before command output, so the
assertion requiring adjacent raw `CRLF + NEXT_OUTPUT_OK + CRLF` bytes timed out.

Updated the follow-up assertion in `crates/core/tests/shell_integration.rs` to
require an exact `NEXT_OUTPUT_OK` row in the core's current terminal grid. The
five-second write deadline and full 3KB output check remain. Command echo alone
cannot satisfy the new assertion. This is an uncommitted local test correction;
no additional runtime change was made.

## Linux results from this run

Linux, Rust 1.95.0, Node 26.8.1; dependencies reinstalled with `npm ci`.
Results use `5df5e36` plus the test correction above.

| Check | Result | Scope |
| --- | --- | --- |
| `cargo test --locked` | **274 passed, 1 ignored** | Default workspace members, excluding native Tauri; unavailable zsh cases internally skipped |
| `cargo build --locked -p conn-web -p conn` | **PASS** | Actual Rust web host and MCP CLI |
| `npm run build:web` | **PASS** | Zero Svelte errors; 17 existing warnings and bundle-size warning remain |
| `npm run test:ui` | **65 passed** | Common UI state and behavior |
| `npm run test:collaboration` | **8 passed** | Production common UI, real Rust and persistent MCP; no page errors |
| `npm run test:input -w @conn/collaboration-tests` | **23 passed** | Local Bash input/rendering; no SSH fixture configured in this run |
| `npm run test:screen -w @conn/ui` | **85 passed** | Core/xterm current grid and cursor |
| `npm run test:checkpoint -w @conn/ui` | **984 passed** | Checkpoint restoration, continuation and resize |
| `npm run test:app-lifetime -w @conn/ui` | **PASS** | Actual common UI with controlled host/IPC fixtures, not native OS, real PTY or MCP |
| Generated owner contract / common UI import boundary | **PASS / PASS** | Rust/TS contract and thin app entry points |

The two existing low-severity dependency findings were not force-upgraded. Native
Linux UI, physical IME, user servers and system notifications were not rerun.
The Mac report's 24 loopback SSH cases remain Mac evidence and are not added to Linux counts.

## Completed work and remaining work

Completed: common UI/runtime refactor, the Mac long-input deadlock fix, the named
Mac native paths and the Linux regression checks above. These branch changes are
not claimed to be in the installed public v0.8.6. Functional approval-stage PASS
does not establish that people can distinguish and understand those stages.

| Remaining work | Next acceptance boundary |
| --- | --- |
| **P1: human input state** | [#65](https://github.com/eggp-dev/conn/issues/65): fix both false blocks at empty prompts and unsafe appending to remaining human input; last-byte heuristics remain |
| **P1: coherent approval state** | Deliver reason, target, scope and eligible actions together; delayed status must not change the same request's buttons or rationale; retain current backend checks |
| **P1: accurate review reasons** | Separate command risk from uncertain shell context; reproduce compound-command SSH differences rather than attributing them to privacy or platform UI |
| **Agreed control UX** | Morph the top-right Conn badge into and out of control requests; keep command review below; implement with common components/runes and validate repetition, other tabs, keyboard and reduced motion |
| **Runtime load before release** | Measure unbounded queue memory, latency and final output ordering under sustained output and slow consumption; a blocking bounded queue can reintroduce the deadlock |
| **Linux installed-app startup** | The renderer environment-variable workaround confirmed in this conversation is local; the Mac commits contain neither an installer fix nor validation on other devices |
| **Environment and release acceptance** | User SSH gateway, physical IME, native notifications, native IPC fault injection, signed/notarized release and updater artifacts need separate checks |

Sequence: **input and approval-state correctness → agreed control UX → acceptance
through the same real paths**. Queue load testing is an independent pre-release
item. New visual polish must not conceal stale state or ask again throughout
ordinary work already within a grant.

Local changes comprise the product invariant, control UX design, this work summary
and the Linux test correction. Morphing UI implementation, commit/push, main merge,
tags and deployment were not performed during this handoff.
