# Platform support and distribution

English · [한국어](platform-support.ko.md) · [Install](getting-started.md) · [Release](releasing.md)

Conn distributes its MIT-licensed source and native binaries through GitHub
Releases. One Rust engine and one Svelte UI serve all desktop platforms. The
browser harness is a development adapter to that engine.

## First preview policy

These are release targets. A target becomes a tested platform only when the
corresponding release asset passes the checklist below; see the dated
[verification record](backend-verification.md) for evidence.

| Platform | Release build host | Files | Publication policy |
|---|---|---|---|
| Ubuntu 24.04 and 26.04, x64 | GitHub `ubuntu-24.04` | `.deb`, `.AppImage`, CLI `.tar.gz` | Verify the same CI assets on both Ubuntu versions. |
| Windows x64 | GitHub `windows-2022` | NSIS `.exe`, CLI `.zip` | Unsigned preview is intentional; disclose publisher/SmartScreen prompts. Test on a Windows desktop before publication. |
| macOS Apple Silicon | GitHub `macos-15` | `.dmg`, CLI `.tar.gz` | Public distribution will use Developer ID signing and notarization after Apple enrollment. Current ad-hoc artifacts are for testing. |
| macOS Intel | GitHub `macos-15-intel` | `.dmg`, CLI `.tar.gz` | Same signing policy; validate independently on Intel hardware. |

The Tauri config currently declares macOS 12.0 as its deployment minimum. That
setting is not evidence of a tested minimum. Windows Server CI is likewise not
proof of Windows desktop/WebView2 behavior. List actual tested OS versions in each
release. Linux ARM, Windows ARM, Ubuntu 22.04 and other Linux distributions have
no binary compatibility commitment in this preview.

### Linux: build once on the oldest intended Ubuntu

The development host uses Ubuntu 26.04. A local build there can depend on newer
glibc or system libraries, so it must not replace the Ubuntu 24.04 release build.
AppImage also has a system-library baseline; it is not a guarantee of universal
Linux compatibility. Keep the release runner pinned to `ubuntu-24.04`, including
the CLI sidecar, and validate its output on clean 24.04 and 26.04 installations.
See [Tauri's AppImage guidance](https://v2.tauri.app/distribute/appimage/).

### Windows: unsigned first

A Windows certificate is not a prerequisite for this preview. Distribute the
unsigned installer and CLI with SHA-256 checksums and an explicit signing note.
SmartScreen or an unknown-publisher prompt may appear; managed machines may block
installation. Do not ask users to disable system protection. Certificates or a
signing service can be added later. Windows signing is separate from Apple's
notarization process. See [Tauri's Windows signing guide](https://v2.tauri.app/distribute/sign/windows/).

### macOS: enrollment, then signing

Apple enrollment is pending. The current workflow uses
`APPLE_SIGNING_IDENTITY=-`: ad-hoc signing, without notarization. Keep these
packages in testing/draft status. Enrollment alone does not configure CI; follow
the [macOS signing checklist](macos-signing.md) before public Mac downloads.

## Release verification checklist

Run against the downloaded draft artifacts, not only a development checkout.
Record the tag/commit, asset name and SHA-256, OS version, CPU architecture, shell,
desktop session (Wayland/X11 on Linux), and pass/fail observations.

- [ ] Verify the checksum, install the desktop and extract the CLI on a clean user account or VM.
- [ ] Launch without Node.js, Rust or the development server installed; check `conn --version` and the bundled CLI in Diagnostics.
- [ ] Type, paste, resize and open/close several tabs; confirm the shell exits when its tab closes.
- [ ] Test a local shell: bash on Ubuntu; PowerShell and cmd.exe on Windows; zsh on each Mac architecture.
- [ ] Connect an agent via `conn mcp`, read a disposable file, and inspect the original request in Timeline.
- [ ] Deny a deletion and confirm the file remains; approve a separate deletion and confirm its outcome.
- [ ] Cancel execution during grace, reclaim control by typing, and verify that the agent stops writing.
- [ ] Check English/Korean, keyboard focus, wrapping, and reduced motion in the native app.
- [ ] Close/reopen the app, check saved settings/timeline, and uninstall the desktop. Record what user data remains.
- [ ] On Ubuntu, test both `.deb` and AppImage on 24.04 and 26.04, including a real graphical session.
- [ ] On Windows, record installer trust prompts, WebView2 availability, and named-pipe connection/cleanup. Test WSL only if advertising it as verified.
- [ ] On macOS, verify app, sidecar and standalone CLI signatures, notarization and a fresh browser download on both architectures.

SSH, Docker, WSL and Git Bash are profile options. Mark them verified only after
testing the actual external environment. Automated builds, PTY tests, package
creation and native interaction remain separate evidence.

## Publication and updates

The workflow creates a **draft prerelease** with nine binaries and `SHA256SUMS`.
A maintainer reviews evidence and publishes explicitly. While Apple setup is
pending, a complete three-platform draft can remain unpublished; do not silently
substitute ad-hoc Mac packages for the intended notarized release.

Updates are manual: download a newer release. There is no in-app updater, updater
signing key, app-store listing or hosted package repository in this preview.
