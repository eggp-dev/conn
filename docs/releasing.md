# Maintaining and releasing Conn

English · [한국어](releasing.ko.md) · [User installation](getting-started.md) · [Platform policy](platform-support.md)

## Repository and CI

`eggp-dev/conn` uses `main` as its integration branch. Keep Actions tokens read-only by default, enable private vulnerability reporting, and require **Required checks** and resolved conversations for changes to `main`. The [versioned ruleset](../.github/main-ruleset.json) provides PR/CI requirements and an explicit owner recovery bypass. Update an existing ruleset rather than creating duplicates.

CI checks version consistency, repository hygiene, relative documentation links, release tooling, frontend tests and the Svelte build. Rust tests and native desktop debug builds cover Linux, macOS Apple Silicon and Windows. Release packaging uses the same three targets; Intel Mac distribution is paused after v0.4.1. The macOS debug build is bundled and signed ad-hoc in PR CI, then checked with `plutil` and `sdef`; `osacompile` compiles the external automation examples against that exact app without running it. This catches missing scripting resources and dictionary syntax without release credentials. These checks do not replace installer or interactive desktop testing.

CI sizes itself to the change (`scripts/ci_scope.py`, decided by the **Scope** job). Documentation-only changes run the docs, release tooling and frontend job alone. Web UI changes add one Linux desktop build. A pull request touching Rust runs the tests on all three systems and builds the desktop once on Linux; changes to the native shell, packaging, external automation or CI itself build every desktop before merging. Code merged to `main`, manual runs and every release validation always use the full matrix, and anything the scope cannot classify is treated as code. To get the full matrix on a pull request, add the `full-ci` label before pushing or run the CI workflow manually on the branch. **Required checks** fails if a job that was in scope did not succeed.

Actions are pinned by commit SHA. Pull requests receive no signing secrets. Only the draft-upload job gets `contents: write`. Mac release jobs use the six credentials documented in [macOS signing](macos-signing.md); the temporary keychain password is generated during the job.

## Prepare the version

1. Align the workspace version, `conn-core` dependency, desktop Cargo version, Tauri config, frontend package/lock, plugin manifests and marketplace. Refresh both Cargo lockfiles.
2. Add a user-facing [CHANGELOG](../CHANGELOG.md) entry and check the exact release version:

   ```sh
   python3 scripts/release.py check --tag v0.8.1
   python3 scripts/check_repo.py
   python3 -m unittest discover -s tests/release -v
   cargo test --workspace --locked
   cd frontends/tauri
   npm ci
   npm test
   npm run build
   ```

3. Merge the reviewed release changes and wait for **Required checks** on that commit. Include the agent-connection UI and binary installation documentation in the tagged source.
4. Create and push the version tag deliberately:

   ```sh
   git tag -a v0.8.1 -m "Conn v0.8.1 preview"
   git push origin v0.8.1
   ```

Tagging and publishing are maintainer release actions. Do not move a published tag.

## Draft pipeline

A `v*` tag or manual dispatch with an existing tag starts `release.yml`. It requires version agreement and a tag resolving to a commit on `main`, then reruns CI at that commit.

| Native target | Build host | Assets |
|---|---|---|
| Ubuntu x64 | Ubuntu 24.04 | CLI `.tar.gz`, desktop `.deb`, desktop `.AppImage` |
| macOS Apple Silicon | `macos-15` | CLI `.tar.gz`, desktop `.dmg`, signing report |
| Windows x64 | Windows Server 2022 | CLI `.zip`, NSIS installer `.exe` |

The platform contract is Ubuntu 24.04/26.04 runtime targets, intentionally unsigned Windows preview assets, and Developer ID signing plus notarization for Mac assets. GitHub-hosted Mac runners handle signing; a personal Mac is not required to run the CI job. There is no silent fallback to ad-hoc signing.

Each native runner also extracts its standalone CLI archive outside the checkout and checks `--version` without Rust or Node.js on its execution path. This checks the packaged CLI, not desktop GUI interaction.

The release stages are:

1. Build the matching CLI and desktop package on each native runner.
2. On Mac, sign and notarize the app, embedded CLI, standalone CLI and DMG as described in [macOS signing](macos-signing.md). Generate the Apple Silicon report, including acceptance and final asset hashes.
3. `release.py package` normalizes filenames. `finalize` requires all installers, updater archives/signatures and the valid Apple Silicon report before writing `SHA256SUMS` and bilingual notes.
4. After the exact commit's CI and all build/signing jobs pass, `draft` uploads **14 assets**: seven binaries/installers, one macOS updater archive, three updater signatures, `latest.json`, one signing report and `SHA256SUMS`. It creates or updates an unpublished prerelease and refuses to modify a public release.

Re-running can repair an incomplete draft. No workflow publishes automatically. Actions retains temporary build artifacts for seven days; uploaded release assets persist. The CLI archives contain the license and installation notes. The updater manifest is release-scoped and discovered via GitHub release metadata, including the explicit preview channel.

## Review and publish

Review the downloaded draft artifacts, not a different local build.

- Check the Apple Silicon report, `Accepted` notarization results, and the final asset hashes. Signing reports must match the exact release files.
- Compare checksums. With all files present, use `sha256sum -c SHA256SUMS` on Linux or `shasum -a 256 -c SHA256SUMS` on macOS. On Windows, compare `Get-FileHash <file> -Algorithm SHA256` with its entry. Checksums are separate from publisher signatures.
- Run the [platform checklist](platform-support.md#release-verification-checklist). Record build/signing results separately from clean installation, native GUI and external-agent connection results. Mark untested OS versions or behaviors explicitly.
- Confirm the desktop runs without Rust, Node.js or a dev server, and that copied agent configuration works without `PATH` setup. Test Windows PowerShell/cmd and Apple Silicon before describing those interactions as verified.
- Keep the intentional Windows unsigned notice. Mac assets need successful Developer ID and notarization evidence. Do not tell users to disable system protection.
- Review the bilingual release notes and download links, replace the pending changelog date with the publication date, and publish the reviewed prerelease explicitly. Then open the public version page and each linked asset to confirm availability.

A green CI build alone is not a claim that every installer or interaction passed. Release notes must say which platform checks ran and what remains untested. Preserve secrets in Actions; never attach certificates, credentials or detailed private logs.

For implementation details, see [Tauri's GitHub pipeline guide](https://v2.tauri.app/distribute/pipelines/github/) and the repository's [signing contract](macos-signing.md).

## One-line installs after a release

Nothing to do by hand. `scripts/install.sh` always resolves the newest published release. The Homebrew tap ([eggp-dev/homebrew-tap](https://github.com/eggp-dev/homebrew-tap)) checks for a new release every six hours, rewrites the cask from that release's `SHA256SUMS`, and then installs it for real on a macOS runner (audit, install, signature, notarization and version checks, uninstall). To update it at once, run its **Follow Conn releases** workflow.

## The repository's old address

Conn moved from `github.com/eggplantiny/conn` to `github.com/eggp-dev/conn` on 2026-09-20. Two rules follow from that, and both protect people who already installed Conn:

- **Update manifests keep the old address.** Conn 0.6.0 to 0.8.1 only install an update whose download address starts with the address they were built with. `UPDATER_REPOSITORY` in `scripts/release.py` therefore stays on the old address, a release test pins it, and builds from 0.8.2 on trust both addresses. Do not change it in a search and replace.
- **Never create or fork a repository named `eggplantiny/conn`.** GitHub redirects the old address only while that name is unused. Reusing it would stop updates for every older installation.

If the repository ever moves again, change `REPOSITORY` and `REPOSITORY_SLUG` in `scripts/release.py`, the `github.repository` conditions in the release and signing workflows (signing refuses to run anywhere else), `RELEASES` in the updater sources (add the previous address to the trusted list rather than replacing it), `REPO` in `scripts/install.sh`, the tap's cask and `CONN_REPO`, `media/demo/src/brand.ts` (then re-render the films), and the install lines in the READMEs.

## Failed release or rollback

A build or signing failure leaves the release unpublished. Correct the cause and rerun the draft workflow. An unpublished retag decision must be explicit and documented. For a public release, preserve its tag and binaries, document the problem, and ship a patch version. Never silently replace public assets.

## Updater signing

Release jobs require repository variable `TAURI_SIGNING_PUBLIC_KEY` and secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The private key must be encrypted; keep an owner-only encrypted backup and retain the password in your approved secrets manager. Losing the signing key prevents existing installations from trusting future updates. Do not rotate it casually or print it in logs.

`updater_artifacts.py config` embeds only the public key using a release-only Tauri override. `CONN_UPDATER_ENABLED=1` enables native installation and `CONN_RELEASE_CHANNEL=preview` defines the initial channel; a future stable workflow must set `stable`. Debug builds and browser tests cannot self-install. Release signing runs after final native signing/notarization: macOS archives the verified stapled app, Linux reuses AppImage and Windows reuses the NSIS installer. `latest.json` includes artifact signatures; the app verifies downloaded bytes before exposing installation. Installer code signing and updater signatures are separate protections.

No agreement is injected into the DMG. MIT notices remain bundled as `LICENSE`; the verification mount provides closed stdin, never an automatic Agree response.

Before publishing record native upgrade coverage: older updater-enabled build → candidate; interrupted/offline download; invalid signature; missing asset; permissions; relaunch; active shell confirmation; and startup after an installation failure. Build success does not establish these installed-app results. v0.6.0 is the bootstrap release for older manually updated clients.
