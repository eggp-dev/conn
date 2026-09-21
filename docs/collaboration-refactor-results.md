# Collaboration boundaries — implementation and validation

[한국어](collaboration-refactor-results.ko.md) · [Plan](collaboration-refactor-plan.md)

2026-09-21 · Implementation based on `263f255`, in the working tree.
**S0–S3 implemented; S4 local acceptance passed, native platform acceptance remains open.**
This report records implementation validation before the 0.8.3 version bump. It is not a release certification. See the [0.8.3 changelog](../CHANGELOG.md) for the subsequent release scope and remaining native verification. Signing credentials stay in release CI.

## Responsibility boundaries

| Owner | Implementation | Responsibility |
| --- | --- | --- |
| Connection registry | `crates/core/src/connections.rs` | Admission policy, live connection IDs, lifetime, revisioned owner events/snapshots |
| Collaboration coordinator | `crates/core/src/collaboration.rs` | Session catalog; common cross-session control requests, owner grants, hand-back and navigation |
| Protocol / agent service | `protocol.rs`, `agent_commands.rs` in `conn-core` | Request/result contracts and agent operations; IPC handles transport and waiting |
| Participation | `crates/core/src/session/participation.rs` | Audience and generations inside the existing Session lock |
| Pending work | `crates/core/src/session/pending_work.rs` | Scoped cancellation of reviews, proposals, grace and control requests |
| Owner transition | `crates/frontend/src/surface_commands.rs`, `automation.rs` | Owner authorization, audit preparation, external-writer fencing and participation commit |
| UI state | `frontends/tauri/src/lib/collaboration/` | Typed contracts, admission reducer, session snapshot application, stale-read reconciliation and decision progress |
| Presentation | `DecisionCard.svelte` and existing interaction dock | Shared card layout, action slots, errors, busy state and reduced-motion handling |

No additional crate, per-module mutex, or plugin runtime was introduced. `ipc` re-exports existing
public names for embedders. Existing AppleScript terminology, event codes, D-Bus operations and
external return shapes are unchanged. The existing settings format is retained.

### Admission is independent of sharing

Pending connections can be answered in private, empty and preparing views, and inside Settings.
Agents setup and app-wide admission settings remain available on private tabs; per-session policy
controls remain scoped to shared tabs. Admitting a connection does not reveal a private session,
select it as a participant, or grant input authority.

Admission snapshots/events use a monotonically increasing revision. The UI reconciles missed events
and rejects stale per-connection updates. A failed decision remains actionable. Confirmed decisions
and authoritative reconciliation remove requests. Same-name reconnects retain distinct connection IDs.

### Sharing commits after successful preflight

The owner service validates selected live IDs and prepares the audit resource before mutation.
The external writer's existing operation lock fences delivery; Session preflight runs under its lock.
Only a successful commit revokes the external handle and drains queued payloads. A failed input or
history precondition preserves private state, generations and external input. No lease or PTY is
replaced to change the audience.

Lock order is external operation → Session for sharing. Control transitions use the coordinator lock,
a cloned session catalog and the existing session locks, held only for the synchronous transition.
The catalog lock is released before session locks; no approval wait runs under these locks. Registry
listeners run after releasing the registry lock and must not acquire Session locks.

### Cancellation semantics

| Trigger | Result |
| --- | --- |
| Explicit human takeover | Cancel reviews, proposals, grace and unanswered control requests, even without a held lease |
| Agent release or move | Cancel that connection's pending work and release its lease |
| Connection close | Cancel only that connection's work; another connection with the same name is independent |
| Stop/change sharing | Preserve existing whole-session cancellation behavior; advance participation generation |
| Process exit / switch to Observe | End pending work as well as authority |
| Human typing | Preserve existing preemptive input behavior and raw-input privacy boundary |
| Review wait / lease timeout | Preserve the existing review-wait renewal policy and expiry rules |
| Window blur, background or minimization | No added access restriction |

The intentional changes are cleanup of reviews/proposals after explicit take/release, and cleanup of
control requests after take/process exit. A resolved or cancelled review cannot later deliver ENTER.
The existing TUI `input_pending` guard is preserved; its heuristic is not redesigned here.

### UI state and reusable decisions

Admission, control and command-review cards use one presentation component while retaining distinct
commands and labels. Keyboard review responses and card buttons share per-request busy/error state.
Targets are captured by session/request ID, so an awaited response cannot follow a changed active tab.
Session reads are coalesced; reads racing events retry before applying. Full polling repairs missed
state, including grace state. Sharing errors survive participant refreshes. English and Korean use
i18n entries. Toasts sit above the interaction dock instead of covering its controls.

### Sharing concurrency and UI state

The owner-only `sharing_state` command returns participants and their participation revision together.
The UI sends that revision as `expectedRevision` when applying a selection. A stale request returns
`sharing_changed` before revoking external input; the user reloads the current selection and applies
explicitly. Live connection IDs are checked again at commit. Existing `sharing_participants` responses
and owner calls without a revision remain compatible.

Session events now have a scoped discriminated union. Event and snapshot projection share the same
participation/request helpers, and command-palette decisions use the same actions as cards and keyboard
review. Recovered grace countdowns use the same monotonic clock as live events and retain their start
across polling.

## Validation evidence

| Layer | Result |
| --- | --- |
| `cargo test --locked` | 246 passed; the external SSH-lab test is ignored in this default command |
| Real SSH lab test, explicitly enabled | Passed: wrong-password retry, successful authentication, same-session sharing, hidden credential exclusion and no private-history backfill |
| Frontend unit tests | 46 passed, including 12 new admission/reconciliation cases |
| Frontend production build | Passed; 17 existing Svelte warnings remain |
| Documentation site build | Passed; 29 pages generated |
| Release-tool unit tests / version check | 78 passed; version declarations still agree on 0.8.2 |
| Production App + contract fixture | Passed: failure/retry, persistent sharing errors, stale-selection reload, private/empty/preparing/Settings admission, Korean narrow layout, keyboard and reduced-motion checks |
| Production App + real Rust Harness + persistent agent | Passed: stop sharing, private admission, explicit participation, control grant, separate command review, actual command output and sharing stop |

The real Harness test sets a shell variable before sharing changes, then requires its standalone
output after agent execution. Merely seeing a typed command or a prior terminal line does not pass.
Tests use disposable sockets/configuration; the SSH fixture uses synthetic credentials. The test
container was returned to its prior stopped state. Local screenshots live under
`artifacts/collaboration-refactor/` and are not part of the public source inventory.

### Repeat the portable checks

```sh
cargo test --locked
cargo build --locked -p conn-browser-harness
cd frontends/tauri
npm test
npm run build
npm run test:collaboration
npm run test:collaboration:harness
```

Browser checks require `agent-browser` 0.24.1 and its browser installation. Their CI job installs this
pinned tooling outside the repository. Both browser flows are included in Required checks; remote CI
has not been run from this unpushed working tree.

## Remaining native acceptance

- **Linux native GUI / D-Bus:** compilation stopped because `gdk-3.0` development metadata is absent,
  also confirmed outside the filesystem sandbox. Install the desktop dependencies used by
  `.github/actions/setup/action.yml`, then build and run the isolated native adapter tests.
- **macOS Apple Silicon:** the prior SSH endpoint failed host-key verification. No trust override was
  made. On a verified Mac, build this exact revision and validate cold/warm AppleScript launch,
  hidden/masked login, connection approval in the private window, sharing of that same session,
  background observation/control, takeover and stop. Check the scripting dictionary on the built
  app, not another installed copy. Signed installer/updater acceptance is still required for release.
- **Windows:** no native runtime is available here. Compile/test on the existing Windows CI runner,
  then exercise MCP admission, sharing, takeover and close/reconnect in the native app.

Browser and core passes do not substitute for these native checks. Packaging, signing, installation
and update delivery have not been exercised with this working-tree revision.
