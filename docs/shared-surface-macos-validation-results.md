# Shared-surface macOS validation results

Native acceptance of the session-screen hard cut on an Apple Silicon Mac. This is a
development-build assessment, not a release decision. Earlier foreground-denial results
(pre-hardcut contract) are summarised at the end for history only.

## Source and scope

- Tested revision: `cc380ae0cd8c0126f61530f4f81b7e78e7428fa0` (`refactor/shared-surface`),
  2026-09-19.
- macOS 26 (Darwin 27.0.0), arm64. Rust 1.95.0, Node 25.9. Xcode present, but
  `xcode-select` points at Command Line Tools.
- Ad-hoc signed debug bundle (`APPLE_SIGNING_IDENTITY=-`, `--bundles app`) with a distinct
  identifier and product name, isolated `CONN_CONFIG_DIR` and `CONN_SOCKET`. The user's
  release app, config and MCP setup were not touched.
- Agents were raw-IPC clients speaking the same JSON-lines protocol as the MCP adapter.
  Credentials were synthetic markers only. Device, account and path details are omitted.

## Automated checks

| Check | Result |
|---|---|
| `cargo build --locked --workspace` with vendored `vt100` path dependency | PASS |
| `cargo test --locked --workspace` | 255 passed, 0 failed, 1 ignored |
| Frontend `npm test` / `npm run build` | 38 passed / build passed |
| Tauri library tests | 4 passed, 1 release-artifact-dependent test ignored |
| `scripts/check_macos_scripting.py --app <candidate>` | PASS (exit 0) with `DEVELOPER_DIR` set to Xcode; BLOCKED under the default Command Line Tools (`sdef` needs Xcode) |

The new lifecycle and admission tests (`#![cfg(unix)]`) ran on macOS as part of the
workspace total.

## Native checks

| Area | Result and evidence |
|---|---|
| Connection admission | PASS. A first `hello` returned `admission: pending`; discovery calls returned `admission_pending` at once; the native window showed the "wants to join" card. A `snapshot` issued before **Allow** waited and succeeded 5 s later, right after the click, and `tools_changed` was delivered. A second connection later produced a second card and was admitted independently. |
| Hard cut: minimized window | PASS. With the agent holding `lease#1`, `snapshot`, `type` and ENTER (`echo HARDCUT_MINIMIZED_$((6*7))`) executed while the window was minimized; the agent snapshot showed the output, the lease survived, and the native view caught up on restore. |
| Hard cut: unfocused / covered | PASS. With Finder frontmost (Conn not the active app), the same probe executed and was observed by the agent. Occlusion by another app's window was not measured separately; the contract does not claim it. |
| Hard cut: human on another tab | PASS. Human opened tab 2 and stayed there (`attended` = tab 2); the agent bound to tab 1 executed and observed `HARDCUT_OTHER_TAB_42` in tab 1. Tab 2 stayed empty. |
| External AppleScript login (private) | PASS. `create window with default profile command "/usr/bin/ssh …"` against a loopback SSH transport with synthetic password, hidden second prompt and star-masked third prompt. An admitted agent could not see the session: absent from `list_tabs`, and `snapshot`/`status`/`switch_tab` by its ID returned `session unavailable`. Native view: hidden input absent, masked input as stars. |
| Sharing the same external session | PASS. Owner opened the sharing panel, which listed the live connection unselected; **Start sharing** stayed disabled with nothing selected. After selecting the agent, the agent's `snapshot` (new `generation`) showed the authenticated remote shell with stars for the masked prompt and no synthetic credential in `snapshot`, `status`, `affordances`, `list_tabs` or any file in the config directory. The launcher's late `write text` was rejected (`Session unavailable for this caller`). `Stop external input` disappeared. |
| Unselected admitted connection | PASS. A second admitted connection could not see or address the shared external session. |
| Agent write after human ESC in the external session | Expected. `type`/ENTER returned `input_pending` because the human's unfinished input line must be cleared first. |

Native evidence was inspected as window captures during the run; no images are committed.

## Findings for follow-up

1. **A connection can hold leases in two sessions at once.** Bound to tab 1, the agent
   called `request_control` without `session` (granted `lease#1` in tab 1) and then
   `request_control` with `session` naming the shared external session (granted
   `lease#2` there); both `status` replies showed the same agent as controller. Earlier,
   `switch_tab` to tab 1 while holding a lease in the explicitly addressed session did
   not release that lease; it ran to natural expiry. The core test
   `moving_to_another_tab_releases_everything_held_in_the_source` covers only the bound
   tab. Explicit `session` addressing bypasses the per-tab lease scoping.
2. **Unexplained loss of access and silent app exit (first run, not reproduced).** About
   20 s after sharing an external session with an admitted, selected connection and one
   successful `snapshot`, every per-session call on it (`status`, `snapshot`,
   `request_control`, `switch_tab`) returned `session unavailable` while `list_tabs`
   kept listing it and the audit showed no sharing change. About three minutes later the
   app process exited with no crash report and nothing on stderr; the socket was removed
   as on a clean quit. In between, an Escape key was sent into that window, a second
   automation window was created, and a second agent connection sent `hello`. A second
   run repeated each of those steps in isolation with access re-checked after each, and
   none reproduced either symptom. Worth a look at the writer-side disclosure guard and
   the connection-loop `bound` state for explicitly addressed sessions.
3. **One `create window` reply was lost.** The first login run's `create window … command`
   never returned to `osascript`, although the window appeared and the SSH connection was
   made; a later identical run and a `nohup` re-run returned normally. The external
   automation docs already warn that a lost reply may follow a successful spawn.

### Follow-up from Linux

1. **Fixed.** Reproduced on Linux with the same two calls. A connection now holds one
   lease at a time: `request_control` in another session gives up what it holds
   elsewhere, and `switch_tab` / `open_tab` release every other tab, not only the bound
   one. Regression: `naming_a_session_explicitly_cannot_collect_leases_across_tabs`.
   This needs a macOS re-check.
2. **Not reproduced; now traceable.** Replaying the sequence on Linux (shared external
   session, control requested in both sessions, calls every four seconds for half a
   minute, then `switch_tab`) kept access. One detail narrows it: `list_tabs` and
   `switch_tab` apply the same per-connection participation filter, so within a single
   connection one cannot list a session that the other refuses. That pattern is what an
   admitted but unselected connection sees, so a client that issued the refused calls on
   a different connection than the listing one (for instance after a reconnect, or the
   second agent) would produce it. This is a hypothesis, not a finding. To settle it,
   start the app with `CONN_TRACE_REFUSALS=1`: each generic refusal is then printed to
   stderr with its connection id and cause (private, not selected, sharing changed,
   closed). The quiet exit with the socket removed matches an ordinary quit; with native
   input being driven by coordinates, a stray quit or window close is the first thing to
   rule out.

## Not verified here

- Signed distribution, notarization and updater artifacts (release artifacts required).
- A real OpenSSH `sshd`: this Mac has no root, so the login used a loopback paramiko
  transport with a synthetic prompt loop, as in the earlier validation. The ignored test
  `real_ssh_login_injected_by_a_launcher_stays_secret_after_sharing` and the
  `macos_hidden_login.py` fixture were not available on this machine.
- Windows.

## Historical: pre-hardcut foreground contract (superseded)

The earlier report on `validation/macos-f5a3aa7` (tested `9495568c…`) established
`surface_unavailable` for minimized and unfocused windows under the old presented-surface
contract, plus PASS results for viewport/PNG parity, AppleScript authentication and
sharing, control and approval, stop sharing, timeline, theme/keychain and completion
fixtures, with 238 workspace tests passing. Those focus/minimize denials are no longer
expected and are not acceptance evidence for the hard cut.
