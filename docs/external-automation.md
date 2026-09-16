# External terminal automation

[한국어](external-automation.ko.md)

**UNRELEASED — private external automation.** This guide describes the replacement
contract under implementation, not protections available in v0.5.1. That release
uses the earlier recorded, agent-based automation path. The first replacement
adapters target macOS Apple Silicon and Linux D-Bus; the Windows adapter is not
implemented. Native Apple Event and external-launcher acceptance checks remain
release gates.

## Enable once, choose profiles

In **Settings → Automation**, choose the saved profiles an external application may
use, then enable external automation. It is off by default. Upgrading from the previous
adapter retains the profile selection but requires one explicit re-enable
(`requiresReenable` in settings metadata). An earlier approval must not silently
become permission for direct execution.

This grant allows local AppleScript callers to launch supplied programs with an
allowed local profile; it is **not an executable allowlist**. Authorized external
operations have no agent control request, proposal, command-policy approval or
Grace card. Actual AI-agent operations in ordinary sessions keep their existing
mode, policy and takeover rules.

macOS may also ask whether the sender may control Conn. Session ownership comes
from the native sender and its process lifetime, not its displayed application
name. Create, write, poll and release in the **same sender process**. Separate
`osascript` invocations cannot reuse a previous invocation's session handles. If a
launcher uses `osascript`, that process is the sender Conn can identify.

## Start a program in a new window

```applescript
tell application "Conn"
    create window with default profile command "/bin/sh -c '__RUN_COMMAND__ ; echo \"Press [Enter] key to exit.\"; read ANSWER;'"
end tell
```

[Complete window template](../examples/applescript/external-window.applescript).
The external application replaces `__RUN_COMMAND__`; Conn does not expand it.
For a local demonstration, substitute `pwd`.

The `command` replaces the allowed local profile's program and arguments for this
session. It is **started on the new PTY**, not typed into a profile shell. The
profile's cwd and environment remain in effect; the temporary executable and argv
are not saved back into the profile.

Conn tokenizes POSIX words, quotes and escapes. It does not expand variables,
substitute commands, glob paths or insert an implicit shell. Unquoted shell
operators and malformed quoting are rejected with a payload-free error. Use an
explicit `/bin/sh -c '…'` argument when shell syntax is required. Command override
is supported only for local macOS profiles, not SSH, WSL or Docker profiles.

The window's private renderer must be ready before the child starts. Omitting
`command` starts the allowed default profile privately. A successful reply means
the process was spawned; it does not establish command completion or successful
authentication. A lost reply may follow a successful spawn, so do not automatically
retry an uncertain creation request.

In this example, `echo` and `read` provide the message and Enter wait. After Enter,
`/bin/sh` may exit. Conn retains the finished terminal view until the human closes
it; no fallback shell is started. Closing the window terminates its child and
cancels its queue while leaving other windows alone.

The return value is an opaque session handle, not a window object. Only the
commands below are supported; current-session object scripting and split-pane
object models are not implemented.

## Write to the caller's private session

```applescript
tell application "Conn"
    activate
    set sessionID to create session
    set requestID to write text "pwd" to session sessionID
    -- Poll request state requestID; see the complete example for timeout handling.
end tell
```

[Complete input and polling example](../examples/applescript/external-connect.applescript).
Run `osascript external-connect.applescript` for a `pwd` demonstration, or provide
one terminal input line as its argument. The default profile must be allowed.

`write text` uses a dedicated external-input path. Conn does not reconstruct the
input as a command, analyze it against agent policy or add an echo. The child may
echo received bytes normally. Input is serialized in order and stays bound to the
exact private session the caller created; switching tabs never redirects it.
PTY readiness does not identify a password prompt or prove authentication has
finished. The external application determines when its next input is appropriate.

## Script commands

| Command | Result |
| --- | --- |
| `create window with default profile [command "program and arguments"]` | New private window and opaque session handle after successful spawn. |
| `create session [profile "profile-id"]` | New private session using the allowed profile; omitted profile uses the default. |
| `write text "text" to session id [newline true] [intent "reason"] [request id "unique-id"]` | Request ID; queues one line and optional Return. `intent` is accepted only for compatibility, ignored and discarded. |
| `request state requestID` | Plain state string. |
| `request status requestID` | Metadata-only JSON: `requestId`, opaque `session`, `state`, optional generic `error` code. |
| `session status sessionID` | Metadata-only JSON: opaque `session`, `processAlive`, `externalPrivate: true`, `inputAvailable`. |
| `cancel request requestID` | Cancellation result; revokes this session's external writer and cancels its remaining queued writes. |
| `release session sessionID` | Revokes the external writer; leaves the terminal available to the human. |

Write states are `queued`, `delivering`, `delivered`, `cancelled` and `failed`.
`delivered` means the PTY accepted the input, not that a command or login succeeded.
There are no approval, proposal or Grace states, and no `detail.proposed`, raw
error details, input text, intent, paths or terminal output in status results.

A request contains one UTF-8 line, at most 16 KiB, without NUL or embedded control
characters. Send Return with the `newline` option. An explicit request ID may be
repeated with the same payload/options during the same app run; a conflicting
reuse is rejected. Deduplication uses an in-memory per-run keyed digest, not a
saved copy of the input or a persisted password hash. Never retry automatically
after a crash or an unknown timeout outcome.

Limits: 16 waiting writes per session, 32 live owner bindings, 256 request IDs per
app run and a 120-second write deadline including queue time. Completed polling
entries contain metadata only. Payload buffers are released on delivery, failure,
cancellation or timeout. Bytes already sent to the child cannot be recalled.

## Private sessions and human takeover

External sessions are private from creation. They are absent from MCP/public IPC
tab lists, and explicit public access cannot reveal their screen, input, events,
status details, titles or cwd. Only the owning native window receives their output.
**Sharing a private external session with an AI agent is not implemented.** A
normal manually opened Conn session retains its existing collaboration behavior.

Human input, explicit takeover, cancellation, release, disabling automation,
removing the permitted profile or closing the session revokes the external writer
and cancels queued writes. Human input proceeds in the private terminal. Later
requests cannot reacquire the writer or target an existing human tab; the caller
must create a new session. Launcher exit does not kill a successfully started
terminal, but an unverifiable caller cannot deliver pending writes.
Released session handles become unavailable; status cannot be used to reacquire
input access.

xterm terminal-protocol replies (for example cursor/status responses) are not
human input. They must not revoke the external writer or create activity records.

## Recording and limits

Conn does not record activity for private external sessions: no private-session
or AppleScript audit events, timeline rows, recent-activity settings, raw input
history, payload-derived titles or persisted request results. This applies to
human typing in that external-origin session for its entire lifetime. Settings
store permissions only, and restart restores no owner handles, queued payloads,
processes or requests. Programmatic clipboard writes from private terminal output
are disabled; explicit human copy remains possible.

This is not a zero-retention or OS-isolation claim. The terminal view and scrollback
exist in memory. The child, shell history/tracing, argv, environment, OS buffers,
clipboard, crash dumps and external-application logs are separate surfaces. There
is no password detector, authentication-success detector or credential vault.
Same-user tools may use their own shells and files outside Conn's API gates.

This checkout also disables raw human-input command history in ordinary sessions. Existing
audit files, saved timeline records and backups are not automatically erased.
Review historical data separately before sharing it. See the
[trust model](security.md#private-external-sessions-unreleased).

## Validation and release gates

### Quick check on an Apple Silicon Mac

Use a **new build containing this unreleased change**, not the v0.5.1 download.
Use synthetic markers only and confirm which Conn app your script targets.

1. Open **Settings → Automation**, keep a local default profile selected and
   explicitly re-enable the migrated permission. The pane should show permissions,
   not recent request history.
2. Run this in Script Editor. Expect a new private window, both markers, and no
   Conn approval/Grace card. Press Enter: the child exits and the finished view
   remains, without starting another shell. Repeat with Conn already running.

   ```applescript
   tell application "Conn"
       create window with default profile command "/bin/sh -c 'echo CONN_PRIVATE_START; echo Press-Enter-to-finish; read answer'"
   end tell
   ```

3. Run the [complete input example](../examples/applescript/external-connect.applescript)
   in **one** process with `echo CONN_PRIVATE_WRITE` as its argument. In its success
   branch, put `set resultJSON to request status requestID` before `release session`
   and return `resultJSON`. It must contain metadata only, never the marker or command.
   Check the marker on the terminal screen; `delivered` alone is not command success.
4. For takeover, run a copy of that script with `delay 10` and then
   `write text "echo CONN_MUST_NOT_RUN" to session sessionID` inserted immediately
   **before** `release session sessionID`. During the delay, type Ctrl-C in the
   private terminal. The late write must be rejected and its marker must not appear.
   Query `session status sessionID` before that late write: expect
   `inputAvailable: false` or a generic unavailable error if the revoked handle has
   already been removed. Catch that expected error to continue to the write check.
   Terminal-protocol replies by themselves must not trigger this takeover.
5. Confirm there are no new private command/control rows in the timeline and no
   markers in Conn-managed saved activity. From a separate existing MCP connection,
   `terminal_list_tabs` must omit the private session and snapshots must not expose
   its marker. Keep ordinary sessions open to check they still collaborate normally.

These are manual acceptance checks, not a claim that they passed on your Mac.

The intended checks cover exact argv and Unicode/quotes, launch before renderer
readiness, child exit without fallback, caller identity, request bounds,
revocation during queued input and private data exclusion from every Conn-managed
recording/public-IPC route. Actual agent policy tests must continue to pass.

macOS CI checks bundle metadata, the scripting dictionary and example compilation.
Compiling a script does not execute Apple Events or establish consent behavior.
Before release, test the signed Apple Silicon build with a disposable launcher:

- Cold and warm launch; first macOS consent accepted and denied.
- Direct startup program, explicit shell syntax, Unicode and quote boundaries.
- Private output visible to the human and unavailable to MCP/public IPC.
- Human takeover, cancel, disable, profile removal, close and two concurrent callers.
- Metadata-only status, ignored `intent`, no private activity recording, and upgrade re-enable.
- The external application's actual permitted template, with synthetic input.

Later AI sharing remains separate work. Ordinary local Bash/Zsh sessions now have
[shell command integration](shell-integration.md). External private sessions never
install it and remain unrecorded, including after human takeover.

## Linux D-Bus adapter (unreleased)

The native Linux app exports `dev.eggp.Conn` on the user session bus, object
`/dev/eggp/Conn/Automation`, interface `dev.eggp.Conn.Automation1`.
`Call(operation: string, payload: string) → string` takes the same JSON parameters
and returns JSON. Supported operations are `window.create`, `session.create`,
`session.write`, `session.status`, `session.release`, `request.status`, and
`request.cancel`. Both create operations open a dedicated native window on Linux.

Enable external automation and select profiles in Settings → Automation, then add
absolute caller executable paths under **Allowed Linux executables**. An empty
list denies all callers. Keep one D-Bus connection open for the lifetime of your
session; separate `gdbus` commands have different owners and cannot reuse handles.
Start Conn normally before connecting; automatic D-Bus service activation is not
installed by this change. A launcher may start Conn and wait for its bus name.

The adapter obtains UID/PID from the bus and checks `/proc` executable and process
start time. It never accepts a caller identity from JSON. Session ownership includes
the unique bus connection; disconnect, process exit and executable change invalidate
its authority. Profile/permission changes use the shared service's revocation path.
Requests are bounded to 64 KiB and eight concurrent operations; the shared service
also bounds sessions, queued input and payload size.

Allowing Python, a shell or another interpreter authorizes scripts run through that
executable, not a particular script. Path authorization is not code signing or a
sandbox against other processes under the same user. D-Bus and the OS transport
may temporarily hold payloads; do not treat this as protection against session-bus
monitoring by privileged tools. No automation payload is intentionally persisted
by Conn. Authorization is off by default and is separate from MCP permissions.

For an isolated native acceptance run, the desktop adapter accepts
`CONN_CONFIG_DIR` for its config/data directory and `CONN_SOCKET` for its agent socket.
Use a private `dbus-run-session` and virtual display, never the user's live settings.
The external `terminal-auth-fixtures` project drives the real native windows and
common automation service; it does not bypass renderer readiness or authorization.
