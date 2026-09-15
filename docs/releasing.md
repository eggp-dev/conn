# Maintaining and releasing Conn

[한국어](releasing.ko.md)

## Repository setup

The repository is `eggplantiny/conn`; `main` is the integration branch. Enable Actions, Issues and private vulnerability reporting in GitHub settings. Keep workflow tokens read-only by default. Use a branch ruleset requiring the **Required checks** status and resolved conversations; allow the maintainer a deliberate bypass for recovery. For a solo-maintained preview, do not require an unavailable second reviewer.

Suggested About description: **A shared terminal for humans and AI agents. Review, hand over, take back.** Topics: `terminal`, `mcp`, `ai-agents`, `human-in-the-loop`, `tauri`, `rust`, `svelte`. English and Korean issues and contributions are welcome. The future website is a separate approval decision; no custom domain is configured by these workflows.

The versioned [main ruleset](../.github/main-ruleset.json) requires pull requests and CI, blocks force-push/deletion, and gives the repository owner an explicit recovery bypass. Apply it with `gh api --method POST repos/eggplantiny/conn/rulesets --input .github/main-ruleset.json` only when no matching ruleset exists; update the existing ID instead on later runs.

## What CI verifies

`ci.yml` checks version consistency, public repository hygiene, relative documentation links, release tooling tests, timeline/icon tests, and the Svelte production build. Rust tests and native desktop debug builds run on Linux, macOS ARM and Windows. Release builds additionally cover Intel macOS. Unix-only PTY tests do not establish Windows runtime compatibility; run platform smoke tests before publishing.

Actions are pinned to commit SHAs. Dependabot proposes weekly grouped updates. Pull requests receive no release credentials. The release publisher alone has `contents: write`.

## Distribution policy

Follow [platform support](platform-support.md): Ubuntu 24.04 builds with 24.04/26.04 runtime checks; intentionally unsigned Windows previews; Developer ID signing and notarization for public Mac downloads after enrollment. Windows certificate acquisition does not block this preview. Current Mac ad-hoc builds are testing artifacts.

## Version a release

1. Update the workspace version, the `conn-core` workspace dependency version, standalone desktop Cargo version, Tauri config, frontend package/lock, plugin manifests and marketplace version. Refresh both Cargo lockfiles.
2. Add a user-facing entry to [CHANGELOG.md](../CHANGELOG.md), then run:

   ```sh
   python3 scripts/release.py check --tag v0.3.0
   python3 scripts/check_repo.py
   python3 -m unittest discover -s tests/release -v
   cargo test --workspace --locked
   cd frontends/tauri
   npm ci
   npm test
   npm run build
   ```

3. Merge the reviewed change to `main` and wait for **Required checks**.
4. Tag that commit (`git tag -a v0.3.0 -m "Conn v0.3.0 preview"`) and push the tag (`git push origin v0.3.0`). Do not move a published tag. These are maintainer publication actions, not part of a local build.

## Draft release pipeline

A `v*` tag or manual dispatch with an existing tag starts `release.yml`. It verifies that the version matches and the tagged commit belongs to `main`, then reruns CI for that exact commit. Four native jobs build the CLI sidecar and desktop:

| Target | Assets |
|---|---|
| Ubuntu 24.04 / 26.04 x64 (built on 24.04) | CLI `.tar.gz`, desktop `.deb`, desktop `.AppImage` |
| macOS Apple Silicon | CLI `.tar.gz`, desktop `.dmg` |
| macOS Intel | CLI `.tar.gz`, desktop `.dmg` |
| Windows x64 | CLI `.zip`, desktop NSIS `.exe` |

`release.py package` normalizes names; `finalize` requires all nine expected files and writes `SHA256SUMS` plus bilingual release notes. Only after all builds succeed does `draft` upload a **draft prerelease**. It refuses to update an already published release. Re-running the workflow can repair an incomplete draft. No workflow publishes automatically.

The desktop includes the matching CLI sidecar. Standalone CLI archives include the license and installation notes. Artifacts are temporarily retained by Actions for seven days; release assets persist once uploaded. There is no automatic updater or updater signing key in this preview.

## Before pressing Publish

Download the draft assets and verify the checksums (`sha256sum -c SHA256SUMS` with all assets present; macOS: `shasum -a 256 -c SHA256SUMS`; Windows: `Get-FileHash` on each downloaded asset and compare). A checksum detects corruption; it does not establish publisher identity without a trusted source.

On each advertised OS/architecture, install and launch the app, install/extract the CLI, open a profile, connect `conn mcp`, read a disposable file, deny a deletion, approve a separate deletion, test takeover/grace, inspect original request details in Timeline, switch English/Korean, and close the session. Record results and known limitations in release notes. Change the pending changelog date only when publishing.

### Signing

macOS builds currently use ad-hoc signing (`APPLE_SIGNING_IDENTITY=-`) and are **not notarized**. Keep these in testing/draft status until the [Apple signing handoff](macos-signing.md) is complete. That checklist covers enrollment, Developer ID certificates, protected CI credentials, the separate CLI, and verification before checksums/publication.

Windows installers are intentionally **not certificate-signed** for the preview. Record possible SmartScreen/unknown-publisher prompts and any managed-device restrictions. A Windows signing certificate is a later improvement, not a release prerequisite. Do not describe an unsigned artifact as publisher-verified or ask users to disable system protection. Never commit certificates, passwords or keys.

Use the [release verification checklist](platform-support.md#release-verification-checklist) for every advertised platform. Local Ubuntu 26.04 packages cannot substitute for the Ubuntu 24.04 CI assets in compatibility checks.

See the official [Tauri GitHub pipeline guide](https://v2.tauri.app/distribute/pipelines/github/), [distribution overview](https://v2.tauri.app/distribute/) and [Windows installer guide](https://v2.tauri.app/distribute/windows-installer/). Signing and native installation must be verified on the target platform; a Linux check cannot substitute for them.

## Failed release or rollback

A build failure leaves no published release. Fix the source, run CI and cut a new version when necessary. For an unpublished tag, document any retag decision explicitly with collaborators. For an already public release, preserve its tag and assets, document the problem, and ship a patch version. Never silently replace published binaries.
