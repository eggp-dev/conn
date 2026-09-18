# Conn product contract

**Shared-surface hard cut · v0.7.0 preview.** [Architecture](architecture.md) · [한국어](PRD.ko.md)

## One shell, one shared view

Conn lets a person and their agents work in the same terminal. They observe the
same presented viewport and exchange input control. The person can correct a path,
enter a password, stop a command, or continue alone without creating another shell.

The first-use path is: install the app, open a shell, connect an agent, review its
request, work together. Settings use progressive disclosure; English and Korean
share the same components and motion. Extensions support this path without turning
the terminal into a configuration dashboard.

## Responsibilities

| Concept | Owns |
| --- | --- |
| Session | PTY, process, execution profile, lifetime |
| Surface | Actual rendered viewport, frame identity, freshness and visibility |
| Actor | A particular connection or native caller; its name is only a label |
| Participation | Whether agents may join, and which connections may participate |
| Authority | Who may submit input now; mode, lease, policy, review and grace |
| Origin | Immutable external/local launch provenance; never inferred from sharing |
| Activity | Explicit collaboration and verified shell execution events |

## Invariants

1. **One execution substrate.** Sharing never replaces the PTY or reconnects SSH.
2. **One observation source.** An agent snapshot uses the owner-rendered terminal
   viewport, including the person's scroll position. No headless VT fallback.
3. **Visible means presented by Conn.** Hidden tabs, background windows, covered
   terminal views and expired frames are unavailable. Conn does not claim to detect
   every overlap from another OS application.
4. **No hidden input observation.** Password input with echo disabled contributes
   no characters; masking contributes the rendered masks. Terminal conceal/hidden
   cells must not expose underlying text. Visible secrets are visible to participants.
5. **Human input wins.** It revokes conflicting writes before reaching the PTY.
6. **Participation is independent of control.** Sharing begins with the human in
   control. Selected agents must still obtain authority under the mode and gate.
7. **Transitions invalidate work.** Sharing changes and takeover revoke conflicting
   work. Unavailable surfaces pause observation and execution, while cancelling
   completion proposals; pending human decisions can resume after a fresh frame.
   Stale frames, handles and completion proposals cannot
   restore permission. Stopping sharing does not recall information already sent.
8. **Origins stay honest.** External launch/input payloads are not converted into
   activity history. A later shared external session keeps per-command review. An integrated local shell
   also requires review inside an unconfirmed foreground program, until its trusted
   outer-shell completion restores local prompt context.
9. **No raw human-input history.** Only supported shell execution hooks can create
   human command records. Editors and authentication input have no key-capture fallback.
10. **Human decisions remain distinct.** Control grant, command approval, proposal
    acceptance, co-sign and execution are separate events in one session timeline.
11. **Never forget physical input.** A transition must not silently strand tracked
    agent text or let another actor append to unfinished human input.
12. **Extensions use the same boundaries.** A model provider receives an authorized
    visible frame; suggestions are one-shot insertions accepted by the human.
    Acceptance does not send Enter. An internal agent gets no private-screen exception.

## Sharing

External launches begin private before the child starts. The person may enable
sharing in the owning window and select connected agents by connection identity.
The external writer and queued writes are revoked before collaboration begins.
Private/shared changes keep the process and visible terminal intact. Returning to
private cancels participation while leaving the human's shell alive. Sharing is
not silently restored after process/app restart.

Ordinary tabs permit connected agents subject to their mode and permissions.
After an explicit participant selection, newly connected sockets do not inherit
another connection's selection, even when their display names match.

## Initial extension boundary

A versioned registry exposes declarative themes and reviewed built-in provider and
suggestion integrations. The host renders their settings and proposal UI. There is
no arbitrary third-party JavaScript, CSS, DOM, PTY or filesystem API, marketplace,
or general executable-plugin loader in this cut.

OpenAI is the first model adapter. Keys belong in the OS credential store; model
calls run in native code. Missing keychain support fails without plaintext fallback.
Automatic suggestions require confirmed shell-prompt state. Explicit requests can
be used in uncertain environments, still with a currently shared visible surface.

## Removed paths

Unattended Entrust, public `hello kind=human/frontend`, raw output subscriptions,
owner controls over agent IPC, and headless/proxy agent observations are removed.
The native app and authenticated browser test adapter use the same harness and UI.
The CLI remains an agent/MCP adapter and profile/local-file utility.

## Acceptance

Verify actual rendered Unicode, wrapping, scroll, resizing, alternate screens,
concealed/masked/hidden input, frame expiry and occlusion. Exercise wrong actor IDs,
sharing/output races, stale completions, cancellation, private history suppression,
external authentication followed by same-session sharing, and human correction.

Linux tests do not establish native macOS or Windows acceptance. Release notes
must distinguish automated checks, browser runs and installed-app validation.
