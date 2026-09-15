# macOS signing handoff

English · [한국어](macos-signing.ko.md) · [Platform policy](platform-support.md)

**Status: preparation only.** Apple Developer enrollment and signing credentials
are pending. The current workflow produces ad-hoc test packages; the steps below
describe the work required to enable Developer ID distribution, not an already
active signing integration.

## What the maintainer prepares

1. Complete Apple Developer Program enrollment and locate the Team ID.
2. On a Mac, generate a certificate signing request and create a **Developer ID
   Application** certificate for distribution outside the Mac App Store. Keep its
   private key, export the certificate/key as a password-protected `.p12`, and
   record the full signing identity. App Store registration is not required for
   GitHub downloads.
3. Choose notarization credentials: Apple ID + an **app-specific password** +
   Team ID is one option. An App Store Connect API key is an alternative.
4. Create a protected GitHub Actions environment for release signing. Store
   credentials directly in its secrets; do not paste them into an issue, PR,
   chat, repository file or build log.

For the Apple ID route, the intended configuration is:

| Name | Value |
|---|---|
| `APPLE_CERTIFICATE` | Base64-encoded `.p12`, including its private key |
| `APPLE_CERTIFICATE_PASSWORD` | Password used to export the `.p12` |
| `APPLE_SIGNING_IDENTITY` | Full Developer ID Application identity |
| `KEYCHAIN_PASSWORD` | Separate temporary CI keychain password |
| `APPLE_ID` | Apple account email |
| `APPLE_PASSWORD` | App-specific password, not the account login password |
| `APPLE_TEAM_ID` | Developer team ID |

## Repository work after credentials are ready

- Limit signing credentials to the macOS release jobs and protected release
  refs/environment. PR checks and ordinary builds need no signing secrets.
- Import the certificate into a temporary runner keychain, replace the current
  ad-hoc identity, and pass notarization credentials to Tauri. Fail the signing
  build when required credentials are absent; do not fall back silently.
- Build both Mac targets. Check the app and embedded `conn` sidecar. The
  separately packaged CLI comes from the workspace's `target` directory;
  signing the app does **not** establish that this separate CLI is signed.
  Sign that binary with hardened runtime, submit it in an accepted notarization
  container, and verify acceptance before packaging the final CLI archive.
- Verify the application/DMG notarization and stapling. Recompute `SHA256SUMS`
  **after** every signing/stapling change. Generate release notes describing the
  actual signatures on each asset; the current template says ad-hoc.
- Download the draft through a browser on Apple Silicon and Intel Macs, install
  and run the [platform checklist](platform-support.md#release-verification-checklist).

Example checks on the resulting app include:

```sh
codesign --verify --deep --strict --verbose=2 /Applications/Conn.app
codesign -dv --verbose=4 /Applications/Conn.app
spctl --assess --type execute --verbose=4 /Applications/Conn.app
xcrun stapler validate /Applications/Conn.app
```

Check the identity and notarization result, not just command exit codes. Archive
the non-secret verification results with the release evidence. Preserve existing
public assets; any later change to a published binary needs a new version.

Sources: [Tauri macOS signing and notarization](https://v2.tauri.app/distribute/sign/macos/),
[Apple Developer enrollment](https://developer.apple.com/programs/enroll/).
