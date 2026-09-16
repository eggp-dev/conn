# Architecture

> v0.2: the core lives in `crates/core` (library `conn-core`), the binary in `crates/cli`, the reference frontend in `frontends/tauri`. The big picture is in [PRD.md](PRD.md) §4; the socket contract in [protocol.md](protocol.md).

## Three deployments

```
1. terminal mode   Terminal.app ─ stdin/stdout ─ conn(proxy) ─ Engine
2. embedded        Tauri ─ Engine::write_input / subscribe ─ Engine          (+ ipc::serve_in_background for the agent socket)
3. headless        any frontend ─ UDS (input/output streaming) ─ conn serve ─ Engine
```

In all three the same `Session` upholds the same invariants.

## Terminal mode (v0.1)

```
        Terminal.app  (does the rendering)
              │ stdin (human keyboard)
              ▼
      ┌───────────────────────┐
      │         conn          │
      │  session/  ── core    │
      │   authority  policy   │
      │   approval   audit    │
      │   screen     input    │
      └───────┬───────────────┘
              │ PTY master
              ▼
            zsh ──> ssh ──> ...
              │ stdout → Terminal.app (passed through untouched)

  conn ◀─ UDS ─ conn mcp   ◀─ MCP stdio ─ Copilot CLI
       ◀─ UDS ─ conn status/take/approve
```

## Two doors

Humans come in through **stdin**, agents through a **Unix domain socket**. Because the origin is structurally distinct:

1. audit attribution (human vs agent) is exact at the byte level, and
2. the invariant "human input revokes the agent's lease" is enforced in one place.

## Modules

| Module | Role | Depends on |
|---|---|---|
| `session` | the core: all state and invariants, frontend events, pacing, mask | authority, policy, approval, audit, screen, input, config |
| `engine` | PTY creation, reader / tick / child-wait threads, the embedder handle | session |
| `config` | `Pacing` | — |
| `authority` | lease grant / revoke / expiry. `Controller::{Human, Agent(Lease)}` | — |
| `policy` | YAML loading, `deny → confirm → default` evaluation, session allows, reload | regex |
| `analysis` | line splitting, tokenising, wrapper peeling, opaque constructs, `cd` simulation, target resolution | — |
| `approval` | pending queue, TTL, approval prompt rendering | — |
| `audit` | append-only JSONL | — |
| `screen` | headless VT model (`vt100`), used only for the agent's projection | — |
| `input` | input-line tracking with a VT fallback | — |
| `affordance` | `affordances_for(actor, state)`: the single permission API shared by MCP and CLI | authority |
| `ipc` | UDS server (tokio) + synchronous client; maps methods to `Session` calls; the `Hub` of sessions | session |
| `proxy` (cli) | raw mode, stdin thread, SIGWINCH, shutdown — a thin layer over Engine | engine, ipc |
| `serve` (cli) | headless: Engine + socket only | engine, ipc |
| `mcp` | JSON-RPC stdio ↔ UDS bridge. No permission logic | ipc |
| `cli` | status / take / approve / log … | ipc, audit |

Boundaries:

```
session  ≠  mcp / cli / ipc      (the core knows nothing of sockets or MCP; tests/session.rs runs on the core alone)
authority ≠ approval UI          (an approval prompt on screen has no bearing on lease state)
screen   ≠ human output          (the VT model is never shown to the human)
```

## Frontend events and controls

`Session` distinguishes connections as `Agent / Human / Frontend`. A frontend connection (or `Engine::subscribe`) receives every lifecycle event and, on request, the PTY output as base64 `Output` events. A frontend may change only `Pacing` and the affordance mask, and both only ever **narrow** the agent.

`enterGraceMs` holds an ENTER judged allow as a `ScheduledExec` and runs it from the tick (50 ms). Human input, `take`, `cancel_exec`, interrupt, loss of the lease and a mask change all cancel it. The socket layer (`call_with_pacing`) waits until the grace ends and returns the final result, so the agent-side protocol is the same as v0.1.

## Concurrency

`Session` is protected by a single `Arc<parking_lot::Mutex<Session>>`.

- stdin reader thread: `read(0)` → `human_input()`
- PTY reader thread: `read(master)` → `pty_output()` (writes stdout + feeds the VT model)
- tick thread (50 ms): lease/approval expiry, grace execution, coalesced screen_changed, policy reload
- tokio: UDS connections (and SIGWINCH/SIGHUP/SIGTERM in proxy mode)
- child-wait thread: child exit → cleanup

Every write decision is taken under the lock, so the "revoke → PTY write" order inside `human_input` never interleaves with an agent write. `tests/session.rs::preemptive_takeover_order` checks that order directly.

## Preemptive takeover

```
human_input(bytes):
  1. authority.revoke()          lease revoked, controller = Human
  2. pty.write(bytes)
  3. control_revoked event to the agent
  4. audit takeover
```

1 precedes 2. Keys pressed while an approval prompt is up are consumed as the answer and never reach the PTY, so they are not a takeover.

## When policy is checked

`terminal_type` is not checked. `terminal_send_key(ENTER)` treats the accumulated input line as the command and checks it.

The line is reconstructed by `input::InputTracker` from the bytes written to the PTY (human and agent alike). Input that makes the shell redraw the line — tab completion, history (↑/↓), cursor movement — marks it `dirty`, and at ENTER the value is taken from the VT model's cursor row with the prompt prefix (the cursor column at the first keystroke) removed. Clean tracked input remains authoritative even when the terminal wraps it. On the agent path, dirty input gives the echo 60 ms to settle before using the VT fallback. This fallback covers only the cursor row and can be incomplete for wrapped edited commands; see [the trust model](security.md).

Per verdict:

| Verdict | PTY | Response |
|---|---|---|
| allow | `\r` | `executed` |
| deny | `Ctrl-U` (clear the line) | `denied` |
| confirm | nothing; the prompt is shown | `pending { approvalId }` |

Approval sends `\r`; denial, expiry, disconnect and interrupt send `Ctrl-U`. While an approval is pending, `type`/`send_key` from the same agent are refused so the approved command and the executed command cannot diverge.

## Approval prompt

Drawn directly on stdout in the alternate screen. PTY output produced while the prompt is up is buffered and released afterwards. `conn approve <id>` can decide it too.

## IPC protocol

Newline-delimited JSON.

```
→ {"id":1,"method":"hello","params":{"kind":"agent","agentId":"copilot"}}
← {"id":1,"result":{"conn":2,"kind":"agent"}}
→ {"id":2,"method":"send_key","params":{"key":"ENTER","intent":"list files"}}
← {"id":2,"result":{"status":"pending","approvalId":"apr-1","cmd":"...","label":"..."}}
← {"event":"control_revoked","lease":"lease#1","reason":"human_input"}
```

Methods: `hello affordances snapshot request_control release_control type send_key interrupt check_approval status take approve …` — the full list is in [protocol.md](protocol.md).
When a connection drops, the lease it held and its pending approvals are cleaned up immediately.

## Session lifetime

`conn` ends when the child shell ends. It removes the socket file and restores termios. There is no detach/reattach.

## Client integration adapters

The shared frontend owns local MCP/skill setup through a client registry, lossless
config editors and a guarded file transaction layer. Terminal and permission
semantics remain in the core. See [the adapter contract](agent-integrations.md#adapter-contract).

## External automation (unreleased)

The [external automation contract](external-automation.md) ([한국어](external-automation.ko.md))
describes the **UNRELEASED replacement** for the recorded automation path in
v0.5.1. The macOS AppleScript adapter binds a native caller to its own private
session. A startup program replaces the allowed local profile's executable and
argv; later input follows a dedicated external writer, not agent control or policy.

Private origin is established before spawn. The existing native window and PTY
renderer are reused, while private input/output is excluded from public IPC/MCP
and Conn activity recording. Settings keep permissions only; request polling keeps
bounded, volatile metadata with generic error codes. Human input, cancellation or
revocation invalidates the external writer. Upgrades require one explicit
re-enable of old permissions. Native Apple Event and external-launcher acceptance
checks remain release gates.

Sharing an external private session with an AI agent, Windows/Linux external
adapters, and replacement of ordinary-session human-input history are later work,
not features of this increment. See [the design](automation-design.md) for those
separate transitions and [the trust model](security.md) for limits.
