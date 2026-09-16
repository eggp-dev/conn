# External terminal automation

[한국어](external-automation.ko.md)

**Experimental in v0.5.0; disabled by default.** The first
native adapter is macOS AppleScript. Windows/Linux share the internal contract;
their external launch adapters are not implemented yet. Actual PAM compatibility
and macOS permission behavior require a Mac acceptance test.

## Connect an external application

A PAM client or launcher can open Conn, create a tab from a saved profile and send
its connection command to that session. The application owns the AppleScript;
Conn receives Apple Events. An already-running Conn follows the same path.

1. In **Settings → Automation**, select allowed profiles and enable AppleScript.
2. Use the [example script](../examples/applescript/pam-connect.applescript) in the
   external application's script template. Run `osascript pam-connect.applescript`
   for a harmless `pwd` demonstration, or supply one SSH connection command as its
   argument. The default terminal profile must be among those allowed.
3. macOS may request permission for the sender to control Conn. Then approve the
   session's control request inside Conn. Command policy and Co-pilot proposals
   still apply after control is granted.
4. The script polls its request and releases control after input delivery. The
   terminal stays open for the human.

**Create, write, poll and release must run in the same sender process.** Separate
`osascript` invocations have different identities. When a PAM app uses osascript,
Conn identifies the intermediary, not the PAM vendor. No persistent per-vendor
trust is inferred from an application name. Settings grant profile access to local
AppleScript callers, while each new session requires control approval.

A minimal source template:

```applescript
tell application "Conn"
    activate
    set sessionID to create session
    set requestID to write text "pwd" to session sessionID intent "Show the current directory"
    -- Poll request state requestID; see the complete example for timeout handling.
end tell
```

This is Conn's dictionary, not drop-in iTerm2 or Terminal compatibility. A PAM
product that permits script templates can use it; a fixed bundle ID or compiled
vendor script may need a vendor adapter. Check the actual product and sanitized
script before claiming compatibility. Multi-window/split-pane scripting is not
part of this first adapter.

## Script commands

| Command | Result |
| --- | --- |
| `create session [profile "profile-id"]` | Stable session ID; creates and selects a tab. Omitted profile uses the default. |
| `write text "text" to session id [newline true] [intent "reason"] [request id "unique-id"]` | Request ID immediately; queues one line. Return goes through command policy. |
| `request state requestID` | Plain state string, suitable for polling. |
| `request status requestID` | JSON with request ID, session, state and result detail. |
| `session status sessionID` | JSON mode, effectiveMode, processAlive and attended. |
| `cancel request requestID` | Whether cancellation was requested; revokes that automation session and its remaining queue. |
| `release session sessionID` | Revokes automation access; leaves the tab open. |

States: `queued`, `awaiting_permission`, `delivering`, `grace`,
`awaiting_acceptance`, `awaiting_approval`, then `delivered`, `denied`, `cancelled`
or `failed`. A non-newline write in Co-pilot remains a proposal; inspect result
`detail.proposed` / `detail.inputDelivered` for that distinction.

`delivered` means Conn processed the requested input, not that the shell command
succeeded or SSH completed authentication. PTY/UI readiness is not prompt or login
readiness. Do not blindly queue follow-up commands after an SSH connection command.
Scripts cannot read terminal output through this first adapter.

Text is limited to one line (16 KiB), without embedded control characters. Use the
newline option to send Return. Repeating an explicit request ID with the same
payload returns the existing request; another payload is rejected. This applies
only within one app instance. Never automatically retry after a crash or an
unknown timeout outcome.

Limits: 16 waiting writes per session, 32 active automation sessions, 256 retained
requests per app run, and a 120-second request deadline including queue time.
Request IDs remain retained for deduplication; restart Conn after reaching that
limit. Cancellation cannot undo bytes already delivered.

## Shared architecture and safety

```text
PAM → AppleScript → native Cocoa command adapter
                             ↓
             shared automation service (crates/frontend)
         caller/session ownership · bounded queue · request state
                             ↓
              existing Hub / Engine / Session
       control gate · policy · takeover · the same UI/timeline
```

The `.sdef` dictionary and Objective-C command class are bundled on macOS. Commands
suspend and resume Apple Event replies while work runs off the Cocoa main loop.
The Rust FFI exposes only the narrow automation service, never trusted UI dispatch.
The sender is derived from the Apple Event PID and process start time. It cannot
select an existing human tab by passing its ID.

Runtime startup is idempotent and shared with the UI. External creation before
frontend startup is adopted by the frontend's startup snapshot. Pre-attachment
terminal output is buffered with a size limit. If normal UI startup has already
created a default tab, an external create opens another tab. It does not replace
the user's existing tab or retarget subsequent writes when focus changes.

External automation uses a dedicated connection through existing agent permission
checks; it is never submitted as human keystrokes. Its name and timeline badge
identify AppleScript/external automation. Human takeover, policy denial, scope
revocation, session close or request cancellation stops remaining queued work.
After losing control it cannot silently reacquire through a later queued write.
Settings changes revoke existing automation connections. No external approval or
policy-editing command is exposed. Existing MCP behavior is preserved.

Conn retains its [same-user trust boundary](security.md); this is not OS isolation.
Ordinary commands and intents can be stored in audit/timeline records. **Credential
text injection and automatic secret redaction are not supported.** Use SSH keys,
SSH agents or PAM-managed authentication without inline secrets. A request marked
sensitive is rejected by the shared API before queueing; the scripting dictionary
does not offer a password input command. Shell history and echoed output are
separate storage surfaces.

## Verification

Shared tests exercise real disposable PTYs: disabled/profile scope, cold startup,
control approval, delivery, duplicate IDs, foreign callers, cancellation, human
takeover during grace, and rejection of multiline/control/sensitive input.

macOS CI builds an ad-hoc debug app and checks its Info.plist, dictionary and
example compilation. This does **not** establish Apple Event delivery or macOS
consent behavior. Before release, test the signed/notarized Apple Silicon app:

- Cold and warm launch from one osascript invocation.
- First-time macOS consent accepted and denied.
- Conn control approval, policy approval/denial, Co-pilot acceptance and grace.
- Unicode, quotation marks, newline false, request status and release.
- Concurrent callers, tab switching, human takeover, close, timeout and cancel.
- Actual PAM template, sender lifetime and SSH-managed authentication.

References: [Cocoa scripting](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/ScriptableCocoaApplications/SApps_intro/SAppsIntro.html)
(archived conceptual guide), [deferred command replies](https://developer.apple.com/documentation/foundation/nsscriptcommand),
[Tauri macOS bundles](https://v2.tauri.app/distribute/macos-application-bundle/),
[iTerm2 terminology](https://iterm2.com/documentation-scripting.html).
