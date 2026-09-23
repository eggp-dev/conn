# Terminal input investigation and fix candidates

[한국어](terminal-input-candidates.ko.md)

2026-09-23 · Source baseline `05b6214` / 0.8.7. **Investigation only: no product fix, native macOS acceptance, or deployment is claimed.** Existing Vim investigation changes were preserved.

## Findings and evidence boundaries

The user photos do not establish a single cause. Missing trailing newlines, input redraw, command identity at approval, and cancellation leftovers are separate concerns. Some Conn defects reproduce with correctly marked colored prompts.

Experiments used Linux Chromium, the built common web UI, real Rust web host/Bash PTY, a persistent MCP process, and real owner approval controls. Only synthetic data and disposable files were used. No user Mac, remote server, or authentication input was accessed. Cursor navigation below uses MCP LEFT unless stated otherwise.

| Finding | Evidence | Consequence |
| --- | --- | --- |
| An unchanged long printf/redirection command needs overwrite approval; after one LEFT it is assessed from a suffix and executes automatically | Real UI/PTY; actual output file verified | Policy command and physical submitted command disagree |
| Denial with a middle-of-line cursor leaves `probe.out`; the next accepted input becomes `printf NEXTprobe.out` | Real UI/PTY | Cancelled does not mean physically empty |
| With `[DEV]한글$`, LEFT then submit turns `printf` into `intf` in review text | Real UI/PTY | Cell columns used as Unicode character indices |
| Unmarked ANSI prompt sequences misposition the cursor after Ctrl-A/E | Separate real UI/PTY comparison | Plausible field mechanism; user shell/PS1 unverified |
| LEFT 30 times mispositions the cursor with PTY 80 columns/core+UI 104; the all-104 control is correct | Deliberately induced mismatch; real UI/PTY comparison | Geometry mismatch causes cursor errors even with a properly marked colored prompt; field occurrence unverified |
| Clearing the output queue does not invalidate an in-flight checkpoint completion callback | Delayed-completion reproduction against the actual common queue | Obsolete completion can affect readiness; not proof of the Mac Vim cause |
| PTY resize and normal write/flush errors are discarded | Source inspection | Potential false success; real failure injection and field occurrence unverified |

## A. Preserve complete command identity and provenance — P0

`crates/core/src/input.rs:156` resolves uncertain input from the cursor's single physical row. For wrapped commands that row is only a fragment. `strip_prompt` also uses cell-based `prompt_col` to slice `Vec<char>`, breaking wide-character prompts. `session.rs:1766` assesses this result, but Enter submits the entire physical shell buffer.

The reproduction used `printf '%s\n' > probe.out 'WRAP_BEGIN_…_END'`, with 200 synthetic padding characters. Without editing: approval pending, no file. After one LEFT: the returned command was only `…_END'`; the complete file write executed without approval. No deletion or production file was used.

Recommended approach: distinguish reliable agent-owned tracking from uncertain input; track supported edits with a cursor; use cells, actual wrap metadata and confirmed prompt boundaries if reconstructing visible logical lines. Never elevate one row or stale tracking into a complete command. Do not inspect hidden text or offscreen history. If completeness is unavailable, recover that input before policy/approval/Enter rather than applying a blanket restriction to SSH or ordinary automatic commands. Review, policy and execution records must share command provenance. Simply ignoring dirty state is not a fix.

Acceptance: wrapped LEFT/RIGHT/Backspace/Tab/history, wide/combining-character prompts and input beyond the viewport cannot turn a fragment into the policy command. Ordinary verified automatic commands still work.

## B. Separate cancellation from physical input cleanup — P1

`session.rs:1985` sends Ctrl-U for denial/expiry then resets tracking. In default Bash editing, a suffix can remain to the right of the cursor. The same path also serves takeover/disconnect/release; policy Deny contains the same assumption. Human-input tracking already avoids treating Ctrl-U as proof of an empty buffer, but approval cancellation does not share that rule.

Recommended approach: resolve cancellation immediately while retaining physical input state. In a confirmed shell editing context, safely cancel the complete line and wait for an observed empty boundary before resetting tracking. If cleanup is unverified, keep remaining input visible and explain that it needs clearing; do not append a new agent command. Do not inject generic Ctrl-C/editing sequences into editors or unknown foreground programs. Do not retain human input content.

Acceptance: start/middle/end cursors and wrapped input across denial, expiry, release, disconnect and takeover leave no hidden suffix or automatic execution. Recovery must not create a permanent unexplained `input_pending` state.

## C. Make PTY I/O success correspond to committed state — recommended hardening

`session.rs:1092` changes the model and emits dimensions before discarding the PTY resize result. `write_pty` discards write/flush results too. Ordered requests are not proof of successful I/O.

Recommended approach: return resize failure and do not commit failed dimensions; distinguish requested size, applied local PTY size and rendered size through content-free diagnostics. Do not report success or retry automatically after possibly partial input delivery. Checking local PTY size cannot certify remote geometry.

In the geometry comparison, the same input started at zero-based `(row 3, col 12)`. After 30 LEFT keys the consistent control reached `(row 2, col 86)`, while the 80-column PTY case reached `(row 3, col 0)`. Core/xterm grid and cursor parity passed in both. Opening review set the PTY to 18 rows/104 columns in both. The mismatch was injected by the fixture; this does not establish that Conn creates it in the user's environment.

Acceptance: failing PTY adapters and real SSH fixtures exercise errors and recovery; fixed-window review cycles, rapid opening/closing and input during long output preserve ordering. Avoid per-frame resize and arbitrary timing delays.

## D. Fence renderer readiness by attachment generation — bounded hardening candidate

`packages/ui/src/lib/terminalOutput.ts:18` still invokes an old in-flight completion after `clear()`. The reset completion in `Term.svelte` checks only `mounted` before setting `renderReady=true`. Fence these callbacks by the current attachment/checkpoint generation.

Also check whether checkpoint waiting silently drops keys indefinitely. Show delayed recovery and explicit reconnect through the existing connection UI, without replaying uncertain input. Collect only focus/readiness/queue progress/delivery outcome metadata. This is a candidate for Vim investigation, not a proven cause of the reported freeze.

Acceptance: delayed obsolete completions, disconnect/reconnect and reattachment during real Vim editing cannot restore readiness from an old checkpoint. Input becomes ready only after the current checkpoint is applied.

## Scope and validation

Recommend **A+B as the first fix bundle**. Design C around explicit failure outcomes and validate D's generation boundary separately. Include correctly and incorrectly marked colored prompts in validation; do not automatically rewrite user PS1 or strip colors.

Do not append newlines to arbitrary terminal output, remove all motion, force 80 columns, or retry resize without checking outcomes. Do not equate Linux web tests with native Mac acceptance or merge the shell redraw and Vim input-freeze reports into one established cause.

Final checks must cover intended cursor location, complete review text, policy outcome, actual synthetic output/file contents and cancellation leftovers in addition to core/xterm parity. Use local and disposable real SSH shells, plain/colored/wide/multiline prompts, editing and takeover, fixed-window review cycles, and native Mac Vim edit/save. The user's gateway and physical IME require separate evidence.

The two diagnostic web scripts, synthetic screenshots and JSON from this investigation are preserved under `/tmp/conn-terminal-investigation/`. Their `PASS` output means capture/parser parity, not semantic correctness. Permanent regression tests must assert the intended correct behavior.

References: [GNU Bash killing commands](https://www.gnu.org/s/bash/manual/html_node/Commands-For-Killing.html), [Linux PTY geometry and error returns](https://man7.org/linux/man-pages/man2/TIOCSWINSZ.2const.html), [separate Vim input investigation](vim-input-investigation.md).
