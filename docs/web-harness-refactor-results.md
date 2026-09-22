# Shared app and web host refactor results

[한국어](web-harness-refactor-results.ko.md) · [Design](web-harness-design.md) · [Run and verify](browser-testing.md)

2026-09-22 · Unreleased changes based on `6470ba5` / v0.8.6. No commit, push or public release is implied by this report.

## Implemented boundary

| Owner | Responsibility |
| --- | --- |
| `packages/ui` / `@conn/ui` | One ConnApp, common components and EN/KO messages; per-app runes, requests, timers and disposal; terminal rendering, command ordering and output recovery |
| `frontends/tauri` | Thin native bootstrap, IPC, attention, clipboard/links and updater ports |
| `frontends/web` / `@conn/web` | Thin browser bootstrap, cookie authentication, WebSocket transport and host-managed update guidance |
| `crates/frontend::AppRuntime` | Shared owner commands, attachment epochs, per-session input/response/resize sequence, operation deduplication/outcome query and independent output delivery |
| `crates/web` / `conn-web` | Loopback process owner, authenticated owner attachment, static UI serving and server lifetime |
| `crates/core` | Existing PTY, policy, sharing and control authority; owner renderer checkpoint and shell prompt lifecycle repair |

The old `crates/browser-harness`, desktop browser-test mode, test transport and alternate app fixtures were removed. Pure UI tests moved with the UI. Production collaboration tests use a matching persistent `conn mcp` process, actual Rust PTYs and the built web UI; owner decisions use rendered controls.

Browser reload reconnects to existing sessions. Explicit takeover moves the single input/size renderer without granting agent permissions. Pending input is never blindly replayed. A checkpoint and sequenced frames restore the owner screen; output gaps block input until recovery. The checkpoint is not an agent observation API. Agent observation remains the authorized current terminal grid.

An uncertain ordered operation, a sequence failure or a failed initial/recovery checkpoint retires the attachment and presents explicit reconnection. Queued input is discarded. The UI asks the user to check the terminal and current request state after reconnecting before retrying. This also applies when the socket-close notification arrives before the pending input rejects. Reconnecting with an existing web cookie does not automatically request takeover.

Sharing still invalidates knowledge of private command context. An initial environment review can therefore be necessary after sharing; the first confirmed shell completion restores ordinary prompt knowledge. Connection admission, sharing, control approval and command approval remain distinct.

## Verification evidence

| Check | Observed result |
| --- | --- |
| Rust workspace, excluding native desktop | 272 passed, 1 existing ignored test. Actual PTY/socket tests were run outside the restrictive sandbox after EPERM there. A zsh integration case internally skips when zsh is absent. |
| Shared UI unit regressions | 65 passed: reconciliation, decisions, per-session ordering, output gaps, disconnect fencing, explicit reattachment after ordered/checkpoint failures, terminal input/output and resize |
| App/host lifetime | Actual simultaneous ConnApp mounts isolate language, same-ID requests, links and timers; unmount detaches listeners and fences late work. Actual bootstrap components and web transport verify first-visit authentication, EN/KO consistency, explicit takeover, recovery checkpoint failure, queued-input fencing and the socket-close/input-rejection race with controlled HTTP/WS and IPC fixtures. This does not execute native IPC, a PTY or MCP. |
| Production web + actual persistent MCP | 8 scenarios passed: private/selected sharing, sessionless preparation, admission/control, repeated safe commands, risk denial, long input, reload, explicit owner takeover, background tabs and fresh admission for a same-name new process. The added case injects `owner_busy` into the first input response over the actual production UI/Rust/MCP WebSocket path, then verifies explicit reattachment, the same PTY, no old-input replay and restored ordering for new input. The initial sandbox PTY EPERM was resolved by rerunning the isolated fixture outside that restriction. |
| Local input/rendering | 23 scenarios passed in Linux Chromium + Bash: delayed resize, density changes, fixed-window approval motion, long/Unicode input, line endings, settings, session allow and cursor editing |
| Actual SSH input/rendering | 24 scenarios passed with the existing disposable OpenSSH fixture, remote Bash and return to the original local shell. Only the fixture container was started/stopped; keys and authentication files remained byte-identical. |
| Parser and checkpoint | 85 grid/cursor comparisons and 984 actual xterm checkpoint comparisons, including byte splits, continuation, history/reflow and supported modes |
| Tooling | 79 Python tests, synchronized 0.8.6 manifests, generated Rust/TypeScript contract, shared UI import boundary and prospective source/link hygiene checks passed |
| Frontend builds | Native and web production frontend builds passed; 17 existing Svelte warnings remain |
| Site and demo | Site production build passed with 29 pages; demo TypeScript check passed. Site synchronization produced no tracked changes. |
| Development reload | Component HMR is disabled; source edits trigger a full page reload. Touching the entry source during manual verification triggered that reload and preserved the same terminal session and PTY. |
| Standalone package | Final local release-profile bundle: `release-artifacts/conn-web-0.8.6-linux-x64-cWEYiG`. Server + matching MCP + compiled UI, manifest and checksums were generated; packaged HTML/assets/API/version routes served without Node or Vite. The manifest records the uncommitted source state; this is not a public release. |

Tests print temporary evidence directories and can be rerun using the [validation guide](browser-testing.md). Temporary logs, screenshots, credentials and packaged binaries are excluded from source changes.

The final app/host fixture record is `/tmp/conn-app-lifetime-slRIee/result.json`. The eight production web/MCP cases are recorded in `/tmp/conn-web-mcp-IJThcw/evidence/results.json`, with no page errors. These are paths from this local validation run and are not distributed artifacts.

## Limits

This does not establish macOS WebKit/IME/native notification behavior or the user's actual SSH gateway route. Native Rust desktop compilation on this Linux host was blocked by missing GTK development libraries. Native frontend compilation is separate evidence.

The package is local and loopback-only. Remote authentication, TLS, multi-user service deployment and mirroring a separately running desktop instance are not implemented. Server termination ends its PTYs; a process restart does not recover them or resurrect permissions. Checkpoint fixtures do not prove every VT extension: advanced character sets, custom tab stops and OSC hyperlinks remain outside the tested terminal model.
