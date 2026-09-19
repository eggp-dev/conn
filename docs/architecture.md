# Architecture

**Shared-surface hard cut · v0.7.0 preview.** [Product contract](PRD.md) · [한국어](architecture.ko.md)

```mermaid
flowchart LR
  H[Human] --> U[Native owner UI]
  U -->|input and decisions| S[Session authority and policy]
  S --> P[PTY and child]
  P -->|sequenced output| U
  P -->|PTY output| F[Core terminal grid]
  A[External MCP actor] -->|observation and control request| S
  S --> F
  F -->|authorized grid| A
  F --> E[Extension host]
  E --> M[Native model adapter]
  K[OS credential store] --> M
  M -->|proposal| U
```

## Ports and owners

- `conn-core::Engine` manages the PTY and lifetime. Session serializes authority,
  mode, policy, proposals, grace and sharing transitions under one lock.
- The native owner supplies a sequenced output callback before PTY reading begins.
  Output buffering preserves startup rendering; callbacks never relock Session.
- Tauri and the token/Origin-protected browser harness use `conn-frontend::Harness`.
  Each command carries the adapter-established owning window, never a public
  caller's claimed window identity.
- The Svelte/xterm renderer displays output; it does not authorize observation.
  Core snapshots exclude scrollback and suppressed cells.
- Public IPC is an agent endpoint. MCP does not decide permission; core checks it
  for every request. The native owner bridge is not available through IPC.

## Frame identity and freshness

A grid revision identifies `surfaceId`, `generation`, `revision`, `outputSeq`,
size, visible text, nullable cursor and alternate-screen state. The core parses PTY
output with a bounded, zero-scrollback terminal model. Snapshots are text-only.

Sharing changes advance the generation; output, cursor and resize changes update the
screen revision. Completion acceptance checks this same identity. No foreground,
heartbeat or owner-renderer publication participates in authorization.

## Transitions

| Transition | Required effects |
| --- | --- |
| Human input | Revoke conflicting authority; cancel stale proposals; deliver human input |
| Start sharing | End external writer and queue; select actual connections; invalidate old frame; human retains control |
| Stop sharing | Cancel agent work/observations; invalidate frame; keep PTY/process |
| Hide/blur/cover/tab switch | UI state only; shared-session access and leases remain unchanged |
| PTY output | Update bounded core terminal grid and revision |
| Disconnect | Remove connection identity; revoke its authority; never transfer selection by name |
| Close | Cancel jobs, writer and sessions owned by that window |

Origin and participation are independent. An external-origin session has no
startup activity audit or execution hooks. Sharing can start new collaboration
history under the same session ID; it never reconstructs prior private activity.

## Extension host

A typed manifest registry owns API compatibility, declared capabilities and
reviewed implementation selection. The first extension kinds are theme, provider
and completion. Themes are bounded data; arbitrary code and global event buses are
not extension contracts.

Native mediation constructs visible context from the core frame while its
cancellation is registered in the same session transaction. Workers obtain a key
from the OS store and make bounded, cancellable requests. Results carry the session,
surface, generation and frame identity. Acceptance checks them again and uses the
normal human-input path, without Enter. Theme and provider configuration persist;
keys, private frames and completion jobs do not enter configuration files.

See [extension contract](extensions.md), [protocol](protocol.md) and [trust model](security.md).
