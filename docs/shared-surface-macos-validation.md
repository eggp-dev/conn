# Shared surface: macOS release acceptance

Validate the unreleased `refactor/shared-surface` branch on an Apple Silicon Mac.
Read [the product contract](PRD.md), [protocol](protocol.md),
[external automation](external-automation.md) and [extensions](extensions.md) first.
The package version is still 0.6.0; record the exact Git commit instead of relying
on the version label. This is a release assessment, not authorization to publish.

## Prepare an isolated candidate

- Inspect repository instructions and Git state. Preserve existing edits and user
  sessions. Fetch the branch into a separate worktree if necessary.
- Use the pinned Rust toolchain and Node 24. Run `npm ci` in `frontends/tauri`.
- Run `cargo test --workspace --locked`, `npm test`, `npm run build`, and
  `cargo test --manifest-path frontends/tauri/src-tauri/Cargo.toml --lib --locked`.
  The release-payload signature fixture needs release artifacts; report it separately.
- Build the actual application from `frontends/tauri` with
  `APPLE_SIGNING_IDENTITY=- npm run tauri build -- --debug --bundles app`.
  This ad-hoc development build does not verify distribution notarization.
- Use a distinct development bundle identifier and separate `CONN_CONFIG_DIR`
  and `CONN_SOCKET`. Inspect the existing development setup before reusing it.
  Do not install over the release app or change the user's existing MCP setup.
- Run `python3 scripts/check_macos_scripting.py --app <candidate-app-path>`.
  Distinguish unavailable Xcode tooling from an application failure.

## Exercise the actual app

Use computer interaction and screenshots of the native app, not only a Chromium
browser harness. Use synthetic credentials and markers; never request real keys
or passwords in a report, screenshot, shell command or commit.

1. Confirm native WebKit viewport text and MCP snapshots agree for Unicode, wide
   characters, wrapping, scroll position and alternate-screen programs. Check
   concealed, transparent and equal foreground/background text. Masked input must
   remain masked. Verify PNG output where available; explicitly report
   `imageUnavailable` or any missing renderer capability.
2. Check inactive tabs, internal overlays, unfocused/minimized windows and window
   closure: shared sessions must remain readable and writable under the same lease and policy. Closing a window must terminate its sessions and cancel pending work.
   The contract does not claim detection of every other application's occlusion.
3. Use AppleScript to start an isolated external session, inject synthetic login
   input, and observe the real child program. Test hidden and star-masked input.
   Follow the current documented command syntax and app-path targeting.
4. Share that same running session through the owner UI with a selected persistent
   MCP connection. Confirm the process/SSH connection survives and control starts
   with the human. A connection with the same display name must not inherit access.
5. Request control and run a harmless command. In SSH or an unconfirmed foreground
   environment, require individual command review and disallow session-wide approval.
6. Let the human interrupt or correct an agent draft. Verify subsequent agent writes
   stop. Stop sharing with and without pending input: access must end immediately,
   while the human can continue in the same process. Verify sharing boundaries in
   the timeline and no retroactive private-input or startup-payload history.
7. Apply and revert a theme through Extensions. Test OS credential save/presence/
   deletion using a synthetic value and isolated config, with provider requests
   disabled. Do not send the synthetic value to OpenAI. Verify no plaintext file
   fallback and no read-key API. Do not lock or alter the user's entire login keychain.
8. Exercise completion acceptance/cancellation using fixtures: accept inserts only,
   never Enter; input, sharing, frame and control changes cancel stale proposals.
   A real provider call is a separate opt-in acceptance test with user-owned credentials.

## Report and fixes

Write a concise report with the exact commit, macOS/architecture, test counts,
native screenshots and PASS/FAIL/BLOCKED for every section. Keep device addresses,
account identities, credentials and raw authentication transcripts out of public docs.
Do not mark a browser-only result as native validation. Record development-build
evidence separately from signed-release/update evidence.

Fix reproducible issues on a separate branch based on this candidate; include
focused regression tests and summarize the changed files. Return the report and
patch or branch reference. Do not merge, tag, release or publish automatically.
