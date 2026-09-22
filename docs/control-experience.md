# Conn control experience — principles and direction

[한국어](control-experience.ko.md) · [Product contract](PRD.md) · [Motion rules](UX-MOTION.md)

Status: **implemented in the 0.8.7 release candidate** · 2026-09-23.
[Validation and native acceptance limits](release-0.8.7-validation.md). Control expands at the top right with a 320 ms surface morph; command review stays in the bottom dock. Admission reserves space above the terminal.

## Product invariant

Conn's value is the UX and affordances that let people understand and manage an
agent working in their shared, real PTY.

> People must understand what an agent can do now and what their next choice
> changes without memorizing the internal permission model. Repeated requests
> must not train people to approve without thinking.

Consistency means **the same meaning has a predictable location, motion and action**.
Making every request a similar card with an Allow button does not meet this rule.
Admission, participation, control and command review remain separate responsibilities
within one understandable collaboration flow.

## Direction selected with the user

Preserve the dark terminal, agent colors, perimeter effects, badge and space-opening
motion. **Control requests expand from the existing top-right Conn badge. Command
execution reviews keep the existing bottom dock.**

Reference: [Motion Primitives — Morphing Dialog](https://motion-primitives.com/docs/morphing-dialog).
Its documentation and opening/closing example were inspected in the browser on
2026-09-23. The reference is continuity of position, size and corners between a
small trigger and expanded content. Literal liquid shapes or connecting tendrils
are not the direction. Background blur, modality and focus changes are not adopted
automatically from the example.

### Control: the badge becomes the request

1. A new request creates a pending state in the top-right badge.
2. The same visual element expands into a readable upper or central panel.
3. Show the agent, target shell, purpose and scope. Ask whether to hand over input
   in this shell, rather than making people interpret an internal permission name.
4. Use explicit actions such as `Hand over input / Deny`. Describe the actual mode
   and policy; do not promise automatic ordinary commands in every environment.
5. Collapsing returns to the badge and leaves the request pending. It is neither
   approval nor denial. Granting updates the controller display and does not approve a command.

New requests expand automatically at the top right, without moving terminal geometry.
Do not steal focus or turn a key typed into the terminal into an approval shortcut.
After explicit entry into the panel, specify keyboard navigation, dismissal and focus return.

### Commands: review execution below the terminal

- Keep the current bottom dock and reserved space.
- Lead with command execution review, the concrete effect and why review is required.
  The command, target and scope must be understandable without expanding details.
- Default actions are `Run once / Deny`. Any broader grant is a separate choice
  with explicit targets, policy scope and termination conditions, not an ambiguous session label.
- Briefly show existing permission when helpful, such as control already granted.
  Do not turn each request into a mandatory connection-to-execution wizard.
- Details supplement a decision; raw JSON must not hide its consequences or scope.

### Notifications: locate the decision

- Cross-tab and system notifications identify the shell and kind of decision.
  Opening them never grants permission, shares a session or takes control.
- Notification, badge and panel follow one request identity without presenting it
  as multiple unrelated requests.
- Admission means access to shared screens; control means input in a specific
  shell; command approval means this execution. Admission uses a separate upper dock with reserved terminal space.
- Keep routine progress quiet. Avoid perpetual pulsing or repeated expansion.
- Distinguish resolution, denial, cancellation and expiry; stale notices cannot revive work.

## Flow observed before implementation

The in-app browser used the common UI with the real Rust web host and MCP, a separate
state directory and synthetic fixtures. The user's working shell and files were untouched.

| Step | Observation | Assessment |
| --- | --- | --- |
| 1. Idle shell | Minimal terminal and Conn badge | Preserve |
| 2. Admission | Shared-screen scope, Allow / Deny | Same action wording as control |
| 3. Control and details | Bottom card, Allow, raw request/JSON disclosure | Internal structure dominates scope distinction |
| 4. Ordinary command | Synthetic file read runs after control grant | Valid observed path, not every environment |
| 5. Deletion review | Target, command, single and session grants | Broad grant scope unclear in default label |
| 6. Other tab and return | Pending link navigates to correct shell; approval remains pending | Preserve |
| 7. Denial and takeover | Synthetic deletion denied; control reclaimed through control center | Working affordance inside another panel |

These observations do not validate the proposed morphing interaction, native macOS,
screen readers, contrast ratios, small windows, zoom or reduced motion.

## State contract before implementation

- Reason, target, scope and available actions belong to one coherent pending request.
  Delayed status replies must not change the same card's actions or rationale.
- Explain genuine condition changes and reject stale decisions. A stable UI must
  not bypass the backend's current authorization checks.
- Separate command risk from unconfirmed shell state. Recovery must not bypass
  command review or expand permissions.
- Distinguish reclaiming input from terminating a process already running.
- Separate visual morphing from PTY geometry. Reserve final dock space and do not
  resize the PTY on every animation frame.

## Further acceptance

The implemented **badge → control request → badge → bottom command review**
transition and Linux evidence are recorded in the [release validation](release-0.8.7-validation.md). Use common components, runes and
Rust boundaries rather than a separate harness approval experience.
Cover repeated/concurrent requests, other tabs, cancellation, expiry, reconnects,
delayed status, keyboard use, reduced motion, long commands, short windows and
native macOS shells. Ask people to explain what they just permitted and what can
still require a separate decision.
