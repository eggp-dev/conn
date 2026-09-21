# Collaboration context — implementation and validation

[한국어](collaboration-context-results.ko.md) · [Product backlog](product-backlog.md)

2026-09-21. **v0.8.4 implementation evidence** after the v0.8.3 source. The product criterion is that a person
can follow and intervene in an agent's work in the same real shell. Build results, automated checks,
the shared browser harness and native app acceptance are separate evidence levels.

## Backlog classification and scope

| Category | Decision |
|---|---|
| Already present | Observe, shared background observation, tab discovery/navigation, per-session leases, human takeover, session attention and executed-command history. Improve discoverability and the connected flow. |
| Essential improvements | UX-08 multi-tab context, UX-02/03 target/state guidance, UX-04 sessionless preparation, UX-05 rediscovery and UX-01 display fidelity. Include Observe navigation/tool guidance. |
| Incompatible directions | Focus/selected-tab access gates, hidden parallel execution shells, forcibly following every agent move, approving/sharing/granting on notification click, inheriting permissions by display name, or collecting hidden input. |
| Separate design/validation | UX-07 reviewable batches, SEC-01 masking shared by people and agents, TTL/renewal policy and PROD-01 first-use/reuse studies. These need separate work, rather than rejection as product ideas. |

## Implemented behavior

- **Keep other tabs visible:** pending requests elsewhere and each live connection's recently used
  terminal remain available in the bottom dock. Agent tab creation/navigation preserves the person's
  selection. Notices re-read state and navigate by session ID; resolved request IDs get a status
  message. State reconciliation recovers requests racing new-tab registration.
- **Ask for a terminal:** after admission, an agent with no available live shell can use the existing
  attention tool for preparation. The person chooses **Choose terminal / New terminal / Later**.
  New terminals start private and reuse the existing participant-selection dialog. One request per
  connection coalesces duplicates, with a ten-second cooldown after dismissal. Cancellation, disconnect
  or an available shared shell resolves it. Before admission, the normal connection decision applies.
- **Consistent guidance:** Observe permits navigation to authorized shared tabs and attention, while
  continuing to deny control/input. The control panel displays approximate lease expiry from server
  state and distinguishes connection loss from shell exit. Human takeover applies to the receiving
  terminal. Pending requests update the title count and native taskbar/Dock attention without command
  or output payloads. Focus affects notification delivery only. System-notification actions selecting
  individual requests were not added.
- **Rediscovery:** tab listings expose stable session ID, profile label and process liveness. Profile
  labels are not inferred remote host/account identities. Tool guidance directs reconnecting clients
  to choose the existing session and read a fresh screen, without replaying the last operation.
  A fresh connection needs fresh admission and inherits no selected sharing, control or pending work
  by name.
- **Output fidelity:** reproduced UI soft-wrap reflow while the core truncated columns on narrowing.
  Core normal-buffer resize now preserves cells, attributes and wide-character pairs, using at most
  5,000 internal history rows like the UI; snapshots expose only the current grid. Alternate screens
  resize without reflow. Wide characters retain their soft-wrap relationship, and the official
  Unicode 11 addon corrects the tested emoji width mismatch in the UI.

`ConnectionRegistry` owns live metadata and preparation. `Hub` checks available sessions. Existing
`Session` owns participation, leases and command decisions. The UI reuses sharing and decision components.

## Validation and limits

| Check | Result |
|---|---|
| Rust core/frontend/CLI/browser-harness | 254 passed, 1 existing ignored test. Real sockets/PTYS, admission, preparation identity/coalescing/cancellation, Observe/masks, private new shells and existing regressions |
| Frontend state checks | 48 passed, including request identity/replacement, stale replies, lease display and existing input/timeline checks |
| Shared UI + real Rust + local PTYs | 12 stages passed: new/background tabs, targeted notices, human input in another tab, stale requests, retained shell variable after reconnect, Observe, sessionless preparation/private creation/explicit sharing, same-name noninheritance, EN/KO and narrow viewport |
| Screen comparisons | Rendered rows match snapshots before narrowing, after narrowing and after restoring long paths/URLs/Korean output. 85 core/xterm grid-and-cursor comparisons passed |
| Frontend build | Passed, with existing Svelte warnings and bundle-size warning |
| Native build/acceptance | Tauri check blocked by missing `gdk-3.0` development library. Taskbar/Dock delivery, minimized/inactive/multiple windows and Mac/Windows acceptance remain unverified |
| SSH/product validation | Real SSH reconnect/reauthentication, exhaustive editor/Unicode behavior, and external first-use/reuse validation remain |
| Delivery | The [v0.8.4 release page](https://github.com/eggp-dev/conn/releases/tag/v0.8.4) records publication and signed artifacts; those checks are separate from the local evidence above |

This does not complete the backlog's native/SSH acceptance criteria. See [reproduction commands](browser-testing.md).
Synthetic screenshots, video, states and grid comparisons remain in the temporary directory printed by each run,
outside the repository.
