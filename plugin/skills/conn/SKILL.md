---
name: conn
description: Procedure and etiquette for running commands in a shell shared with a human (conn). Use it when the user says "conn", "the shared shell", "in my terminal", "look at the screen and …", "in the shell that's open", and Conn terminal_* tools are available. For tasks the user wants in Conn, use its tools instead of the built-in shell.
---

# Working in conn

This shell is **watched by a human who can step in at any moment**. You are a guest. The tools arrive over MCP (`terminal_*`); this document covers only the order of things and the etiquette.

## Procedure

1. **Look first.** Read the screen with `terminal_snapshot`. Is there a prompt? Is something running? What did the human just do? Commands the human ran may be on the screen.
2. **Ask with a reason.** The `reason` you pass to `terminal_request_control(reason, command)` appears on the human's screen verbatim. Keep the reason concrete and include the exact planned shell command in `command`, so the human can inspect it before granting control. This metadata does not execute the command. If the call does not return for a while, the human is deciding. On `control_denied`, observe only and tell the user. Do not repeat the request unless they ask you to retry.
3. **Separate typing from running.** `terminal_type` only types. Running is `terminal_send_key("ENTER", intent)`. One command at a time. Never put a newline in `text`.
   - **`intent` is required.** One line saying what the command does and what it changes. It sits at the top of the human's approval card and in the audit log. Describe the outcome: `"delete the hello-mel directory (a git repo, unrecoverable)"`. Without it you get `intent_required`.
   - **One dangerous command per line.** Chaining a delete, privilege escalation, force push or overwrite to other commands with `&&` `;` `|` comes back `denied` without an approval (label "run dangerous commands alone"). If you need a `cd`, run the `cd` on its own line first, then send only the dangerous command.
   - The engine recognises `\rm`, `/bin/rm`, `command rm`, `sudo rm`, `sh -c '…'`, `eval`, `xargs`, `find -delete`, `python -c` and the like. Do not try to route around it. Just ask.
4. **Act on the response status.** (table below)
5. **Check the result.** After running, take another `terminal_snapshot` and read the output and the prompt. For long-running commands, snapshot again after a few seconds.
6. **Give it back when done.** `terminal_release_control`. Then report briefly what you ran and what you saw.

## What to do per `send_key(ENTER)` status

| status | meaning | what to do |
|---|---|---|
| `executed` | it reached the shell | check the result with a snapshot |
| `pending` | policy wants a human approval | poll `terminal_check_approval(approvalId)` every few seconds. **Never work around it.** On `denied`/`expired`, drop that command and tell the user |
| `cancelled` | the human interrupted during the grace window | do not retry; ask the user why they stopped it |
| `rejected` | the human declined the proposal (Co-pilot mode) | do not resend the same command; propose a different approach |
| `denied` | policy forbids it (see the label), a dangerous command was chained with others, or a protected path was touched | if the label is "run dangerous commands alone", split and resend. Otherwise there is no other way; tell the user |

## Revocation and hand-back

- `input_unverified` means the complete edited command cannot be established, or cancelled input still remains. Never submit a screen-row fragment as the command. Read the shared screen; if it is safe to interrupt your own abandoned shell line, explicitly interrupt it and wait for a fresh prompt before typing a complete new command. Do not retry a command the human rejected.
- A denied/expired approval can return `inputRetained: true`: cancelling permission does not guarantee the foreground program cleared its input. Do not append another command. Conn automatically clears only a confirmed idle shell; an unverified foreground needs explicit recovery.
- `input_outcome_unknown` means some input may already have arrived. Inspect the screen and recover deliberately; never replay the failed write or Enter automatically.

- Any keystroke from the human revokes control instantly and the write tools vanish from the list. **Do not fight it.** Snapshot to see what the human is doing, and wait.
- When the human hands control back (a `control handed back` notification) continue where you were. Snapshot once more first; the screen may have changed.
- If you wait long, the lease expires. Just request again.

## You see only what the human sees

Your `terminal_snapshot` is the current terminal grid of your shared session, independent of window focus, minimization and active tab. It contains PTY-rendered output, not raw input or scrollback. The connection remains bound when the human changes tabs. Human input preempts control; sharing revocation and disconnect cancel pending work. Use `terminal_request_attention(reason)` when a human decision is needed. Never use another access path to bypass participation.

## Tabs

`terminal_list_tabs` shows the tabs and which one the human is watching. Open a new tab with `terminal_open_tab(reason)` only when you truly need a separate shell. The human's view stays where it is; the new tab knocks so they can look. Your access there follows participation, mode and control, not whether it is being watched. You cannot change what the human is looking at. `terminal_switch_tab(tab)` moves only your own connection, and leaving a tab releases your control and cancels what you had pending there. Finish or release before you move.

## Joining

A new connection must be allowed once by the human in the Conn window. Until then calls return `admission_pending`: tell the human to press **Allow** in Conn, then call `terminal_snapshot`, which waits briefly for their answer. `admission_denied` is final for this connection; stop and tell the user instead of retrying.

## Co-pilot mode

When the human has Co-pilot mode on, the line you type does not enter the shell; it is shown as a **proposal**. `send_key(ENTER)` waits until the human presses ⏎ or declines, then returns `executed` or `rejected`. Propose one short, readable line at a time.

## Never

- Run the same command through a built-in shell/exec tool. It skips the audit log and the policy.
- Route around `pending` in any way.
- Chain a destructive command to other commands. One approval covers exactly one dangerous action.
- Write an `intent` that says something other than what the command does. The human compares the two.
- Copy secrets (tokens, passwords) visible on the screen.
