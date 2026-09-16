# Platform support and distribution

English · [한국어](platform-support.ko.md) · [Install](getting-started.md) · [Release](releasing.md)

Conn provides its MIT-licensed source and native binaries through [GitHub Releases](https://github.com/eggplantiny/conn/releases/tag/v0.4.0). All desktop packages use the same Rust engine and Svelte UI. The browser harness is a development adapter.

## v0.4.0 preview targets

This table defines the build and distribution contract. It does not claim that every target has passed interactive testing. Each release must identify its actual build, signing, installation, and runtime results; dated evidence also lives in [backend verification](backend-verification.md).

| Target | Build runner | Files | Distribution policy |
|---|---|---|---|
| Ubuntu 24.04 / 26.04 x64 | `ubuntu-24.04` | `.deb`, `.AppImage`, CLI `.tar.gz` | Build on 24.04; record runtime results for both Ubuntu versions. |
| Windows x64 | `windows-2022` | NSIS `.exe`, CLI `.zip` | Intentionally unsigned preview. Disclose installer prompts and Windows desktop checks. |
| macOS Apple Silicon | `macos-15` | `.dmg`, CLI `.tar.gz` | Developer ID signatures and accepted notarization required for public assets. |
| macOS Intel | `macos-15-intel` | `.dmg`, CLI `.tar.gz` | Same signing gate; record Intel runtime results separately. |

Download the correct installer from [Getting started](getting-started.md). The desktop includes its matching CLI; **Settings → Agents → Connect your agent** copies configuration using the CLI's absolute path and the current endpoint. Desktop users do not need Rust, Node.js, or a `PATH` change.

### Linux

Release binaries, including the CLI, are built on Ubuntu 24.04. A local build on newer Ubuntu can require newer glibc or system libraries and must not replace the release baseline. Test the same downloaded `.deb` and AppImage on 24.04 and 26.04, including a graphical session. AppImage still has system-library requirements. See [Tauri's AppImage guide](https://v2.tauri.app/distribute/appimage/).

### Windows

A signing certificate is not required for this preview. SmartScreen or an unknown-publisher prompt may appear; managed machines may block installation. Release notes must say the installer and CLI are unsigned. Do not instruct users to disable system protection. Windows signing can be added independently later. See [Tauri's Windows signing guide](https://v2.tauri.app/distribute/sign/windows/).

A Windows Server CI build does not establish Windows desktop, WebView2, or interactive ConPTY compatibility. Record the desktop OS, shell and installer behavior actually tested.

### macOS

The [signing pipeline](macos-signing.md) uses GitHub-hosted Mac runners, Developer ID Application credentials, and Apple notarization. It requires signatures for the app, embedded CLI and standalone CLI; app and DMG tickets are stapled. A bare CLI is notarized in a ZIP submission but cannot carry a stapled ticket itself.

Both Mac targets produce a public `-signing.json` report bound to the final asset hashes. A successful signing report demonstrates those checks, not a completed first-run GUI test. Verify a fresh browser download on each architecture before claiming that interaction is tested.

Tauri declares macOS 12.0 as the deployment minimum; this is not a tested-minimum claim. Linux ARM, Windows ARM, Ubuntu 22.04 and other Linux distributions are outside this preview's binary compatibility commitment.

## Release verification checklist

Use the downloaded release candidates. Record tag/commit, filename and SHA-256, OS version, CPU, shell, and Linux display session. Mark each check **passed**, **failed**, or **not tested**; do not convert a CI pass into a native interaction claim.

- [ ] Match the checksum and install in a clean account or VM.
- [ ] Launch without Rust, Node.js or a development server. Check the bundled CLI path and copy agent configuration from Settings.
- [ ] Connect an external agent with that copied configuration; read the screen and a disposable file.
- [ ] Type, paste, resize, open/close tabs, and confirm the shell ends with its tab.
- [ ] Verify bash on Ubuntu, PowerShell and cmd.exe on Windows, and zsh on each Mac architecture.
- [ ] Approve a request, deny a separate request, cancel execution grace, and take control by typing. Check the actual shell outcome and original request in Timeline.
- [ ] Check English/Korean, keyboard focus, wrapping and reduced motion in the native app.
- [ ] Reopen the app to check saved settings/timeline, then uninstall and record retained user data.
- [ ] Ubuntu: test both package formats on 24.04 and 26.04 with a real graphical session.
- [ ] Windows: record trust prompts, WebView2 readiness, and named-pipe connection/cleanup.
- [ ] macOS: inspect both signing reports and test fresh downloaded app/CLI execution on Apple Silicon and Intel.

SSH, Docker, WSL and Git Bash are profile options. Claim verified support only for external environments actually tested. A build, a package, a signing check and interactive use are separate evidence.

## Publication and updates

The workflow prepares a **draft prerelease** with nine binaries/installers, two Mac signing reports, and `SHA256SUMS`. It does not publish automatically. Missing or failed Mac signing evidence blocks the release draft upload; unsigned Windows assets are the explicit preview policy.

Before publishing, maintainers review the [release checks](releasing.md), record runtime results and limitations, and confirm that download links match the assets. Updates are manual downloads. This preview has no in-app updater, updater signing key, app-store listing or hosted package repository.
