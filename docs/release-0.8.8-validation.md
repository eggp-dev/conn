# 0.8.8 release validation

[한국어](release-0.8.8-validation.ko.md) · [Investigation](terminal-input-candidates.md) · [Vim investigation](vim-input-investigation.md)

2026-09-23. Source validation is recorded here. The tagged GitHub Actions run and
downloaded release assets establish build, signing, notarization and distribution
results separately; this report does not predeclare them.

## Corrections

- Policy and review use the complete agent-owned command. Supported ASCII cursor
  edits retain text on both sides of the cursor. Unknown completion/history edits
  and ambiguous Unicode editing refuse submission rather than inferring a command
  from one screen row. Human input is still content-free bookkeeping.
- Denial/expiry cancels permission immediately. Only a confirmed idle local shell
  receives automatic Ctrl-C; subsequent input waits for a recovered boundary.
  Unknown foreground programs keep their visible input, expose `inputRetained`,
  and refuse agent appends until explicit recovery. Human intervention revokes
  agent control, cancels review and edits/submits the visible line directly.
- Write/flush errors return `input_outcome_unknown`. A failed or partially delivered
  Enter is never replayed or recorded as successfully approved. Resize publishes
  dimensions only after the local PTY accepts and reports the requested size.
  This does not prove a remote endpoint accepted an SSH window-change message.
- Obsolete output-queue callbacks cannot mark a new attachment ready. Input stays
  paused until the current checkpoint is applied. A prolonged wait exposes a
  restoration status and explicit screen recovery, with no input replay.

## Verification

Tests use isolated state directories and synthetic data. Browser suites mount the
production common UI against the actual Rust web host, PTYs and persistent MCP.
They do not replace the desktop UI or policy engine with a test implementation.

Real-shell assertions check
the exact reviewed command, policy outcome, disposable file contents, cancellation
remainder and next command, as well as core/xterm grid and cursor equality.

| Check | Result | Boundary |
| --- | --- | --- |
| Rust workspace | 288 passed, 1 existing ignored | Includes real Bash review identity and injected partial write/flush/resize failures |
| Common UI | 68 passed | Includes obsolete checkpoint callbacks and uncertain native input responses |
| Production web/MCP collaboration | 10 passed | Same owner contract, persistent connection and real PTY |
| Local input/rendering | 25 passed | Wrapped edits, ANSI/Korean prompts, review, denial, next command, fixed-window motion |
| Direct SSH input/rendering | 26 passed | Disposable loopback OpenSSH, remote Bash and UTF-8 locale |
| Real Vim input/save | 20 passed | Linux Chromium/WebKit × local/SSH × 5 scenarios: private/shared, live/reattached, long buffer resize |
| Core/xterm screen parity | 85 passed | Grid and cursor |
| Checkpoint restoration | 984 passed | Grid, history, cursor, styles, modes, continuation and resize |
| App lifetime | PASS | Common app with owner/transport fixtures; separate from native OS execution |
| Release tooling | 81 passed | Versions, assets, checksums, signing evidence and archive validation |
| Web and desktop frontend builds | PASS | Svelte checks and compiled bundles; existing Svelte/bundle warnings remain |
| Owner contract and UI boundaries | PASS | Same common components and runtime boundaries |

## Limits and failed experiments

The first disposable OpenSSH run inherited POSIX rather than UTF-8 locale. Long
Korean/emoji input failed the screen comparison. The fixture was corrected to
`C.UTF-8`; the complete input/rendering suite then passed. This is an environment
correction, not a product fix for arbitrary byte-oriented readline environments.
User shell configuration is never automatically rewritten or stripped of color.

The initial SSH Vim fixture lacked `vi`; after installing Vim in that disposable
container, edit/save checks passed. One ResizeObserver warning was observed in
each WebKit editor run and recorded separately; successful edits do not establish
that the warning is harmless in every native environment.

Linux Chromium/WebKit Vim coverage is not native macOS WebKit/Tauri acceptance.
The user's Mac Vim freeze, actual SSH gateway, physical IME and installed-app
upgrade/relaunch remain unverified in this session. ANSI prompt configuration and
remote dimensions may still contribute to the reported rendering symptoms; the
reproduced Conn defects are fixed without declaring all screenshots explained.
Output is not given synthetic newlines, terminal motion is retained, and the
previous unbounded PTY output queue is unchanged.
