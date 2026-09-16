# macOS release signing

English · [한국어](macos-signing.ko.md) · [Platform policy](platform-support.md) · [Release](releasing.md)

Public Mac packages must pass Developer ID signing and Apple notarization. This page defines the release pipeline and evidence required; it does not assert that a particular run has passed. Check that run's two `-signing.json` assets and release notes.

GitHub-hosted `macos-15` and `macos-15-intel` runners build and sign the two architectures. The maintainer can operate this CI from Linux; a personal Mac is not required to execute each release job. Native first-run and GUI checks still need a Mac environment. See [GitHub's runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

## Six release secrets

Use the existing Apple ID notarization route. The macOS release jobs consume these six GitHub Actions secrets:

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | Base64 `.p12` containing the Developer ID certificate and its private key |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12` export password |
| `APPLE_SIGNING_IDENTITY` | Full Developer ID Application identity |
| `APPLE_ID` | Apple account email |
| `APPLE_PASSWORD` | App-specific password, not the account login password |
| `APPLE_TEAM_ID` | Developer Team ID |

The job creates a random password for its temporary keychain. **Do not create a `KEYCHAIN_PASSWORD` secret.** Keep credentials in Actions secrets, never in repository files, issues, chats or logs. Pull-request builds do not receive these credentials. Protect the release workflow and refs; an optional protected Actions environment can add a review gate.

A valid Developer ID Application certificate includes its private key and must match the configured Team ID. Membership alone is not a certificate or proof that notarization passed. The workflow fails on missing or invalid credentials and has no ad-hoc fallback. [Tauri's signing guide](https://v2.tauri.app/distribute/sign/macos/) describes the certificate and notarization inputs.

## Optional credential check before tagging

Run [Check macOS signing credentials](../.github/workflows/signing-check.yml) from GitHub Actions on `main`, or from the repository with:

```sh
gh workflow run signing-check.yml --ref main
```

This manual, `main`-only job uses the same six secrets and preparation code on `macos-15`. It checks PKCS#12 import, the expected Developer ID identity and Team, and authentication with Apple's notarization service. It always cleans up its temporary keychain. No app is built and no release assets are created.

A successful check confirms credential preparation only; it does **not** establish that code signing or an artifact's notarization passed. If it fails, inspect the reported stage and error category before changing secrets; an import failure alone does not identify the cause.

## Pipeline stages

1. **Prepare.** Import the certificate into an ephemeral runner keychain; verify the Developer ID identity and Team match.
2. **Build.** Tauri builds the app and bundled CLI, signs them, and performs app notarization and stapling.
3. **Verify and notarize final assets.** Check strict signatures, hardened runtime and secure timestamps. Sign the standalone CLI separately, submit it in a ZIP to Apple, and require `Accepted`. Notarize and staple the final DMG, then inspect its mounted app and ticket. A bare CLI cannot be stapled; do not claim offline ticket availability for it.
4. **Package and record evidence.** Collect the final DMG and CLI archive, then write one `conn-v0.3.0-<target>-signing.json` per Mac target. It records the source commit, submission IDs, acceptance, verification results and final asset hashes without certificate passwords, credential values or detailed notarization logs.
5. **Gate upload.** Finalization requires both reports to match the final DMG and CLI archive. Only then are release checksums and a draft uploaded. The reports themselves are public assets covered by `SHA256SUMS`.
6. **Clean up.** Always remove the temporary keychain and credential files and restore the previous keychain search list, including on failure.

Signing the `.app` does not automatically sign the separately distributed CLI. All signing and stapling must finish before packaging and checksum generation. Rebuilding or altering an asset invalidates its prior hash-bound evidence.

The JSON reports summarize the native CI checks; they are not independent signatures or attestations. The Apple signatures and notarization tickets belong to the distributed software.

## Before public distribution

Download the draft in a browser on Apple Silicon and Intel Macs. Confirm the signatures and tickets, then complete the [native interaction checklist](platform-support.md#release-verification-checklist). Typical app checks are:

```sh
codesign --verify --deep --strict --verbose=2 /Applications/Conn.app
codesign -dv --verbose=4 /Applications/Conn.app
spctl --assess --type execute --verbose=4 /Applications/Conn.app
xcrun stapler validate /Applications/Conn.app
```

Inspect the reported identity and notarization evidence, not just an exit code. Record the tested OS version, asset hash, first-launch result and any untested behavior in release notes. Do not publish ad-hoc Mac artifacts as the notarized release or replace public assets after signing; ship a new version for changed binaries.
