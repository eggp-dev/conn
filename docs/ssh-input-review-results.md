# SSH input and approval fixes

[한국어](ssh-input-review-results.ko.md) · [Reproduce the tests](browser-testing.md#ssh-input-cursor-and-approval-regression)

2026-09-22 · v0.8.5 patch based on v0.8.4 (`06143d0`).

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
