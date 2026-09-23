# Agent socket protocol v2

**v0.8.0 preview.** v2 removes public owner/frontend operations and headless
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

### Admission

The desktop app asks its owner before a new connection participates. `hello` then returns
`"admission":"pending"` with no session, mode or attention data. While pending, discovery
calls (`sessions`, `list_tabs`, `affordances`, `navigation_affordances`, `status`) return
`admission_pending` at once; other calls wait up to 25 seconds for the owner's answer, so the
first real call succeeds right after **Allow**. A denied connection receives
`admission_denied` for everything; the answer is final for that connection and is never
inherited by a later connection with the same label. `tools_changed` is sent when the owner
answers. Embedders of the core default to admitting every connection (`"admission":"granted"`).

A connection binds to the currently attended shared session it may participate in at hello. When the human
changes tabs the binding does not follow. A request can name `session` explicitly, or
use `switch_tab`. Hello may return null session/mode fields when no shared session is
available; the connection can still appear as a sharing candidate in the owner UI.

`kind: "human"`, `kind: "frontend"`, raw output subscriptions, `input`, `take`, approvals,
mode/settings changes, `set_attended` and sharing are unavailable
on this transport. These belong to the window-bound native owner bridge. The browser
test adapter uses that same in-process owner bridge behind its development authentication;
it is not an agent-provided frontend identity. Never expose it publicly.

## Sessions and participation

| Method | Parameters | Result |
| --- | --- | --- |
| `sessions` / `list_tabs` | — | Discovery of sessions this connection may participate in, count, bound session and attended session. Private and unselected sessions are omitted. |
| `open_tab` | `{reason?}` | New tab and connection binding; the lease and pending work in the tab being left are released. The human view stays where it is and the tab asks for attention; access follows participation, mode and control. Requires the host opener and the matching capability. |
| `switch_tab` | `{tab}` number or ID | Changes this agent's binding, not the human's displayed tab. Leaving a tab releases the lease and cancels this connection's pending requests there: a lease belongs to its tab, and a connection holds one at a time. Because any request may name a permitted `session`, `request_control` for one session likewise gives up what the connection holds in every other session; reads by name cost nothing. A live, participating source whose owner mask omits `switch_tab` refuses the move (`masked`); a lost source never blocks recovery. Checks destination participation and navigation capability, even if the old shell closed or access was revoked. |
| `navigation_affordances` | — | Connection-level navigation capabilities derived from permitted destinations; remains available when the bound session is unavailable. |
| `request_attention` | `{reason?}` | Requests that the human return to the bound tab. |

Discovery is not access. Operations require participation in the selected session.
Window focus, minimization, occlusion, overlays and tab attendance do not grant or
revoke access. The agent remains bound to its session when the human changes tabs.
Control, mode, policy, approvals and lease checks still govern writes.

## Session snapshot

`snapshot` returns the current terminal grid parsed from PTY output in the core.
It excludes raw input, scrollback, environment and process memory. It does not track
the owner's scroll position or include window chrome and overlays.

| Field | Meaning |
| --- | --- |
| `surfaceId` | Stable session screen identity |
| `generation` | Sharing boundary |
| `revision` | Terminal text/cursor/alternate-screen revision |
| `outputSeq` | Current PTY output sequence |
| `size` | `{rows,cols}` |
| `cursor` | `{row,col}`, or null when hidden |
| `screen` | Current grid rows, with ANSI conceal and explicit equal colors suppressed |
| `alternateScreen` | Whether the alternate terminal screen is active |
| `controller`, `processAlive` | Collaboration/process state |
| `mode`, `effectiveMode` | Configured and current agent behavior |

There is no renderer publication, heartbeat expiry or focus-dependent fallback.
A sharing transition invalidates previous tokens and queued response disclosure.
See [trust limits](security.md#one-terminal-one-presented-surface).

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
| `check_approval` | `{approvalId}` | State of the caller's command approval; `inputRetained` reports cancelled input awaiting explicit recovery |
| `control_request_state` | `{requestId}` | Pending/granted/denied/expired control request |
| `proposal_state` | `{proposalId}` | Drafting/ready/executed/rejected/denied proposal |
| `exec_state` | `{execId}` | Scheduled/executed/cancelled execution |

Co-pilot accumulates an agent proposal rather than immediately typing it. Enter waits
for human acceptance and policy evaluation. Autopilot may write while holding control;
control approval is separate from command policy. Grace and pacing waits are handled by
the adapter. Surface/participation changes cancel or suspend work instead of creating
an invisible continuation path. A disconnected connection loses its lease and pending work.
Transport closure is monitored while requests wait for control, Co-pilot acceptance or
grace expiry; a pending request does not delay disconnect cleanup. Cancellation does
not undo commands that already executed before the disconnect.

A client may pipeline requests and close its write side. Requests queued before the
close are still answered when they only read (`hello`, discovery, `snapshot`, `status`
and state polls). Anything that writes, requests control or changes the binding is
answered with `connection_closing` and does not run. An agent never submits a cursor
line containing text the snapshot withholds; that ENTER returns `invalid_input`.

There is no public `analyse` filesystem-inspection route or full owner status payload.
Use the review UI for structural policy details and the current snapshot for terminal
context. `conn log` is an explicit local file read using the CLI process's OS privileges,
not a screen/history capability granted by this protocol.

An edited command must remain fully tracked to enter the policy gate. Completion,
history and unsupported edits return `input_unverified` instead of evaluating a
cursor-row fragment. Cancellation at a confirmed idle shell waits for a fresh
input boundary; otherwise the typed line stays visible and new agent appends are
blocked until explicit recovery. Human input cancels pending review and takes over
the visible line; it is never silently rewritten with guessed editing keys.

`input_outcome_unknown` reports a failed PTY write or flush whose partial delivery
cannot be excluded. No automatic replay is allowed. `resize_failed` reports that
the local PTY did not confirm the requested size; the core does not publish that
size as applied. This does not establish the dimensions of a remote SSH endpoint.

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
PTY output is never an event, and owner approval cards, other actors' original request
payloads and owner settings are not made available by setting an agent's `streamOutput` flag.

Queued agent events and successful responses carry a participation generation check.
Snapshots additionally carry the surface generation. Before writing to the transport,
the server rechecks the session's current permission boundary and drops obsolete
content. Revocation cannot recall bytes already delivered to the client/model.

## Relevant errors

`hello_required`, `owner_required`, `surface_unavailable`, `not_available`,
`busy`, `not_controller`, `lease_expired`, `process_exited`, `invalid_input`,
`not_found`, `rate_limited`, `input_pending`, `input_unverified`, `input_outcome_unknown`, `resize_failed`, `approval_pending`, `exec_pending`, `proposal_pending`, `intent_required`,
`masked`, `control_denied`, `wrong_mode`, `unsupported`, `connection_closing`, `admission_pending`, `admission_denied`, `parse`.

Private/unauthorized session access uses a generic unavailable response. The owner can start the app with `CONN_TRACE_REFUSALS=1` to print the cause of each such refusal to stderr (reasons and identifiers only). A client should
not automatically bypass an unavailable surface, switch to a separate shell, or claim a
command ran merely because it received a lease. Ask the human to restore the shared
view and take a fresh snapshot.

## Native automation and extensions

[External automation](external-automation.md) is a separate owner-bound native adapter,
not another public socket role. Only the human owner may transition its private session
to shared participation. [Native extensions](extensions.md) are declarative themes; they
cannot call owner operations through agent IPC either.

## Owner bridge additions (unreleased; unavailable on the agent socket)

- `admission_snapshot`: `{revision,pending}`. Admission events carry the same app-wide revision;
  request identity is the live `connId`. UI reconciliation discards stale per-connection events.
- `sharing_state({session})`: `{revision,participants}` read together under the Session lock.
- `set_sharing({session,shared,connectionIds,expectedRevision?})`: a stale participation revision
  returns `sharing_changed` before external-writer revocation. New UI calls always send it; existing
  owner calls and the `sharing_participants` array result remain compatible.
- Full owner status includes `scheduledRemainingMs` for recovering a missed Grace event. Agent
  status remains limited; these owner commands do not become MCP tools.

한국어: 위 API는 앱 소유자 경로에만 추가되며 에이전트 소켓에서는 사용할 수 없습니다.
연결 승인과 참여 선택은 각각의 revision으로 경합을 처리합니다. 오래된 공유 선택은 외부 입력을
회수하기 전에 거절하며, UI는 다시 불러온 선택을 사용자가 확인한 뒤 적용하도록 합니다.
