# Backend verification — 2026-09-15

## Evidence on the Linux development host

- Core and CLI regression tests: **103 passed, 0 failed, 0 ignored**. Actual PTY command execution, profile environment
  propagation, child termination, IPC leases/disconnect, atomic profile save,
  stale revision rejection, default and enable/disable validation, remote argv
  construction, and mandatory review/deny handling.
- Added probe timeout regression covers a launcher that exits while a descendant
  still holds its output pipes. Added executable lookup regression covers profile
  PATH and relative executable/cwd combinations.
- Svelte type checking and production asset build pass. The source's ten existing
  Svelte accessibility/CSS warnings remain; new profile controls add no warnings.
- Native Linux Tauri code compiles and links using locally extracted, SHA-256
  checked Ubuntu development packages. No system package installation was needed.
- `npm run tauri build -- --debug --no-bundle` passes, including the sidecar build.
  The resulting Linux desktop binary embeds its frontend assets and does not need
  a Vite server. This is a debug app build, not a signed release/installer.
- Windows GNU target: core, CLI, test sources and Tauri application Rust code pass
  cross-compilation checks. The Tauri code-only check excludes the sidecar bundle;
  it is not a Windows installer or a Windows runtime test.

## Browser flow

`frontends/tauri/tests/ui/` mounts the real App, settings and tab components with
Tauri's official `mockIPC`/`mockWindows` APIs. The test fixture is not the production
entry point. Start Vite and visit `/tests/ui/` to reproduce it.

The observed workflow covered:

1. Add/edit a profile including cwd and environment, save, and see it in the picker.
2. Change the default, open a new tab, and retain the original tab's profile.
3. Open an SSH-profile tab from the picker.
4. Show a failed connection-test message and disable the profile for new tabs.
5. Show unavailable WSL entries as disabled on a Linux host.
6. Reopen settings with saved fixture state and verify no console errors.
7. Use Linux `Ctrl+Shift+,`, `Ctrl+Shift+T` and `Ctrl+Shift+1` from terminal focus.
8. Save PowerShell settings, switch to SSH and back, and retain the local shell
   kind, executable and startup arguments.

This verifies the real frontend against a simulated Tauri bridge. Backend
execution is separately covered by Rust integration tests; it is not proof of a
native WebKit/WebView2 interaction or a real SSH/WSL/container connection.

## Remaining validation

- Windows native ConPTY input/output, Named Pipe ownership/lifetime, PowerShell,
  cmd, Git Bash, WSL and WebView2 behavior on a Windows machine.
- macOS native desktop launch, PTY behavior and packaging on a Mac.
- Real SSH authentication/disconnection and Docker container sessions with the
  intended user targets. No remote credentials or target services were created.
- Native desktop mouse/keyboard and graphical behavior. Native UI control was not
  available in this environment; browser fixture behavior is recorded separately.
- GitHub Actions native platform matrix and signed/distributed installers.

The source changes include that CI matrix and explicit platform-specific code;
these pending checks must not be presented as passing tests.
