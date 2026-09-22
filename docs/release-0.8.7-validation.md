# 0.8.7 release validation

[한국어](release-0.8.7-validation.ko.md) · [Mac baseline](shared-runtime-macos-validation-results.ko.md) · [Control design](control-experience.md)

2026-09-23. This records source validation for the release candidate. The tag's
GitHub Actions and release assets separately establish native build, signing,
notarization and distribution results; this document does not predeclare those results.

## Changes and evidence

- Native desktop and web use the same Svelte components/runes, owner contract,
  session coordinator, PTY and policy engine. Web is a separate local host, not a
  mirror of the desktop process or a second simulated UI. The Linux x64 web archive
  includes the server, matching MCP CLI, compiled UI, manifest and checksums.
- Admission reserves an upper strip, control expands from the top-right Conn
  badge, and command review stays below the terminal. Collapse/Escape preserves
  the pending control request; automatic expansion does not steal typing focus.
  Notifications navigate to decisions without granting authority.
- A pending command carries its own review reason and eligible scope. Delayed
  global status cannot change its buttons. Profile/foreground changes invalidate
  a stale grant, and the backend still checks current policy. Session-wide grants
  require an explicit scope disclosure and remain unavailable for unverified shells.
- SSH foreground detection reads executable identity, never process arguments or
  credentials. Both direct SSH and `printf ...; ssh ...` were tested through real
  OpenSSH with the same remote policy behavior. Unknown contexts still require review.
- Human input bookkeeping retains no input bytes or command buffer. Navigation at
  a known empty prompt does not block the agent. Erasure must return the visible
  terminal to a previously observed empty boundary; Home followed by Ctrl-U cannot
  falsely clear a remaining suffix. No echo/hidden input is not inferred empty.
- The Mac long-input deadlock fix is retained. An ordered output fence prevents
  the process-exit event overtaking already queued output. The Linux entry point
  defaults the WebKit DMABUF renderer workaround while honoring explicit overrides.
- Browser QA reproduced whole-frame panning after focus/resize, a menu covered by
  admission, and hidden compact control buttons. Frame clipping, overlay stacking
  and panel height fix those observed defects. This does not prove the reported
  Mac gateway display issue had the same cause.

## Linux results

Linux, Rust 1.95.0, Node 26.8.1; production web build, real Rust host and persistent
MCP. Tests use isolated state directories and disposable files, not user work.

| Check | Result | Boundary |
| --- | --- | --- |
| Rust default workspace | 282 passed, 1 ignored | Native Tauri excluded; missing local zsh cases internally skipped |
| Common UI | 66 passed | State isolation, request-scoped review actions and existing behavior |
| Web/MCP collaboration | 10 passed | Real PTY; no browser page errors; control collapse/reopen, focus, compact/reduced motion, risk denial, human takeover, tab location and reconnect |
| Local input/rendering | 23 passed | Actual Bash, long input, editing, wrapped LF/CRLF output, fixed-window docks and density changes |
| Direct SSH input/rendering | 24 passed | Disposable loopback OpenSSH, remote Bash, UTF-8 locale |
| Compound SSH entry | 24 passed | Same fixture entered through `printf ...; ssh ...`, including ordinary commands and scoped review |
| Core/xterm screen parity | 85 passed | Current grid and cursor |
| Checkpoint restoration | 984 passed | Grid, history, cursor, styles, modes, continuation and resize |
| App lifetime | PASS | Real common UI with minimal host/IPC fixtures; 21 isolation/recovery checks, not native OS execution |
| Release tooling | 81 passed | Version, asset matrix, checksums, signing evidence, unsafe archive paths and source identity |
| Contract and UI boundaries | PASS | Generated owner contract; no alternate browser runtime |
| Web and desktop frontend builds | PASS | Zero Svelte errors; 17 pre-existing warnings and bundle-size warning remain |

The first SSH run used a non-UTF-8 fixture locale and failed Unicode checks. The
fixture was corrected to `C.UTF-8`, then all cases passed. This is a test-environment
correction, not evidence that every remote locale renders Unicode identically.

### Output load

The slow-consumer regression drains 4 MiB in ordinary CI. A separate 64 MiB run
with a 3 ms delay per output callback completed in 20.61 seconds, preserving every
byte and the final marker before process exit. Maximum process RSS was 88,508 KiB
(measured with GNU time, including payload allocation and the rest of the test).
The queue remains unbounded: sustained production faster than consumption can
increase memory without limit. Blocking the reader on a bounded queue would
reintroduce the observed PTY echo deadlock. This finite test is not an unlimited-load guarantee.

## Visual evidence

[Screenshots and checksums](validation/release-0.8.7/) show synthetic sessions only.
Desktop cases are 1280×800 at device scale 1; compact/reduced-motion is 640×420.
The in-app browser additionally verified collapse → badge → reopen → denial,
without focus theft or automatic approval, at 1280×720. The selected
[Motion Primitives reference](https://motion-primitives.com/docs/morphing-dialog)
was opened alongside the implementation: its surface continuity is the reference,
not its lamp image, centered modal, blur or typography.

## Acceptance limits

The incoming Mac native/local/loopback SSH evidence belongs to commit `5df5e36`.
The new control design and additional core fixes were not interactively rerun on a
physical Mac in this Linux session. macOS CI/build/signing and Homebrew install
checks are separate evidence, not a substitute for native collaboration acceptance.
The user's gateway, physical IME composition, native notification center and native
IPC fault injection remain unverified. Native Linux GTK development packages were
unavailable locally; platform compilation is delegated to the release CI matrix.
No remote web hosting, restored PTYs after server restart or restored grants are promised.
