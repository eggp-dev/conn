# SSH input and approval fixes

[한국어](ssh-input-review-results.ko.md) · [Reproduce the tests](browser-testing.md#ssh-input-cursor-and-approval-regression)

2026-09-22 · v0.8.5 patch based on v0.8.4 (`06143d0`).

## Terminal dimensions after v0.8.5 — unreleased

The user confirmed the issue after updating and restarting, then clarified that both axes appear
constrained when notification/approval bars animate, while the application window stays fixed.
A fixed window still changes the terminal's available height as the approval dock opens and closes.
The previous settings-modal fix does not close this report. The actual intermediate SSH gateway
has not been tested.

### Reproduced defects and changes

- The browser server serializes commands; native Tauri dispatch uses independent blocking workers.
  Delaying the first resize by 350 ms left the PTY at 83 columns after the UI returned to 104.
  A separate probe produced 83 versus 137 columns.
- With the window fixed at 860×640, delaying the approval dock's shrink by 500 ms left the PTY at
  27 rows after the UI returned to 38, with cursor rows 26 versus 37 (zero-based). These are controlled
  delay injections into the production UI / real Rust / real SSH route, not captures of native timing.
- Serializing resize requests alone was insufficient. A longer pending command and quick denial
  reproduced a second ordering defect: both grids were 38 rows, but the visible cursor was at row 23
  while the core cursor was at 37. Shell redraw bytes and renderer resizes were applied in different orders.
- The frontend now permits one resize request per terminal and retains the newest pending dimensions.
  The backend includes the applied size in the owner output stream and emits an empty frame on resize,
  under the same session lock as output. The renderer applies these sizes in stream order and waits for
  earlier xterm writes to finish before applying the next frame. Pane measurement requests a size;
  it no longer independently resizes the renderer ahead of the backend.
- Dock motion is retained. No replacement shell, remote hook, hidden input, access condition or
  permission is introduced.

### Verification and remaining scope

- Both delayed-width and fixed-window approval regressions pass after the final change, including
  grid/cursor equality and full available pane dimensions. The normal fixed-window case records
  actual terminal motion; the delayed case closes the dock before the late resize arrives.
- The complete Linux Chromium / real SSH harness passes: 240–7,000-character input, Unicode,
  settings, long approval, session allowance, human cursor editing/takeover and return to the local shell.
  Pixel densities 1, 1.25, 1.5 and 2 preserve pane coverage. Density emulation is not native multi-monitor acceptance.
- Linux WebKit 26.6 passes 18 fixed-window approval open/deny cycles with the cursor initially at the
  bottom, including actual terminal animation, short and wrapped commands and rapid dismissal.
  The final run recorded one ResizeObserver warning, also seen before the change, without a persistent
  grid/cursor mismatch. This is not an error-free-console or native macOS acceptance claim.
- The 12-stage collaboration context harness also passes, including background tabs, preparation,
  private-terminal creation and sharing transitions.
- 259 default Rust tests pass (one optional SSH test excluded), as do 55 frontend tests and the frontend
  build. The Linux desktop bundle was not built: the host lacks the required Pango development package.
- Earlier direct OpenSSH and nested interactive SSH probes each passed 18 input/approval cases at
  three widths. They do not establish correctness of the user's gateway implementation.

This follow-up is unreleased and is not in published v0.8.5. The exact photographed corruption still needs
acceptance in the user's installed macOS app and SSH environment. Resize/output tests do not establish
native keyboard-input ordering or physical-keyboard IME behavior. A separate `/bin/sh` canonical-input
experiment left old wrapped text after approval denial; it remains open and is not identified as the photo's cause.

### Native macOS verification of PR #64

On 2026-09-22, a separate checkout of `codex/terminal-resize-order` was verified at
`e97c2048e320d6c09fa1d556bd343c52deab0b58`.
[CI for that commit](https://github.com/eggp-dev/conn/actions/runs/35705822838) passed: Rust on
Linux/macOS/Windows, and desktop CI on Linux. The Mac application checks below are separate evidence.
The [aggregate evidence](validation/macos-resize-order-results.json) contains no personal paths or raw output.

**Environment and process identity:** macOS 27.0 (`26A428`), arm64, Node.js 24.13.0, Rust 1.95.0.
Root `npm ci`, 259 Rust tests (one optional SSH test excluded), 55 frontend tests and the frontend build passed.
An ad-hoc signed native bundle was built from `frontends/tauri` with
`npm run tauri -- build --debug --bundles app --config <temporary override>`.
The app name was `Conn Resize Validation`, identifier `dev.eggp.conn.resizevalidation`, with separate
`CONN_CONFIG_DIR` and `CONN_SOCKET`. The executable was inside the new checkout at
`frontends/tauri/src-tauri/target/debug/bundle/macos/Conn Resize Validation.app/Contents/MacOS/conn-desktop`;
PID `34233` owned the test socket. Its SHA-256 was
`b199fa89ef6bf141b4c30e989d620d1e51fae5312c712b57d4e4ea813621c139`.
The displayed 0.8.5 version was not used to identify the candidate. Tests used the actual
`tauri://localhost` WebKit application and Tauri IPC.

**SSH and measurement:** SSH was entered through the existing Conn PTY into a separate loopback OpenSSH
server with temporary keys and a pinned host key. The actual remote shell was
`/bin/bash --noprofile --norc -i`, Bash `3.2.57(1)-release`, with `TERM=xterm-256color` and
`LC_ALL=en_US.UTF-8`. Before/after `stty size` returned `41 137`. This exercises Bash line editing;
it does not resolve the separate canonical `/bin/sh` behavior.
Native captures and accessibility trees were inspected. A temporary WebKit-inspector probe recorded
the actual visible DOM rows, cursor geometry, pane dimensions and motion locally for comparison with
agent snapshots. Twelve rapid denials used a bounded temporary UI probe that clicked the actual DOM
deny button 80–82 ms after each card appeared. No product transport, resize or output implementation
was replaced or delayed. The inspector was closed during measurements, and both probes were removed afterward.

| Native check | Result |
| --- | --- |
| Fixed-window approval dock | Kept the outer window at 1120×720 and WebKit content at 1120×688. Filled the screen to put the cursor at the bottom, then tested short and 478–1,678-character commands. Eighteen approval cycles included approval, denial, human interruption and twelve rapid denials. |
| Actual motion and restoration | The rapid cycles retained a fixed window and recorded 302 frames of actual terminal movement. Columns stayed at 137; card height reduced 41 rows to 30, 26 or 20 and then restored them. The human hand-back bar correctly reserved space for a 38-row terminal. The final terminal filled its available pane. |
| Visible/core parity | All 38 checkpoints matched full visible rows, cursor and row count across approvals, editing, output and shell return. These are native DOM comparisons, separate from browser-harness or parser-test counts. |
| Human input | Used left/right movement, insertion, Backspace and Enter in a 1,221-character ASCII/Korean/emoji command, verifying `수정END>` output. The first arrow key reclaimed control with `human_input`; the next agent write returned `not_controller`. |
| Command policy | Ordinary `printf` and `touch` executed automatically. **Allow this session** removed one disposable file; the second deletion under the same rule ran automatically. `recursive delete` and `privilege escalation` still required approval. A harmless `printf` containing dangerous text as an argument confirmed the deny rule. Actual deletions affected only two synthetic files. |
| Human input during approval | Ctrl-C resolved the privileged request as `denied (human_input)` without execution and reclaimed control. This is not the protocol's cancelled state. |
| Same shells | Remote Bash PID `34307` remained unchanged; SSH exit returned to the original local Bash PID `34253`. The screen identity also remained unchanged. |
| Output endings | Six short/wrapped JSON comparisons covered no newline, LF and CRLF. Three additional comparisons used an exact 411-character = 137-column × 3 boundary: even without a newline, the prompt then wraps to the next row. No artificial newline was inserted. |

**Remaining scope:** the twelve rapid denials recorded 24
`ResizeObserver loop completed with undelivered notifications` warnings. The probe recorded 42 overall;
the inspector displayed 46 including earlier warnings. No persistent grid/cursor disagreement appeared
at the checked post-motion points, but the warning itself remains unresolved.
Physical-keyboard Korean IME, the user's actual remote account/gateway and the separate canonical
`/bin/sh` behavior remain unverified or unresolved. No running user Conn/SSH session was available,
and no real-account authentication was attempted. No additional input/resize failure requiring a product
change was reproduced, so product code was unchanged. Personal paths, keys, raw socket/DOM/audit logs
and captures were not committed. This does not verify signed/notarized installed-app upgrades or a public
release. Version remains 0.8.5 and the change remains Unreleased.

## Additional report: JSON followed immediately by the prompt

A further photo shows the closing JSON braces immediately followed by a colored `[DEV]` prompt.
This matches output without a trailing newline, but the response bytes from that particular command
have not been captured, so the photograph alone does not establish the cause.

The current production-UI/Rust/real-SSH harness passed six comparisons: short and wrapped synthetic
JSON, each with no trailing newline, LF or CRLF. Without a newline the prompt followed the JSON on
the same line; with LF or CRLF it began on the next line. Rendered rows and cursor matched the core
in all six cases. Independent ordinary Bash and `/bin/sh` PTYs, without Conn, showed the same three
behaviors. These fixtures reproduce the output bytes, not the user's endpoint or installed Mac app.

A regression now preserves both cases. Conn must not insert newlines into arbitrary terminal output:
that would alter real shell/TUI cursor behavior. When a command returns a body without a newline,
appending `; printf '\n'` explicitly separates the next prompt. This report is distinct from the
reproduced resize/output ordering defects above unless contrary byte-level evidence appears.

## Report and reproduced causes

The user reported macOS with an SSH remote shell: long command input displaced the cursor/input position, **Allow this session** appeared unresponsive, and automatic execution with risk-based approval seemed unavailable. The requested behavior is ordinary commands executing automatically, with approval for risky commands.

- **Approval:** SSH forced every command into review, disabling session allowances even in Autopilot. The UI displayed a disabled button without explaining why.
- **Input/cursor:** the modal settings sheet still activated an obsolete 600-pixel side-panel offset. Opening/closing settings repeatedly resized the real PTY during the width animation. Long input reproduced corruption and disagreement between rendered rows and the core grid in real SSH.
- **Long approval:** a roughly 5,000-character command pushed action buttons below the visible dock. The common card now scrolls its content while retaining its actions.

## Resulting behavior

POSIX SSH uses the existing command policy without host cwd, home or filesystem target inspection. Ordinary commands execute in Autopilot, confirm rules request approval, and deny rules remain blocked. Session allowance applies to the matching confirm label and is published to the UI immediately. Co-pilot, connection admission, sharing and control grants retain their separate roles.

SSH profiles, direct SSH launches and a single SSH command identified by Bash/Zsh integration use this policy. The matching outer-shell exit restores local analysis. Lost SSH lifecycle requires review; other remote backends, non-POSIX shells and unclassified programs retain individual review, with an explanation in the UI.

Opening modal settings no longer resizes the underlying PTY. Actual window and collaboration-dock changes still resize it. No replacement shell, remote hook or separate execution channel is introduced.

## Verification and limits

- 258 default Rust tests passed; the existing optional SSH test is excluded from that count.
- The optional real OpenSSH authentication/sharing test passed separately, including ordinary remote execution and a privileged command held for review without execution. Private credentials stayed absent from shared data.
- 48 frontend tests, the desktop frontend build, 85 core/xterm grid-and-cursor comparisons, site build and repository/document checks passed.
- The production UI with the real Rust backend and loopback SSH exercised 240–7,000-character input, Korean/emoji input, repeated settings open/close, long-command approval at a narrow viewport, session allowance scope, human cursor movement/insertion, takeover and return to the original local shell.
- The existing collaboration-context harness covers the 12 tab, notice, sharing, reconnect and preparation stages.

The browser tests above use Linux Chromium and remote Bash with UTF-8 locale. The native macOS run below is separate evidence. Neither establishes that every cause of the user's symptom is resolved. Release artifact validation is reported separately.

## Native macOS follow-up

On 2026-09-22, `ed57aac936bec98e26f0a69ac06241cef36c5834` on `codex/terminal-input-review` was checked on an Apple Silicon Mac. The checkout was newly cloned with no existing local changes. [CI for that commit](https://github.com/eggp-dev/conn/actions/runs/35691004269) passed the Linux/macOS/Windows Rust and desktop jobs, docs/frontend, collaboration browser flows and required checks. PR #63 remained a draft.

### Environment and evidence

- macOS 27.0 (`26A428`), arm64, Node.js 24.13.0, Rust 1.95.0; root `npm ci` completed.
- `npm run tauri dev` from `frontends/tauri` launched the native application. The UI automation tool could not select the unbundled development process, so final interaction used an ad-hoc signed debug bundle built from the same source with `npm run tauri build -- --debug --bundles app --config <temporary override>`. A temporary wrapper around the development executable was not the final candidate.
- A distinct test app name/identifier, `CONN_CONFIG_DIR` and `CONN_SOCKET` separated the run. The executable path and socket-owning PID were checked. Interaction used the real `tauri://localhost` WebKit UI and Tauri IPC, not the browser harness.
- A separate loopback OpenSSH server used temporary keys and a pinned host key, real `/bin/bash --noprofile --norc -i`, `en_US.UTF-8` and `xterm-256color`. SSH was entered through Conn's existing local PTY. Actual deletions affected only two disposable files created for this run.
- On the Mac: 258 Rust tests passed, with the existing optional SSH test excluded; 48 frontend tests, frontend and native debug-bundle builds, and 85 core/xterm comparisons passed. The last count describes automated parser checks, not native DOM comparisons.

### Native interaction results

| Check | Result |
| --- | --- |
| Ordinary automatic execution | Autopilot was selected in the upper-right control center. Connection admission and control approval were handled separately. Remote `printf` and fixture `touch` returned `executed` without command approval. |
| Long ASCII input and settings | With 1,521- and 7,029-character input, opening settings preserved PTY size, screen, cursor and output sequence. Repeated open/close preserved the input; the 7,000-character payload printed and returned to the prompt. The same agent could execute while settings was open. |
| Korean/emoji and actual window resizing | Pasted 65 repetitions of `한글/경로/🙂/`. Opened/closed settings and resized 1120×720 → 757×433 → original size. Compared the visible cursor with core snapshots as the terminal changed from 137 to 90 columns. |
| Human editing and takeover | Used left/right movement, insertion, Backspace and Enter in long ASCII/Unicode commands; verified `EDITEND>` and `수정END>` output. The first human arrow key revoked the agent's lease with `human_input`; its next write returned `not_controller`. |
| Long approval card | A 5,109-character deletion command stayed pending under `delete files`. At 757×433, content scrolled inside the card and approve/deny/session-allow buttons remained visible. Clicking **Allow this session** returned `granted` and removed the disposable file. |
| Allowance scope and revocation | A second disposable file deletion executed without approval. `recursive delete` and `privilege escalation` still requested approval and were denied through the UI. Settings showed only `delete files` as allowed. Revoking it restored approval for the same rule; individual approval also worked in Korean. |
| Deny rules | The nonexecuting policy tester denied root deletion. The actual agent Enter path was tested with harmless `printf '%s\n' 'rm -rf / '`, which contains the dangerous text only as an argument: it returned `delete root`/`denied` and appeared as policy-blocked in the timeline. No actual root-deletion command was submitted for execution. |
| Human input during approval | Ctrl-C resolved the pending command as `denied (human_input)` without execution and reclaimed control. A subsequent human command confirmed the same remote Bash PID. |
| Original shell return | Exiting SSH returned to the original local Bash PID. The session screen identity also remained unchanged. |

Native accessibility trees and window captures were inspected; selected screen/cursor positions were compared with agent snapshots. This was not an automated comparison of every native rendered row. Socket responses, snapshots, audit records and build logs were kept in the Mac's temporary validation directory. Personal paths, keys, raw logs and captures were not added to the repository. No additional product code change was made.

### Remaining checks and observations

- **Physical-keyboard Korean IME composition remains unverified.** Automated keys produced composed `한글` in the textarea but initial consonants in the PTY. A temporary WebKit event trace contained no `compositionstart/update/end`; it recorded `insertText`/`insertReplacementText` with `isComposing=false` and key events. This delivery cannot establish success or failure for physical-keyboard composition, so no product workaround was added. Successful Unicode paste/editing is not an IME acceptance claim. The temporary trace was removed.
- WebKit's inspector had accumulated `ResizeObserver loop completed with undelivered notifications` messages after window/dock changes. Checked screen/cursor/editing steps did not establish persistent disagreement; this is not an error-free-console claim.
- The user's actual SSH server/shell setup, non-UTF-8 line editors, other macOS versions, and signed/notarized installed-app upgrades still need separate acceptance. At the time of this native run, the checkout was version 0.8.4 with changes under Unreleased; this run itself was not a merge, tag or release.
