# SSH input and approval fixes

[한국어](ssh-input-review-results.ko.md) · [Reproduce the tests](browser-testing.md#ssh-input-cursor-and-approval-regression)

2026-09-22 · Unreleased changes based on v0.8.4 (`06143d0`).

## Report and reproduced causes

The user reported macOS with an SSH remote shell: long command input displaced the cursor/input position, **Allow this session** appeared unresponsive, and automatic execution with risk-based approval seemed unavailable. The requested behavior is ordinary commands executing automatically, with approval for risky commands.

- **Approval:** SSH forced every command into review, disabling session allowances even in Autopilot. The UI displayed a disabled button without explaining why.
- **Input/cursor:** the modal settings sheet still activated an obsolete 600-pixel side-panel offset. Opening/closing settings repeatedly resized the real PTY during the width animation. Long input reproduced corruption and disagreement between rendered rows and the core grid in real SSH.
- **Long approval:** a roughly 5,000-character command pushed action buttons below the visible dock. The common card now scrolls its content while retaining its actions.

## Resulting behavior

POSIX SSH uses the existing command policy without host cwd, home or filesystem target inspection. Ordinary commands execute in Autopilot, confirm rules request approval, and deny rules remain blocked. Session allowance applies to the matching confirm label and is published to the UI immediately. Co-pilot, connection admission, sharing and control grants retain their separate roles.

SSH profiles, direct SSH launches and a single SSH command identified by Bash/Zsh integration use this policy. The matching outer-shell exit restores local analysis. Lost SSH lifecycle requires review; other remote backends, non-POSIX shells and unclassified programs retain individual review, with an explanation in the UI.

Opening modal settings no longer resizes the underlying PTY. Actual window and collaboration-dock changes still resize it. No replacement shell, remote hook or separate execution channel is introduced.

## Verification and limits

- 258 default Rust tests passed; the existing optional SSH test is excluded from that count.
- The optional real OpenSSH authentication/sharing test passed separately, including ordinary remote execution and a privileged command held for review without execution. Private credentials stayed absent from shared data.
- 48 frontend tests, the desktop frontend build, 85 core/xterm grid-and-cursor comparisons, site build and repository/document checks passed.
- The production UI with the real Rust backend and loopback SSH exercised 240–7,000-character input, Korean/emoji input, repeated settings open/close, long-command approval at a narrow viewport, session allowance scope, human cursor movement/insertion, takeover and return to the original local shell.
- The existing collaboration-context harness covers the 12 tab, notice, sharing, reconnect and preparation stages.

The browser tests use Linux Chromium and remote Bash with UTF-8 locale. They reproduce and fix the settings-resize case; they do **not** establish that every cause of the user's macOS symptom is resolved. Native macOS WebKit, the user's exact remote shell configuration, non-UTF-8 line editors and installed-app upgrades remain unverified. These changes have not been released.
