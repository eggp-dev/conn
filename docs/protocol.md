# Socket protocol

`~/.conn/conn.sock` on Unix (directory 0700, socket 0600); a local owner-only Named Pipe on Windows. Override the endpoint with `CONN_SOCKET` or `--socket`. Newline-delimited JSON: one line, one message. Windows accepts a `\\.\pipe\...` name; path-like overrides are mapped to a stable pipe name.

Sessions report `profileId`, `profileName` and `reviewRequired` in `status`. A selected profile is resolved when the session starts; subsequent profile edits apply to new sessions. See [shell backends](backends.md).

```
→ {"id":1,"method":"hello","params":{"kind":"frontend","name":"my-ui","streamOutput":true}}
← {"id":1,"result":{"conn":3,"kind":"frontend"}}
← {"event":"output","data":"<base64>"}
→ {"id":2,"method":"input","params":{"data":"<base64>"}}
← {"id":2,"result":{"written":3}}
```

A response is `{"id", "result"}` or `{"id", "error":{"code","message"}}`. Events have no `id`: `{"event": ...}`.

## Sessions (tabs)

One socket can front several sessions (the app's tabs). Exactly one is **attended** — the one the human is looking at — and a connection is **bound** to the attended session at `hello` time. When the human moves to another tab the connection does not follow. A request may name a `session` explicitly. Every event carries a `session` field.

| method | params | result |
|---|---|---|
| `sessions` / `list_tabs` | — | `{sessions: [{tab, id, current, attended, controller, pending, entrustedTo, attentionRequest, openedBy, processAlive}], tabs, attended, current}` — `tab` counts from 1, `current` is the session this connection is bound to |
| `set_attended` | `{session}` | the human now looks at this session (frontend) |
| `entrust` | `{session?}` | leave the session to the agent while the human is away; cleared when they return |
| `request_attention` | `{reason?}` | the agent asks the human to come back |
| `open_tab` | `{reason?}` | the agent opens a new session (tab) and moves its connection there. `{session, tab, attended: false}`. `unsupported` when the host has no opener |
| `switch_tab` | `{tab}` (number or id) | the agent moves its own connection to another session. The human's view does not change. `{session, tab, attended}` |

In a session the human is not looking at, an agent's `snapshot` is refused with `unattended` and writes with `suspended`. An entrusted agent continues within the policy's `unattended` cap (a mode). Events: `attention_changed{attended}`, `entrusted{agentId, cap}`, `attention_requested{agentId, reason}`, `control_suspended{reason}`, `control_resumed`.

**A tab opened by an agent starts unattended.** The new session emits `tab_opened{agentId, reason}` and carries a pending attention request. Until the human comes to it (`set_attended`) or entrusts it (`entrust`) the agent can neither see nor write there. `switch_tab` emits `agent_switched_tab{agentId, from, to}` on both sessions. Both methods can be masked through the `open_tab` / `switch_tab` affordances and exist only on hosts that install an opener (the app). `switch_tab` remains available while unattended so an agent can return to the tab the human is watching.

## hello

| Field | Value |
|---|---|
| `kind` | `agent` / `human` / `frontend` (default `human`) |
| `name` / `agentId` | the name written to the audit log |
| `streamOutput` | frontend only. `true` streams PTY output as `output` events |

When a connection drops, its lease, pending approvals and scheduled executions are all cleaned up.

## Methods

### Agent

| method | params | result |
|---|---|---|
| `affordances` | — | `["snapshot", ...]` what this connection may do right now |
| `snapshot` | — | `{revision,size,cursor,screen[],alternateScreen,controller,processAlive}` |
| `request_control` | `{reason?, command?}` | `{status: granted, leaseId, agentId, ttlSecs}` — with the gate on, the server waits for the human's decision before answering, or errors with `control_denied` |
| `release_control` | — | `{released}` |
| `type` | `{text}` | `{typed}` — no newlines |
| `send_key` | `{key, intent?}` | `{status: sent \| executed \| cancelled \| rejected \| denied \| pending, ...}` — ENTER requires `intent` (policy `require_intent`); missing → `intent_required` |
| `analyse` | `{cmd}` | `LineAnalysis { cwd, segments[{text, command, opaque, policy, label, targets[{path, exists, isDir, gitRepo, entries, protected}]}], policy, label, isolationViolation }` — verdict only, nothing runs |
| `interrupt` | — | `{interrupted}` |
| `check_approval` | `{approvalId}` | `{approvalId, state, cmd, label}` |
| `exec_state` | `{execId}` | `{execId, state: scheduled\|executed\|cancelled, reason}` |
| `proposal_state` | `{proposalId}` | `{proposalId, state: drafting\|ready\|executed\|rejected\|denied}` |
| `control_request_state` | `{requestId}` | `{requestId, state: pending\|granted\|denied\|expired}` |

In copilot mode `type` does not write to the shell; it accumulates in a proposal (ghost text), and `send_key(ENTER)` waits for the human to commit or reject before answering `executed` / `rejected` / `denied`.

`send_key(ENTER)` waits 60 ms for the echo to settle before evaluating policy. `rate_limited` never reaches the client: the server waits and retries. Likewise `scheduled` is held by the server until the grace ends and answered as `executed` / `cancelled`.

### Human / frontend

| method | params | result |
|---|---|---|
| `status` | — | controller, processAlive, revision, size, pending[], scheduled, sessionAllows, policyPath, connectedAgents, connectedFrontends, pacing, affordanceMask, promptActive, mode, effectiveMode, attended, entrustedTo, attentionRequest, openedBy, controlGate, controlRequests, proposal, lastAgent |
| `take` | — | `{revoked: "lease#N" \| null}` |
| `approve` | `{approvalId, decision: grant\|deny\|allow_session}` | `{approvalId, state, cmd, label}` — the id is required |
| `cancel_exec` | `{execId}` | `{cancelled}` |
| `execute_now` | `{execId}` | `{executed}` — the human co-signs a scheduled execution and runs it now |
| `set_mode` | `{mode: observe\|copilot\|autopilot}` | `{mode}` |
| `set_control_gate` | `{ask: bool}` | `{ask}` — when on, every `request_control` waits for the human |
| `decide_control` | `{requestId, grant: bool}` | `ControlRequest` |
| `accept_proposal` | `{proposalId}` | `KeyResult` — types the proposal into the shell and runs it after the policy check. `confirm` counts as approved because the human just read and committed it; `deny` still blocks |
| `reject_proposal` | `{proposalId}` | `{rejected}` |
| `hand_back` | — | `{leaseId, agentId, ttlSecs}` — returns the conn to the agent that last held it |
| `revoke_session_allow` | `{label}` | `{revoked}` |
| `input` | `{data: base64}` | `{written}` — the human input path (takeover) |
| `resize` | `{rows, cols}` | `{rows, cols}` |
| `get_pacing` | — | `Pacing` |
| `set_pacing` | a subset of `Pacing` | the merged `Pacing` |
| `set_affordances` | `{allow: [..] \| null}` | `{allow}` |

## Events

| event | recipients | fields |
|---|---|---|
| `tools_changed` | agent | — |
| `control_granted` | frontend | `lease, agentId, reason` |
| `control_revoked` | agent (holder), frontend | `lease, agentId, reason: human_input\|taken\|released\|expired\|disconnected\|process_exited` |
| `agent_input` | frontend | `agentId, len` |
| `approval_requested` | frontend | `request{id, agentId, cmd, label, intent, analysis, requestedAt, state}` |
| `approval_resolved` | agent (requester), frontend | `approvalId, state, by` |
| `exec_scheduled` | frontend | `execId, agentId, cmd, graceMs, intent` |
| `exec_cancelled` | frontend | `execId, reason` |
| `agent_exec` | frontend | `agentId, cmd, intent, policy: allow\|deny\|confirm:granted\|...` |
| `human_exec` | frontend | `cmd` |
| `screen_changed` | frontend | `revision` (coalesced per tick) |
| `output` | frontend (streamOutput) | `data` base64 |
| `process_exited` | frontend | `exitCode` |
| `pacing_changed` | frontend | `pacing` |
| `affordance_mask_changed` | frontend | `allow` |
| `mode_changed` | frontend | `mode` |
| `control_gate_changed` | frontend | `ask` |
| `control_requested` | frontend | `request{requestId, agentId, reason, state}` |
| `control_request_resolved` | agent (requester), frontend | `requestId, state` |
| `proposal_changed` | frontend | `proposal{proposalId, agentId, text, intent, state}` |
| `proposal_resolved` | agent (author), frontend | `proposalId, state, cmd, policy` |
| `control_handed_back` | agent (recipient), frontend | `lease, agentId, lastCmd` |
| `session_allows_changed` | frontend | `allows[]` |
| `attention_changed` | frontend, agents | `attended` |
| `entrusted` | frontend, agents | `agentId, cap` |
| `attention_requested` | frontend | `agentId, reason` |
| `control_suspended` / `control_resumed` | agent (holder) | `reason` / — |
| `tab_opened` | frontend, agents | `agentId, reason` |
| `agent_switched_tab` | frontend, agents (both sessions) | `agentId, from, to` |

## Error codes

`busy` `not_controller` `lease_expired` `process_exited` `invalid_input` `not_found` `approval_pending` `exec_pending` `proposal_pending` `intent_required` `rate_limited` `masked` `control_denied` `wrong_mode` `unattended` `suspended` `not_available` `unsupported` `io` `parse`

## In-process (Rust)

The same thing without a socket:

```rust
use conn_core::{Engine, EngineConfig, ServerEvent};

let engine = Engine::spawn(EngineConfig { rows: 40, cols: 120, render_prompt: false, ..Default::default() })?;
engine.subscribe("ui", Box::new(|ev: ServerEvent| { /* ServerEvent::Output { data } etc. */ }), true);
engine.write_input(b"ls\r");
let session = engine.session();
session.lock().set_pacing(Pacing { enter_grace_ms: 1500, ..Default::default() });
// open the socket as well so agents can attach
let hub = conn_core::ipc::Hub::single("t1", session);
let _guard = conn_core::ipc::serve_in_background(conn_core::paths::socket_path(), hub)?;
```

### Original control request

Agents should include `command` when the exact planned shell command is known. `conn agent run` sends it automatically; `conn agent request --reason R --command COMMAND` requests control without typing or executing that command. MCP `terminal_request_control` accepts the same optional field.

Control request events/status and the request/resolution audit records carry `originalRequest: {method: "request_control", params: {...}}`. This preserves submitted JSON parameter values, including whitespace inside strings, rather than transport framing or JSON whitespace. Resolution audit records are self-contained so denied/expired requests can be restored independently. Request parameters must be an object of at most 64 KiB; reason/command must be strings or null. A pending request retains its first payload; resubmitting a different non-null command is rejected.

The UI exposes this payload in a collapsed original-request section. Planned commands are metadata, not authorization or evidence of execution. Actual execution retains its own command record. Older records without a payload and clients without a planned command are shown as unavailable, never reconstructed from a reason.
