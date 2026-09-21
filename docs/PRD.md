# Conn product contract

**Shared session screen · v0.8.0 preview.** [Architecture](architecture.md) · [한국어](PRD.ko.md)

## One shell, one shared view

Conn lets a person and their agents work in the same terminal. They observe the
same current terminal screen and exchange input control. The person can correct a path,
enter a password, stop a command, or continue alone without creating another shell.

The first-use path is: install the app, open a shell, connect an agent, review its
request, work together. Settings use progressive disclosure; English and Korean
share the same components and motion. Extensions support this path without turning
the terminal into a configuration dashboard.

## Responsibilities

| Concept | Owns |
| --- | --- |
| Session | PTY, process, execution profile, lifetime |
| Surface | Current terminal grid, revision and output sequence |
| Actor | A particular connection or native caller; its name is only a label |
| Participation | Whether agents may join, and which connections may participate |
| Authority | Who may submit input now; mode, lease, policy, review and grace |
| Origin | Immutable external/local launch provenance; never inferred from sharing |
| Activity | Explicit collaboration and verified shell execution events |

## Invariants

1. **One execution substrate.** Sharing never replaces the PTY or reconnects SSH.
2. **One observation source.** Core parses PTY output into the current terminal grid.
   Agent snapshots use this projection, with no scrollback.
3. **Observation belongs to the shared session.** Current terminal output remains
   available when a window is unfocused, minimized or covered, or another tab is active.
   No raw input, scrollback or process memory is exported. Hidden input stays absent;
   printed secrets are visible. The agent's binding never follows the human silently.
4. **No hidden input observation.** Password input with echo disabled contributes
   no characters; masking contributes the rendered masks. Terminal conceal/hidden
   cells must not expose underlying text. Visible secrets are visible to participants.
5. **Human input wins.** It revokes conflicting writes before reaching the PTY.
6. **Participation is independent of control.** Sharing begins with the human in
   control. Selected agents must still obtain authority under the mode and gate.
7. **Transitions invalidate work.** Sharing changes and takeover revoke conflicting
   work. Window visibility never invalidates participation. Explicit approvals still
   gate execution. Stale frames and handles cannot
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
12. **Extensions use the same boundaries.** A contribution never receives a PTY, an
    owner bridge or a private screen. An internal agent gets no private-screen exception.

## Sharing

External launches begin private before the child starts. The person may enable
sharing in the owning window and select connected agents by connection identity.
The external writer and queued writes are revoked before collaboration begins.
Private/shared changes keep the process and visible terminal intact. Returning to
private cancels participation while leaving the human's shell alive. Sharing is
not silently restored after process/app restart.

A new agent connection waits until the person allows it once in the Conn window; the
choice applies to that live connection only, and the prompt can be turned off in settings.
Ordinary tabs permit admitted connections subject to their mode and permissions.
Control is a revocable per-tab lease: one writer at a time, the person always preempts,
and an agent that moves to another tab releases what it held. See the
[control lease model](control-lease-model.ko.md).
After an explicit participant selection, newly connected sockets do not inherit
another connection's selection, even when their display names match.

## Initial extension boundary

A versioned registry exposes declarative themes. The host renders their settings.
There is no arbitrary third-party JavaScript, CSS, DOM, PTY or filesystem API,
marketplace, or general executable-plugin loader in this cut.

The registry keeps provider and completion kinds for reviewed built-in integrations,
and rejects them because none ships. The native OpenAI suggestion preview and its
OS credential-store adapter were removed; external systems are expected to plug in
through this registry instead.

## Removed paths

Unattended Entrust, public `hello kind=human/frontend`, raw output subscriptions,
owner controls over agent IPC, and renderer-authorized observation are removed.
The native app and authenticated browser test adapter use the same harness and UI.
The CLI remains an agent/MCP adapter and profile/local-file utility.

## Acceptance

Verify actual rendered Unicode, wrapping, scroll, resizing, alternate screens,
concealed/masked/hidden input, background observation and sharing revocation. Exercise wrong actor IDs,
sharing/output races, cancellation, private history suppression,
external authentication followed by same-session sharing, and human correction.

Linux tests do not establish native macOS or Windows acceptance. Release notes
must distinguish automated checks, browser runs and installed-app validation.

### Collaboration lifecycle clarification (unreleased)

App admission remains answerable without a shared session. A failed sharing preflight leaves the
external writer intact; stale participant selections require review again. Explicit takeover/release
and process exit end the corresponding pending work so a later decision cannot execute it.
See the [transition table and verified scope](collaboration-refactor-results.md).
