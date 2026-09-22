# Conn product backlog

English · [한국어](product-backlog.ko.md) · [Existing refactor plan](collaboration-refactor-plan.md)

Latest handoff: [2026-09-23 Mac fixes, Linux regressions and remaining work](shared-runtime-follow-up.md).
Keep the dated records below separate from the installed public release's status.

## 2026-09-21 — Feedback from real remote-shell collaboration

Status: essential collaboration scope implemented and verified in the shared harness for v0.8.4.
See [scope and remaining validation](collaboration-context-results.md). Priorities do not imply release
commitments or completion of every acceptance criterion.
The product goal is now **500+ GitHub stars**. Prioritize first collaboration success, clear work
context, repeat use and trust over feature count. This document does not authorize publication or promotion.

### Core value and evidence

> A person can follow where an agent works in a real terminal and intervene in that same work context.

| Source | Report | Evidence boundary |
|---|---|---|
| F01 · Copilot session report | Screen read → control request → command approval → read-only command → result verification → release worked in a real SSH shell | Agent report relayed by the user, not independently rerun while writing this backlog |
| F02 · Comparative usability feedback | Connected requests, approvals and results felt convenient for repeated deployment/operations; the agent preferred it to iTerm2 MCP for that work and described the steps as simple | Not a controlled comparison or independent user study |
| F03 · Direct user observation | Movement between shells logged into different accounts, with a movement notice; visible tracking of work in real remote shells was valuable | Evidence of shell navigation, not exhaustive account/permission isolation testing |
| F04 · Follow-up report | Intent review, human takeover and approval flow inspired confidence; reconnecting, leases, repeated execution and long output caused friction | Perceived trust is not a safety guarantee or proof that bypass is impossible |

The earlier human correction of a working directory is documented in the [collaboration story](collaboration-story.md).
Do not retain real addresses, accounts, command payloads or authentication responses here. Reproduce with synthetic data.

## Priorities and dependencies

| ID | Priority | Work | Source | First dependency |
|---|---|---|---|---|
| UX-01 | P1 | Long paths, wrapping and response rendering | F04 | Separate display defects from data corruption |
| UX-02 | P1 | Tab, host, account and navigation identity | F03, F04 | Reliable metadata sources |
| UX-03 | P1 | Connection, approval and lease state guidance | F01, F04 | Separate states and who is waiting |
| UX-04 | P1 | Sessionless attention and contextual notifications | F01, F03 | UX-02, UX-03 |
| UX-05 | P1 | Rediscover an existing shell after reconnecting | F04 | UX-02, session-liveness identity |
| UX-08 | P1 | Preserve context across human and agent tabs | Additional user report and real harness reproduction | UX-02/03/04 |
| UX-06 | P2 | Discoverability and flow of Observe | F04 | Audit existing mode/participation behavior |
| UX-07 | P2 · design | Reviewable command batches | F04 | Failure, cancellation and approval boundaries |
| SEC-01 | design investigation | Cookie/token masking | F04 | Shared screen, recording and input boundaries |
| PROD-01 | P2 · validation | New-user collaboration, repeat use and demo | F01–F04 | Main P1 flows |

### UX-01 · Display fidelity for long output

- 2026-09-23: The unreleased common-runtime branch fixes the Mac long-input deadlock and validates
  native local/loopback SSH input and fixed-window approval panels. Linux regressions also pass.
  See the [handoff summary](shared-runtime-follow-up.md); the user's gateway and physical IME remain unverified.
- 2026-09-22: **still open after v0.8.5**. The user reports constrained width/height and misplaced input
  after updating/restarting, even with a fixed window when notification/approval bars animate. Resize
  and output ordering defects were reproduced and fixed in v0.8.6; the exact installed native/remote
  environment remains unverified. See [dimension follow-up](ssh-input-review-results.md#terminal-dimensions-after-v085).
- Problem: long paths and OAuth responses reportedly look broken when wrapped.
- Scope: compare actual output, terminal grid, rendering and agent snapshots to locate the defect.
- Acceptance: synthetic long paths, URLs, token-shaped strings, Korean/Unicode, paste, resize and
  local/SSH output exercise character loss, duplication and ordering. Distinguish intentional soft
  wrapping from corruption. Do not collect hidden input or weaken visibility rules as a fix.

### UX-02 · Identify the target shell

- Problem: navigation feels natural, but different accounts/hosts need clearer target identity.
- Scope: share target presentation across tab titles, movement notices and request cards.
- Acceptance: requests and results remain distinguishable across two accounts/hosts. Display only
  established host/account metadata; represent unknown values without guessing. Follow stable session
  identity across renames, exit and reconnection. Keep the shared UI visually simple.

### UX-03 · Explain waiting and lease state

- 2026-09-23: The [control experience audit and proposal](control-experience.md)
  defines scope clarity, consistent pending requests and validation under the
  [product invariant](PRD.md). The direction uses a Conn-badge morph for control
  requests and retains bottom command review. This records design, not completed UI changes.
- Problem: admission, control approval, command review and lease expiry create friction or ambiguity.
- Scope: consistently explain the reason, waiting actor, next action and lease expiration.
- Acceptance: cover renewal, expiry, denial, cancellation and human takeover. Remaining time agrees
  with server state; stale approvals never reappear. Show the needed action without a noisy permanent
  countdown. Default TTL changes and read-triggered renewal require separate policy decisions.

### UX-04 · Preparation requests and notifications

- Problem: without an accessible tab, the agent cannot easily ask the human to prepare one; requests
  in other shells can be missed.
- Scope: inspect and extend existing attention support for a sessionless request. Use admission before
  connection approval, and a terminal-selection request after admission when no shell is accessible.
- Acceptance: cover no tabs, all-private tabs, another selected tab and an inactive app. Offer concise
  actions equivalent to `Choose terminal / New terminal / Later`. Clicking navigates to the target;
  it never automatically shares, approves or grants control. Keep movement notices quiet; make required
  human action more prominent. Distinguish established completion/blockage from agent-reported status;
  delivering input does not establish command success. Coalesce repeated requests and clear resolved
  or cancelled ones. System notifications omit command/output/authentication payloads by default.
  Do not forcibly activate a window or steal keyboard focus.

### UX-05 · Rediscover and recover existing shells

- Problem: finding tabs/sessions again after a disconnection is cumbersome.
- Scope: distinguish agent transport loss, SSH disconnection and shell exit; rediscover surviving sessions.
- Acceptance: a surviving session retains account/process state and can be selected again. A newly
  created shell is not presented as the old session restored. New connection IDs do not automatically
  inherit sharing selection, control or pending approval. The human handles reauthentication;
  credentials are not stored or reinjected.

### UX-06 · Observe flow

- Problem: read-only observation was requested, although Observe already exists.
- Scope: determine whether the issue is discoverability or an access restriction before adding a feature.
- Acceptance: users can read an explicitly shared shell and understand Observe's limits. Observation
  does not grant access to private shells or input/control authority. Improve existing settings/guidance
  where needed rather than duplicating the mode.

### UX-08 · Context across multiple tabs

- Reproduced: while the person stayed in tab 1 and the agent opened/worked in tabs 2 and 3,
  request cards appeared only in the target tab. Clicking a notice opened the person's current
  tab panel instead. Background-to-background moves were easy to miss.
- Scope: persistently show pending requests elsewhere and live connections' most recently used
  terminal; address notices by stable session/request IDs. Recover requests during tab startup.
- Acceptance: preserve the person's selected tab until they choose to navigate. Notices lead to
  the correct shell/request; closed tabs, resolved requests and same-name new connections remain
  distinct. Navigation grants no sharing, decision or control. Explain that typing reclaims only
  the terminal receiving that input.

### UX-07 · Command batches

- Problem: repeated single-line typing and execution interrupt routine work.
- Scope: first design a bounded batch whose complete execution content the human can review.
- Acceptance: define per-command policy checks, approval scope, stop/continue on error and partial
  completion. Verify cancellation of remaining commands on human input, takeover, stop-sharing or
  disconnection. Authentication/TUI interaction is not a command batch. An agent calling a command
  safe or read-only does not bypass policy. Check overlap with existing compound-command support.

### SEC-01 · Investigate sensitive-content masking

- Problem: concern about visible or recorded curl Cookies/tokens, with a request for automatic masking.
- Scope: design optional shared-screen masking, minimal notifications and recording boundaries;
  implementation mechanism remains undecided.
- Acceptance: preserve the common human/agent screen model and review snapshot, timeline, notification,
  diagnostic and clipboard paths. Use synthetic wrapped/chunked/ANSI output, repeated values and
  false-positive/negative cases. Explain that display masking does not remove shell history, process
  arguments or remote logs. Never collect hidden/masked input or claim regex detection is complete.

### PROD-01 · Reproduce the value and validate with new users

- Scope: build a demo/onboarding scenario where the human authenticates, the agent observes, the human
  corrects the target shell or direction, then approval, execution and control return follow.
- Acceptance: EN/KO materials distinguish observed behavior from reenactment. With participants'
  consent, assess first collaboration success, intervention success, friction and reuse on a later task.
  These are usability outcomes, not permission to automatically collect commands, output or credentials.
  Do not establish superiority, safety or ease of use from a single agent's testimonial.

## Sequence and tracking

1. Reproduce UX-01; align shared state/presentation for UX-02/03.
2. Use UX-04/05 to remove first-connection and reconnection friction.
3. Audit UX-06; design UX-07/SEC-01 separately. Use PROD-01 to gather repeat-use evidence.

When work starts, link changes/issues, reproduction environments and observations to each ID.
Track `not started → design → implementation → validation → done`; keep pending native acceptance
explicit. Earlier TUI input issues and native acceptance remain separate follow-ups in the
[existing refactor plan](collaboration-refactor-plan.md).
