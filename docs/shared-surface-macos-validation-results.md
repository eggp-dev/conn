# Shared-surface macOS validation results

The previously observed tooling blocker is resolved. Exercised functionality passes, including writes and Enter denied after focus loss and minimization **while the same agent lease remains valid**. No product release blocker was confirmed. Signed distribution, notarization, and update-artifact validation remain outside the completed development-build assessment.

## Source and scope

- Baseline: `f5a3aa7381cbcf18f0f44d665f6d95e7d09c38e0` (`refactor/shared-surface`).
- Tested revision: `9495568c720ee21875a2eeaa4f40cbed9012d13d` (`validation/macos-f5a3aa7`). The subsequent report commit changes documentation and evidence only.
- The three validation commits are `d68cbff1327d3c4307a9c3f64fd5e7fa9cebb289`, `62db039c360f69ad5bae4e397a962d8245aa5e03`, and `9495568c720ee21875a2eeaa4f40cbed9012d13d`. They change test fixtures, tooling diagnostics, and regression coverage; product runtime code is unchanged from the baseline.
- Actual native application, isolated worktree/config/socket/bundle, synthetic local shell and credentials. Existing installation and work were preserved.
- This public report contains selected synthetic-test results only. Device/account identifiers, local paths, original authentication transcripts, and unrelated desktop content are excluded.

## Live-lease focus and minimization checks

A persistent MCP connection obtained `lease#2` in autopilot mode. Each case was measured separately, with the window restored between cases. Public `terminal_list_tabs` responses immediately before and after the write/Enter probes established agent ownership and positive remaining lease lifetime. No ownership was injected through a test-only API.

| Case | Controller before probes | Write | Enter | Controller after probes | Result |
|---|---|---|---|---|---|
| Native minimize button | agent, `lease#2`, 39 seconds remaining | `surface_unavailable` | `surface_unavailable` | agent, same lease, 59 seconds remaining | PASS |
| Restore window, then activate another application; Conn window controls visibly inactive | agent, `lease#2`, 19 seconds remaining | `surface_unavailable` | `surface_unavailable` | agent, same lease, 59 seconds remaining | PASS |
| Human Ctrl-C comparison | human | `not_controller` | `not_controller` | human at probe start | PASS |

Lease lifetime refreshes during requests; the observations above are the returned values. Both hidden-state cases retained the **same** agent lease before and after rejection. These results therefore establish a visibility denial, rather than rejection because the agent had already lost control or expired.

[Selected MCP evidence](validation/shared-surface-macos/lease-visibility-evidence.json) retains response IDs, controller/lease information, and exact error messages. The native UI evidence is [agent control before minimizing](validation/shared-surface-macos/minimize-before.png), [unfocused window with agent control](validation/shared-surface-macos/unfocused-agent.png), and [human-control comparison](validation/shared-surface-macos/human-control.png).

A preliminary attempt did not actually deactivate Conn and was excluded from the focus-loss result. The screenshots retain the harmless command-not-found output from that setup attempt. The accepted unfocus case was taken after native window controls became inactive and both probes returned `surface_unavailable`.

## Completed acceptance checks

| Area | Result and scope |
|---|---|
| Native viewport and MCP text/PNG | PASS: Unicode, wide/combining characters, wrapping, scroll, alternate screen, conceal and equal foreground/background colors |
| Transparent foreground | PASS: separate native WebKit renderer fixture |
| Unavailable surfaces | PASS: inactive tabs, overlays, closed windows; live-lease focus/minimize cases documented above |
| External AppleScript authentication and sharing | PASS: synthetic hidden/star-masked input; same SSH transport survives sharing; human initially owns control; an unselected connection with the same display name cannot access the session |
| External writer after sharing | PASS: the original script's subsequent write is denied |
| Control, approval, human intervention | PASS: request control, individual SSH command approval, session-wide approval unavailable, subsequent agent writes denied after human interruption |
| Stop sharing | PASS: with and without pending input; agent access ends while the human continues on the same SSH connection |
| Timeline | PASS: sharing boundaries present; no retroactive private input or startup-payload history |
| Theme and keychain | PASS: apply/revert theme; synthetic key save/presence/delete; provider disabled; no plaintext setting fallback |
| Completion | PASS within fixture scope: native WebKit insertion/cancellation and actual Rust Harness cancellation boundaries. No real provider request |
| macOS scripting bundle check | PASS, exit 0: dictionary discovery, strict ad-hoc bundle signature verification, shipped AppleScript example compilation |
| Distribution signing, notarization and updates | NOT VERIFIED: requires release artifacts; not a demonstrated product failure |

PNG comparisons were visual, not pixel-equality assertions. The SSH fixture used a real loopback SSH transport with a synthetic command loop, not a production server. Native component fixtures are distinguished from the full application's real-provider path.

## Automated results and fixes

| Check | Result |
|---|---|
| Final workspace suite | 238 passed, 0 failed |
| Frontend tests / build | 42 passed / build passed |
| Tauri library | 4 passed, 1 release-artifact-dependent test ignored |
| macOS scripting checker regression tests | 10 passed |
| Completion cancellation boundary tests | 5 passed, included in workspace total |
| Native WebKit renderer / completion fixtures | 10 / 14 assertions passed |

The initial workspace run had 226 passing tests and one authentication-fixture failure. The fixture waited for a shell prompt that macOS could reset; it now waits for the explicit authentication-success marker while preserving same-process and private-history assertions. The checker distinguishes unavailable Xcode tooling/license state from an application failure. Two further regressions cover atomic cancellation of ready completion proposals on sharing stop and surface invalidation.

No additional runtime fix was required by the live-lease checks. No main merge, tag, release, or deployment is part of this validation.
