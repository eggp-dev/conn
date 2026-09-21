<!--
Release notes template. `release.py finalize` fills it in and writes release-notes.md.

Keep only text that holds for every release here. What changed in a release comes from
the `## X.Y.Z` section of CHANGELOG.md and replaces {changes}; nothing in this file should
name a specific version's features or fixes. This comment is not copied into the notes.

Placeholders: {tag} {version} {repository} {source_commit} {changes}
  {mac_dmg} {windows_setup} {linux_deb} {linux_appimage} {mac_cli} {windows_cli} {linux_cli}
An unknown {placeholder} fails the release instead of being published as written.
-->

# Conn {tag}

**Preview / prerelease.** Conn gives a human and an agent one shared shell with
visible control handoff, command approval, and a combined collaboration timeline.
These binaries are built by CI; a green build is not a claim of full manual
validation on every operating system.
{source_commit}
## Download the desktop app

| Your computer | Download |
|---|---|
| Mac — Apple Silicon (M1 or newer) | [Conn for Apple Silicon]({repository}/releases/download/{tag}/{mac_dmg}) |
| Windows — x64 | [Windows installer]({repository}/releases/download/{tag}/{windows_setup}) |
| Ubuntu — x64 | [Ubuntu .deb]({repository}/releases/download/{tag}/{linux_deb}) · [Linux AppImage]({repository}/releases/download/{tag}/{linux_appimage}) |

Installers include the app and its Conn CLI sidecar; Rust and Node.js are not
required to use these binaries. Windows preview installers are unsigned, so a
SmartScreen or unknown-publisher prompt may appear. Native CLI startup is checked
in CI; full interactive GUI installation/collaboration checks remain pending.

## New in this release

{changes}

## Updating

Update the app and its CLI/MCP adapter together, then restart your MCP clients.
Existing files, profiles and saved activity remain; running shell sessions close
during app restart and are not migrated.

Installations from v0.6.0 on can discover this preview through the signed updater. Older
clients need a manual install. Updates support Apple Silicon macOS, Windows x64
and Linux AppImage; Debian packages use package-manager/manual updates. Installation
and restart remain your choice. Full installed-app upgrade coverage on every target
is not claimed by artifact signature checks.

한국어: 앱·CLI를 함께 업데이트하고 MCP 클라이언트를 재시작하세요. 파일·설정·저장 기록은
유지하며 실행 중인 셸은 앱 재시작 시 종료됩니다. v0.6.0 이후 설치본은 서명 업데이트를 사용할
수 있고 그보다 이전 버전은 직접 설치하세요.

## Native validation and remaining coverage

Validation coverage is recorded per version in the changelog and linked reports.
CI builds, scripting-dictionary checks, signing and notarization are separate from
interactive testing of an installed app. Previous development-app results do not
establish that every native interaction works in this build. Windows interactive
GUI and full installed-app upgrades remain unverified.

AppleScript is disabled by default. Enable it for selected local profiles in Settings
and consult the [automation guide]({repository}/blob/{tag}/docs/external-automation.md).

## Getting started

- [English guide]({repository}/blob/{tag}/docs/getting-started.md)
- [한국어 사용법]({repository}/blob/{tag}/docs/getting-started.ko.md)
- [What changed]({repository}/blob/{tag}/CHANGELOG.md)

For a separate MCP connector, download the matching CLI for
[Apple Silicon]({repository}/releases/download/{tag}/{mac_cli}),
[Windows x64]({repository}/releases/download/{tag}/{windows_cli}) or
[Linux x64]({repository}/releases/download/{tag}/{linux_cli}).
Linux x64 assets are built on Ubuntu 24.04 and target
Ubuntu 24.04/26.04; runtime coverage of these exact assets is separate from CI.
Windows targets x64 and is intentionally unsigned for this preview; a certificate
is not a release prerequisite. SmartScreen or unknown-publisher prompts may appear.
macOS apps, their embedded CLI sidecars, and standalone CLIs are Developer ID
signed with hardened runtime and secure timestamps. Apple notarization is
Accepted for the final DMGs and standalone CLI submissions; app and DMG tickets
are stapled and verified. Apple does not support stapling a standalone CLI or
its archive, so its notarization ticket is retrieved online when needed.
The Apple Silicon `-signing.json` asset records native runner checks and final asset hashes;
it is evidence of this build, not an independent cryptographic attestation.
Intel Mac packages are paused for new releases; previously published assets remain available.
Native interactive installation and collaboration coverage remains limited for this preview.
See the [platform policy]({repository}/blob/{tag}/docs/platform-support.md).
Follow the installation guide for platform trust prompts; never disable system
protection globally.

## Verify the download

Download `SHA256SUMS` with your selected asset. On Linux, run
`sha256sum --ignore-missing -c SHA256SUMS` from that directory. On macOS, compare
`shasum -a 256 <asset>` to the matching line; on Windows use
`Get-FileHash <asset> -Algorithm SHA256`. SHA-256 detects download corruption;
the checksum file is not a code-signing signature.

## Before giving an agent control

Commands use your account's permissions. Conn's policy and approval UI are not
an operating-system sandbox. An “executed” record means input reached the shell,
not that the command completed successfully. Start with a disposable project.
Read the [security model]({repository}/blob/{tag}/docs/security.md) and
[report security issues privately]({repository}/security/advisories/new).

한국어: AppleScript는 기본적으로 꺼져 있습니다. 버전별 검증 범위는 변경 기록과 연결된 보고서를 확인하세요. CI 빌드·스크립팅 사전·서명·공증 검증은 설치된 앱의 대화형 검증과 별개이며, 이전 개발 앱의 결과가 이번 배포판의 모든 동작을 보장하지 않습니다.
이 릴리스는 프리뷰입니다. 명령은 사용자 계정 권한으로 실행되며, 승인 기능은
운영체제 샌드박스가 아닙니다. 실행 기록은 입력 전달을 뜻하며 명령의 성공을 보장하지
않습니다. macOS 앱·내장 CLI·별도 CLI는 Developer ID로 서명하며 hardened runtime과
보안 타임스탬프를 검증합니다. DMG와 별도 CLI의 Apple 공증이 승인되었고 앱·DMG에는
티켓을 첨부했습니다. 별도 CLI와 아카이브에는 티켓을 첨부할 수 없어 필요 시 온라인으로
조회합니다. Apple Silicon 서명 보고서는 해당 빌드의 검증 기록이며 독립적인 암호학적 증명은 아닙니다.
새 릴리스의 Intel Mac 배포는 중단하며 기존 공개 파일은 유지합니다.
Windows 프리뷰는 무서명으로 배포하며 인증서가 필수 조건은 아닙니다. Linux는
Ubuntu 24.04 빌드를 24.04·26.04 대상으로 제공합니다. CI에서 각 운영체제의 별도 CLI
시작을 확인하며, 네이티브 GUI 설치·협업의 전체 대화형 검증은 아직 완료되지 않았습니다.
