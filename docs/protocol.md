# Agent socket protocol v2

**v0.7.0 preview.** v2 removes public owner/frontend operations and headless
observation. v0.6.0 uses the previous contract. Update the app, CLI and
MCP adapter together; there is no downgrade to a raw-output or Rust-screen fallback.

On Unix, `~/.conn/conn.sock` is local and owner-only (directory 0700, socket 0600).
Windows uses an owner-only local Named Pipe. Override with `CONN_SOCKET` or `--socket`;
Windows accepts `\\.\pipe\...` and maps path-like overrides to a stable pipe name.
Messages are newline-delimited JSON. Responses are `{"id", "result"}` or
`{"id", "error":{"code","message"}}`; events have no request ID.

## Identity and owner boundary

The first message must be `hello` with `kind: "agent"`. Identity is immutable for that
connection. `name`/`agentId` are labels, not permissions. The server assigns `conn`, and
owner sharing selections bind to that actual connection ID. Reconnecting with the same
label creates a new identity and does not inherit explicit membership.

```json
{"id":1,"method":"hello","params":{"kind":"agent","name":"my-agent"}}
```

Example response (IDs and modes vary):

```json
{"id":1,"result":{"protocolVersion":2,"conn":3,"kind":"agent","attended":"t1","session":"t1","mode":"copilot","effectiveMode":"copilot"}}
```

A connection binds to the currently attended shared session it may participate in at hello. When the human
changes tabs the binding does not follow. A request can name `session` explicitly, or
use `switch_tab`. Hello may return null session/mode fields when no shared session is
available; the connection can still appear as a sharing candidate in the owner UI.

`kind: "human"`, `kind: "frontend"`, raw output subscriptions, `input`, `take`, approvals,
mode/settings changes, `set_attended`, frame publication and sharing are unavailable
on this transport. These belong to the window-bound native owner bridge. The browser
test adapter uses that same in-process owner bridge behind its development authentication;
it is not an agent-provided frontend identity. Never expose it publicly.

## Sessions and participation

| Method | Parameters | Result |
| --- | --- | --- |
| `sessions` / `list_tabs` | — | Discovery of sessions this connection may participate in, count, bound session and attended session. Private and unselected sessions are omitted. |
| `open_tab` | `{reason?}` | New tab and connection binding; starts unattended. Requires the host opener and the matching capability. |
| `switch_tab` | `{tab}` number or ID | Changes this agent's binding, not the human's displayed tab. |
| `request_attention` | `{reason?}` | Requests that the human return to the bound tab. |

Discovery is not access. Per-session operations require participation, and observation
and execution require a current presented surface. A selected agent can be denied on
an unattended tab or while its owner surface is obscured. Entrust no longer permits
background observation/execution. An agent-opened tab must be visited by the human before
it can be observed; the agent cannot mark it attended itself.

## Presented snapshot

`snapshot` returns the actual owner-rendered terminal viewport, never independent
scrollback, raw PTY bytes or the internal command-policy tracker. The response includes:

| Field | Meaning |
| --- | --- |
| `surfaceId` | Owner surface identity |
| `generation` | Surface invalidation boundary |
| `revision` | Rendered-frame revision |
| `outputSeq` | PTY output sequence acknowledged by that frame |
| `size` | `{rows,cols}` |
| `cursor` | Visible `{row,col}`, or null when outside the viewport |
| `screen` | Displayed text rows, with concealed text excluded |
| `alternateScreen` | Whether the active rendered buffer is the alternate screen |
| `image` | Optional `{mimeType:"image/png",data:"base64"}` rendered image |
| `imageUnavailable` | True when the renderer supplied text without a raster image |
| `controller`, `processAlive` | Current collaboration/process state |
| `mode`, `effectiveMode` | Configured and current agent behavior |

Publication must match the current output and generation. Missing/old owner frames
return `surface_unavailable`; unattended observation returns `unattended`. The server
may briefly wait for a matching render, but never substitutes hidden backing state.
A sharing transition invalidates previous snapshots and pending response disclosure.
See [visibility and trust limits](security.md#one-terminal-one-presented-surface).

## Agent operations

| Method | Parameters | Result |
| --- | --- | --- |
| `affordances` | — | The currently available agent capabilities |
| `status` | — | Limited controller, mode, effectiveMode, attended, shared and surfaceAvailable state |
| `request_control` | `{reason?,command?}` | Granted lease; with the control gate, waits for the human decision or errors |
| `release_control` | — | `{released}` |
| `type` | `{text}` | `{typed}`; no execution newline |
| `send_key` | `{key,intent?}` | Sent/executed/cancelled/rejected/denied result; Enter requires intent when policy specifies it |
| `interrupt` | — | Ctrl-C through the same PTY; requires this agent's current authority |
| `check_approval` | `{approvalId}` | State of the caller's command approval |
| `control_request_state` | `{requestId}` | Pending/granted/denied/expired control request |
| `proposal_state` | `{proposalId}` | Drafting/ready/executed/rejected/denied proposal |
| `exec_state` | `{execId}` | Scheduled/executed/cancelled execution |

Co-pilot accumulates an agent proposal rather than immediately typing it. Enter waits
for human acceptance and policy evaluation. Autopilot may write while holding control;
control approval is separate from command policy. Grace and pacing waits are handled by
the adapter. Surface/participation changes cancel or suspend work instead of creating
an invisible continuation path. A disconnected connection loses its lease and pending work.

There is no public `analyse` filesystem-inspection route or full owner status payload.
Use the review UI for structural policy details and the current snapshot for terminal
context. `conn log` is an explicit local file read using the CLI process's OS privileges,
not a screen/history capability granted by this protocol.

## Original control request

Include `command` when a planned command is known; it is a declaration, not execution
or permission. Control requests preserve submitted JSON parameter values in
`originalRequest: {method:"request_control",params:{...}}`. The payload is an object of
at most 64 KiB; reason/command are strings or null. Retrying a pending request with a
different non-null command is rejected. Transport whitespace is not retained.

The owner's timeline can show the original request separately from its reason and
actual execution. Denied/expired decisions retain the submitted request where recorded.
Older records without original data remain unavailable, not reconstructed. Sensitive
arguments in explicit agent requests can be recorded; never use request metadata as a
credential channel.

## Events and invalidation

Native frontend events are not a public event feed. Agents receive relevant changes
such as `tools_changed`, `mode_changed`, attention changes, their control revocation,
approval resolution and proposal/execution results. Events are session-tagged. Raw
`output`, owner approval cards, other actors' original request payloads and owner settings
are not made available by setting an agent's `streamOutput` flag.

Queued agent events and successful responses carry a participation generation check.
Snapshots additionally carry the surface generation. Before writing to the transport,
the server rechecks the session's current permission boundary and drops obsolete
content. Revocation cannot recall bytes already delivered to the client/model.

## Relevant errors

`hello_required`, `owner_required`, `surface_unavailable`, `not_available`, `unattended`,
`suspended`, `busy`, `not_controller`, `lease_expired`, `process_exited`, `invalid_input`,
`not_found`, `rate_limited`, `input_pending`, `approval_pending`, `exec_pending`, `proposal_pending`, `intent_required`,
`masked`, `control_denied`, `wrong_mode`, `unsupported`, `io`, `parse`.

Private/unauthorized session access uses a generic unavailable response. A client should
not automatically bypass an unavailable surface, switch to a separate shell, or claim a
command ran merely because it received a lease. Ask the human to restore the shared
view and take a fresh snapshot.

## Native automation and extensions

[External automation](external-automation.md) is a separate owner-bound native adapter,
not another public socket role. Only the human owner may transition its private session
to shared participation. [Native extensions](extensions.md) receive an authorized frame
from the host; they cannot call owner operations through agent IPC either.
