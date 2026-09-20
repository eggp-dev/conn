"""Release contract tests; no network or platform toolchain required."""
import importlib.util
import base64
import json
import os
import re
import subprocess
from pathlib import Path
import shutil
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("conn_release", ROOT / "scripts/release.py")
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)

VERSION_FILES = [
    "Cargo.toml", "Cargo.lock", "package-lock.json", "frontends/tauri/package.json",
    "frontends/tauri/src-tauri/tauri.conf.json",
    "plugin/.claude-plugin/plugin.json", "plugin/.codex-plugin/plugin.json",
    ".claude-plugin/marketplace.json",
]

# A made-up changelog: the tests pin where notes come from, not what any real release said.
CHANGELOG = """# Changelog

## Unreleased

- Unreleased work that must never reach release notes.

## {version} — Preview · 2026-01-02

- First fixture change with `code {{braces}}` kept as written.
- Second fixture change, see [the protocol](docs/protocol.md) and [the site](https://example.com/).

한국어: 픽스처 변경 사항입니다.

## 0.0.1 — Preview · 2026-01-01

- Older fixture change that belongs to another release.
"""

# Every kind of release link the real READMEs and guides use, next to history that must never be rewritten.
PROSE = """| Platform | v{version} preview | v{version} 프리뷰 |
[Download](https://github.com/eggp-dev/conn/releases/download/v{version}/conn-v{version}-aarch64-apple-darwin-desktop.dmg)
[All assets](https://github.com/eggp-dev/conn/releases/tag/v{version}). `CONN_VERSION=v{version}` pins a release.
sudo apt install ./conn-v{version}-x86_64-unknown-linux-gnu-desktop.deb

## New in v0.8.0

Updates need v0.6.0 or later, Intel Macs stopped after v0.4.1, and a library at {version} is not a link.
"""
PROSE_LINKS = 7


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "repo"
        self.root.mkdir()
        for name in VERSION_FILES:
            destination = self.root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, destination)
        (self.root / "LICENSE").write_text("MIT License\n", encoding="utf-8")
        declared = release.read_toml(self.root / "Cargo.toml")["workspace"]["package"]["version"]
        (self.root / "CHANGELOG.md").write_text(CHANGELOG.format(version=declared), encoding="utf-8")
        for name in release.PROSE_FILES:
            (self.root / name).parent.mkdir(parents=True, exist_ok=True)
            (self.root / name).write_text(PROSE.format(version=declared), encoding="utf-8")
        self.version = release.check(self.root)
        self.tag = "v" + self.version
        self.out = Path(self.temporary.name) / "assets"
        self.sha = "a" * 40

    def platform_build(self, target):
        executable = self.root / "target" / target / "release" / ("conn.exe" if "windows" in target else "conn")
        executable.parent.mkdir(parents=True)
        executable.write_bytes(b"test executable\n")
        bundle = self.root / "frontends/tauri/src-tauri/target" / target / "release/bundle"
        for extension in release.TARGETS[target]:
            destination = bundle / ("nsis" if extension == ".exe" else extension.lstrip(".")) / ("Conn" + extension)
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(b"test installer\n" + extension.encode())
        return executable, bundle

    def complete_assets(self):
        for target in release.TARGETS:
            executable, _ = self.platform_build(target)
            release.package(target, self.out, self.root)
            if target.endswith("apple-darwin"):
                signature = {"developerId": True, "teamVerified": True, "secureTimestamp": True,
                             "hardenedRuntime": True, "cdhash": "1" * 40}
                report = {"schemaVersion": 1, "version": self.version, "target": target,
                          "sourceCommit": self.sha,
                          "app": {**{part: signature.copy() for part in ("bundle", "desktop", "sidecar")},
                                  "stapled": True, "gatekeeperAccepted": True},
                          "cli": {**signature, "stapled": False, "sha256": release.sha256(executable)},
                          "dmg": {**signature, "hardenedRuntime": False, "stapled": True,
                                  "gatekeeperAccepted": True, "containedAppVerified": True},
                          "notarization": {kind: {"status": "Accepted", "id": "12345678-1234-1234-1234-123456789abc"}
                                           for kind in ("cli", "dmg")},
                          "assets": {name: release.sha256(self.out / name) for name in release.asset_names(self.version, target)}}
                (self.out / release.signing_name(self.version, target)).write_text(json.dumps(report))
        # Structural test fixture only: runtime cryptographic verification is separate.
        signature = base64.b64encode(("untrusted comment: fixture\n" + base64.b64encode(bytes(74)).decode() + "\ntrusted comment: fixture\n" + base64.b64encode(bytes(64)).decode() + "\n").encode()).decode()
        for target in release.TARGETS:
            name = release.update_asset(self.version, target)
            if target.endswith("apple-darwin"):
                (self.out / name).write_bytes(b"notarized updater archive fixture")
                path = self.out / release.signing_name(self.version, target)
                report = json.loads(path.read_text())
                report["updater"] = {"archive": name, "sha256": release.sha256(self.out / name), "containedAppVerified": True}
                path.write_text(json.dumps(report))
            (self.out / (name + ".sig")).write_text(signature)
        release.finalize(self.out, self.tag, self.sha, self.root)

    def test_updater_manifest_is_complete_and_tied_to_public_assets(self):
        self.complete_assets()
        manifest = release.read_json(self.out / "latest.json")
        self.assertEqual(manifest["version"], self.version)
        self.assertEqual(set(manifest["platforms"]), {"darwin-aarch64", "linux-x86_64", "windows-x86_64"})
        for entry in manifest["platforms"].values():
            # Installed 0.6.0-0.8.1 apps reject any other prefix, so this must survive the move to the organization.
            self.assertTrue(entry["url"].startswith(f"https://github.com/eggplantiny/conn/releases/download/{self.tag}/conn-{self.tag}-"))
            self.assertTrue((self.out / entry["url"].rsplit("/", 1)[1]).is_file())
        self.assertEqual(release.REPOSITORY, "https://github.com/eggp-dev/conn")
        sig = next(self.out.glob("*.sig")); sig.write_text("corrupt")
        with self.assertRaisesRegex(release.ReleaseError, "signature"):
            release.finalize(self.out, self.tag, self.sha, self.root)

    def test_missing_update_and_changed_macos_archive_block_release(self):
        self.complete_assets()
        app = self.out / release.update_asset(self.version, "aarch64-apple-darwin")
        app.write_bytes(b"changed archive")
        with self.assertRaisesRegex(release.ReleaseError, "notarized-app"):
            release.finalize(self.out, self.tag, self.sha, self.root)
        app.unlink()
        with self.assertRaises(release.ReleaseError):
            release.finalize(self.out, self.tag, self.sha, self.root)

    def test_license_is_bundled_without_dmg_agreement(self):
        config = release.read_json(self.root / "frontends/tauri/src-tauri/tauri.conf.json")
        self.assertNotIn("licenseFile", config["bundle"])
        self.assertEqual(config["bundle"]["resources"]["../../../LICENSE"], "LICENSE")

    def test_versions_and_exact_tag(self):
        self.assertEqual(release.check(self.root, self.tag), self.version)
        for tag in (self.version, "v99.0.0", "../v" + self.version, self.tag + "\n"):
            with self.subTest(tag=tag), self.assertRaises(release.ReleaseError):
                release.check(self.root, tag)

    def test_rejects_drift_in_each_published_manifest(self):
        for name in VERSION_FILES:
            path = self.root / name
            original = path.read_text(encoding="utf-8")
            with self.subTest(name=name):
                if name.endswith("Cargo.lock"):
                    changed = original.replace(f'name = "conn-core"\nversion = "{self.version}"', 'name = "conn-core"\nversion = "99.0.0"')
                else:
                    changed = original.replace(f'"{self.version}"', '"99.0.0"')
                self.assertNotEqual(original, changed)
                path.write_text(changed, encoding="utf-8")
                with self.assertRaises(release.ReleaseError):
                    release.check(self.root, self.tag)
                path.write_text(original, encoding="utf-8")

    def tree(self):
        return {path.relative_to(self.root).as_posix(): path.read_bytes() for path in self.root.rglob("*") if path.is_file()}

    def test_bump_rewrites_every_declaration_and_release_link(self):
        # Somebody else's package at our version number must keep it.
        lock = self.root / "Cargo.lock"
        lock.write_text(lock.read_text(encoding="utf-8") + f'\n[[package]]\nname = "not-conn"\nversion = "{self.version}"\n', encoding="utf-8")
        npm = self.root / "package-lock.json"
        packages = json.loads(npm.read_text(encoding="utf-8"))
        packages["packages"]["node_modules/not-conn"] = {"version": self.version}
        npm.write_text(json.dumps(packages, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        changed = dict(release.bump("99.1.0", self.root))
        self.assertEqual(set(changed), set(VERSION_FILES) | set(release.PROSE_FILES))
        self.assertEqual(set(VERSION_FILES), set(release.VERSION_PATTERNS))
        self.assertEqual(sum(changed[name] for name in VERSION_FILES), len(release.declarations(self.root)))
        for name in release.PROSE_FILES:
            self.assertEqual(changed[name], PROSE_LINKS)
            self.assertEqual((self.root / name).read_text(encoding="utf-8"), PROSE.format(version="99.1.0").replace("library at 99.1.0", f"library at {self.version}"))
        self.assertEqual(set(release.declarations(self.root).values()), {"99.1.0"})
        self.assertIn(f'name = "not-conn"\nversion = "{self.version}"', lock.read_text(encoding="utf-8"))
        self.assertEqual(json.loads(npm.read_text(encoding="utf-8"))["packages"]["node_modules/not-conn"]["version"], self.version)
        # The changelog is a person's job: bump says so, and check holds the release until it is done.
        self.assertIn("## 99.1.0", "\n".join(release.bump_followups(self.root, "99.1.0")))
        with self.assertRaisesRegex(release.ReleaseError, "CHANGELOG.md has no '## 99.1.0'"):
            release.check(self.root)
        changelog = self.root / "CHANGELOG.md"
        changelog.write_text(changelog.read_text(encoding="utf-8").replace("## Unreleased", "## 99.1.0 — Preview"), encoding="utf-8")
        self.assertEqual(release.bump_followups(self.root, "99.1.0"), [])
        self.assertEqual(release.check(self.root, "v99.1.0"), "99.1.0")

    def test_bump_refuses_a_disagreeing_tree_and_an_older_version(self):
        before = self.tree()
        lower = ".".join(str(max(int(part) - 1, 0)) for part in self.version.split("."))
        for version in (self.version, lower, self.version + "-rc.1", "1.2", "v99.0.0"):
            with self.subTest(version=version), self.assertRaises(release.ReleaseError):
                release.bump(version, self.root)
        manifest = self.root / "frontends/tauri/package.json"
        original = manifest.read_text(encoding="utf-8")
        manifest.write_text(original.replace(f'"{self.version}"', '"98.0.0"'), encoding="utf-8")
        with self.assertRaisesRegex(release.ReleaseError, "disagree"):
            release.bump("99.0.0", self.root, force=True)
        manifest.write_text(original, encoding="utf-8")
        self.assertEqual(self.tree(), before)
        self.assertEqual(release.bump(self.version, self.root, force=True), [])
        self.assertEqual(len(release.bump(self.version + "-rc.1", self.root, force=True)), len(VERSION_FILES) + len(release.PROSE_FILES))
        self.assertEqual(set(release.declarations(self.root).values()), {self.version + "-rc.1"})
        self.assertNotIn(f"v{self.version} preview", (self.root / "README.md").read_text(encoding="utf-8"))
        # A release is newer than its own prerelease, and the way back restores every byte.
        release.bump(self.version, self.root)
        self.assertEqual(self.tree(), before)

    def test_bump_changes_nothing_when_a_declaration_cannot_be_rewritten(self):
        config = self.root / "frontends/tauri/src-tauri/tauri.conf.json"
        config.write_text(json.dumps(json.loads(config.read_text(encoding="utf-8")), indent=8), encoding="utf-8")
        before = self.tree()
        with self.assertRaisesRegex(release.ReleaseError, "tauri.conf.json"):
            release.bump("99.0.0", self.root)
        self.assertEqual(self.tree(), before)

    def test_versions_order_like_semver(self):
        ordered = ["0.9.9", "1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-rc.2", "1.0.0-rc.10", "1.0.0", "1.0.1", "1.10.0"]
        self.assertEqual(sorted(reversed(ordered), key=release.version_key), ordered)

    def test_check_rejects_a_release_link_left_on_another_version(self):
        readme = self.root / "README.ko.md"
        original = readme.read_text(encoding="utf-8")
        stale = ("| v0.0.9 preview |", "| v0.0.9 프리뷰 |", "(https://github.com/eggp-dev/conn/releases/tag/v0.0.9)", "`CONN_VERSION=v0.0.9`",
                 "releases/download/v0.0.9/SHA256SUMS", "./conn-v0.0.9-x86_64-unknown-linux-gnu-desktop.AppImage",
                 f"releases/tag/v{self.version}-rc.1", f"conn-v{self.version}-rc.1-aarch64-apple-darwin-cli.tar.gz")
        for text in stale:
            with self.subTest(text=text):
                readme.write_text(original + "\n" + text + "\n", encoding="utf-8")
                with self.assertRaisesRegex(release.ReleaseError, rf"README\.ko\.md:{len(original.splitlines()) + 2}: "):
                    release.check(self.root)
                self.assertEqual(len(release.bump_followups(self.root, self.version)), 1)
        readme.write_text(original + "\nSee v0.0.9, 0.0.9 and the v0.0.9 notes; conn-v2 is a name, not an asset.\n", encoding="utf-8")
        self.assertEqual(release.check(self.root), self.version)
        readme.unlink()
        with self.assertRaisesRegex(release.ReleaseError, "README.ko.md not found"):
            release.check(self.root)

    def test_numeric_semver_and_path_traversal_rejected(self):
        for version in ("01.2.3", "1.2", "1.2.3-01", "1.2.3/../../secret", "1.2.3\n", "1.2.3\u0661"):
            with self.subTest(version=version), self.assertRaises(release.ReleaseError):
                release.validate_version(version)
        with self.assertRaises(release.ReleaseError):
            release.asset_names(self.version, "../../secret")

    def test_future_releases_include_only_supported_targets(self):
        self.assertEqual(set(release.TARGETS), {
            "aarch64-apple-darwin", "x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc",
        })
        with self.assertRaises(release.ReleaseError):
            release.asset_names(self.version, "x86_64-apple-darwin")
        # One table serves the manifest and updater_artifacts.py; it must cover every packaged target.
        self.assertEqual(set(release.PLATFORMS), set(release.TARGETS))

    def test_missing_platform_build_is_not_a_successful_package(self):
        with self.assertRaises(release.ReleaseError):
            release.package("x86_64-unknown-linux-gnu", self.out, self.root)
        self.assertFalse(self.out.exists())

    def test_missing_or_duplicate_bundle_fails_before_writing(self):
        target = "x86_64-unknown-linux-gnu"
        _, bundle = self.platform_build(target)
        appimage = next(bundle.rglob("*.AppImage"))
        appimage.unlink()
        with self.assertRaises(release.ReleaseError):
            release.package(target, self.out, self.root)
        self.assertFalse(self.out.exists())
        appimage.write_bytes(b"appimage")
        appimage.with_name("duplicate.AppImage").write_bytes(b"duplicate")
        with self.assertRaises(release.ReleaseError):
            release.package(target, self.out, self.root)

    def test_linux_archive_is_deterministic_and_contains_only_public_allowlist(self):
        target = "x86_64-unknown-linux-gnu"
        self.platform_build(target)
        (self.root / ".env").write_text("PRIVATE=not-for-release")
        (self.root / "test-file.md").write_text("private shell test")
        first = release.package(target, self.out, self.root)
        second = release.package(target, self.out.with_name("other"), self.root)
        self.assertEqual(release.sha256(first[0]), release.sha256(second[0]))
        with tarfile.open(first[0]) as archive:
            self.assertEqual(archive.getnames(), ["conn", "LICENSE", "README.txt"])
            self.assertEqual(archive.getmember("conn").mode, 0o755)
            self.assertIn(b"not guarantee", archive.extractfile("README.txt").read())

    def test_windows_requires_installer_and_cli_zip(self):
        target = "x86_64-pc-windows-msvc"
        self.platform_build(target)
        files = release.package(target, self.out, self.root)
        self.assertTrue(files[1].name.endswith("-setup.exe"))
        with zipfile.ZipFile(files[0]) as archive:
            self.assertEqual(archive.namelist(), ["conn.exe", "LICENSE", "README.txt"])

    @unittest.skipUnless(os.name == "posix", "POSIX executable permissions unavailable")
    def test_linux_appimage_retains_executable_permissions(self):
        target = "x86_64-unknown-linux-gnu"
        _, bundle = self.platform_build(target)
        source = next(bundle.rglob("*.AppImage"))
        source.chmod(0o755)
        packaged = release.package(target, self.out, self.root)
        appimage = next(path for path in packaged if path.suffix == ".AppImage")
        self.assertEqual(appimage.stat().st_mode & 0o777, 0o755)
        self.assertTrue(os.access(appimage, os.X_OK))

    def test_refuses_symlink_input(self):
        target = "x86_64-unknown-linux-gnu"
        executable, _ = self.platform_build(target)
        actual = executable.with_name("real-conn")
        executable.rename(actual)
        try:
            executable.symlink_to(actual)
        except OSError:
            self.skipTest("Creating symlinks is unavailable")
        with self.assertRaises(release.ReleaseError):
            release.package(target, self.out, self.root)

    def test_finalize_requires_exact_matrix_and_checksums_every_asset(self):
        self.complete_assets()
        manifest = (self.out / "SHA256SUMS").read_text()
        self.assertEqual(len(manifest.splitlines()), 13)
        for line in manifest.splitlines():
            digest, filename = line.split("  ")
            self.assertEqual(digest, release.sha256(self.out / filename))
        self.assertNotIn("release-notes.md", manifest)
        (self.out / "private.txt").write_text("do not publish")
        with self.assertRaises(release.ReleaseError):
            release.finalize(self.out, self.tag, self.sha, self.root)
        (self.out / "private.txt").unlink()
        next(self.out.glob("*.dmg")).unlink()
        with self.assertRaises(release.ReleaseError):
            release.finalize(self.out, self.tag, self.sha, self.root)

    def test_notes_are_the_template_filled_with_this_release(self):
        self.complete_assets()
        notes = (self.out / "release-notes.md").read_text(encoding="utf-8")
        self.assertTrue(notes.startswith(f"# Conn {self.tag}\n"))
        self.assertIn(f"Source commit: `{self.sha}`", notes)
        for target in release.TARGETS:
            for name in release.asset_names(self.version, target):
                self.assertIn(f"({release.REPOSITORY}/releases/download/{self.tag}/{name})", notes)
        self.assertNotIn("x86_64-apple-darwin", notes)
        self.assertIn(f"{release.REPOSITORY}/blob/{self.tag}/CHANGELOG.md", notes)
        # Nothing of the template's own machinery is published.
        self.assertNotIn("<!--", notes)
        self.assertEqual(re.findall(r"\{[a-z_]+\}", notes.replace("{braces}", "")), [])
        self.assertNotIn("Source commit", release.release_notes(self.root, self.version, self.tag))

    def test_notes_quote_only_this_version_of_the_changelog(self):
        notes = release.release_notes(self.root, self.version, self.tag, self.sha)
        self.assertIn("- First fixture change with `code {braces}` kept as written.", notes)
        self.assertIn("한국어: 픽스처 변경 사항입니다.", notes)
        self.assertNotIn("Unreleased", notes)
        self.assertNotIn("Older fixture change", notes)
        self.assertNotIn(f"## {self.version}", notes)
        # Repository paths would be dead links on the release page; other links stay as written.
        self.assertIn(f"[the protocol]({release.REPOSITORY}/blob/{self.tag}/docs/protocol.md)", notes)
        self.assertIn("[the site](https://example.com/)", notes)
        self.assertEqual(notes.count("First fixture change"), 1)

    def test_changelog_must_have_a_section_for_the_version(self):
        path = self.root / "CHANGELOG.md"
        original = path.read_text(encoding="utf-8")
        heading = f"## {self.version} — Preview · 2026-01-02"
        self.assertEqual(release.check(self.root, self.tag), self.version)  # `## Unreleased` on top is fine
        broken = [
            ("has no", original.replace(heading, "## 99.0.0 — Preview")),
            ("has no", original.replace(heading, f"## {self.version}1 — a longer number is another version")),
            ("has no", original.replace(heading, "## Unreleased")),
            ("more than one", original + f"\n## {self.version}\n\n- Again.\n"),
            ("is empty", original.replace("- First fixture", "## 0.0.2\n\n- First fixture")),
        ]
        for message, text in broken:
            with self.subTest(message=message):
                path.write_text(text, encoding="utf-8")
                with self.assertRaisesRegex(release.ReleaseError, f"CHANGELOG.md.*{message}"):
                    release.check(self.root, self.tag)
        path.unlink()
        with self.assertRaisesRegex(release.ReleaseError, "CHANGELOG.md not found"):
            release.check(self.root)

    def test_template_mistakes_fail_instead_of_being_published(self):
        template = Path(self.temporary.name) / "notes.md"
        template.write_text("<!-- maintainer comment -->\n# {tag}\n{changes}\n{typo}\n", encoding="utf-8")
        with self.assertRaisesRegex(release.ReleaseError, r"unknown placeholder \{typo\}"):
            release.release_notes(self.root, self.version, self.tag, template=template)
        template.write_text("# {tag}\n", encoding="utf-8")
        with self.assertRaisesRegex(release.ReleaseError, "changes"):
            release.release_notes(self.root, self.version, self.tag, template=template)
        template.write_text("<!-- maintainer comment -->\n# {tag}\n{changes}\n", encoding="utf-8")
        notes = release.release_notes(self.root, self.version, self.tag, template=template)
        self.assertTrue(notes.startswith(f"# {self.tag}\n- First fixture change"))

    def test_template_links_into_the_repository_exist(self):
        template = release.NOTES_TEMPLATE.read_text(encoding="utf-8")
        paths = re.findall(r"\{repository\}/blob/\{tag\}/([^)\s]+)", template)
        self.assertGreater(len(paths), 3)
        for path in paths:
            with self.subTest(path=path):
                self.assertTrue((ROOT / path).is_file())

    def test_finalize_rejects_invalid_commit_identity(self):
        self.complete_assets()
        with self.assertRaises(release.ReleaseError):
            release.finalize(self.out, self.tag, "main", self.root)

    def github_answers(self, existing=None, commit=None):
        return [json.dumps({"object": {"type": "commit", "sha": commit or self.sha}}), json.dumps([[existing] if existing else []]), "", ""]

    def test_draft_creates_only_unpublished_prerelease_with_allowlisted_uploads(self):
        self.complete_assets()
        with patch.object(release, "github", side_effect=self.github_answers()) as github:
            release.draft(self.out, self.tag, self.sha, self.root)
        create = github.call_args_list[2].args
        self.assertEqual(create[:3], ("release", "create", self.tag))
        self.assertIn("--draft", create)
        self.assertIn("--prerelease", create)
        self.assertIn("--verify-tag", create)
        upload = github.call_args_list[3].args
        self.assertNotIn(str((self.out / "release-notes.md").resolve()), upload)
        expected = {str((self.out / name).resolve()) for name in release.release_names(self.version)}
        expected.add(str((self.out / "SHA256SUMS").resolve()))
        self.assertEqual(set(upload[upload.index("--clobber") + 1:]), expected)
        self.assertEqual(len(expected), 14)

    @unittest.skipUnless(os.name == "posix", "POSIX directory symlinks unavailable")
    def test_draft_uploads_resolved_paths_through_a_tempdir_symlink(self):
        self.complete_assets()
        alias = self.out.with_name("assets-alias")
        alias.symlink_to(self.out, target_is_directory=True)
        with patch.object(release, "github", side_effect=self.github_answers()) as github:
            release.draft(alias, self.tag, self.sha, self.root)
        upload = github.call_args_list[3].args
        expected = {str((alias / name).resolve()) for name in release.release_names(self.version)}
        expected.add(str((alias / "SHA256SUMS").resolve()))
        self.assertEqual(set(upload[upload.index("--clobber") + 1:]), expected)

    def test_draft_retry_edits_draft_but_refuses_published_release(self):
        self.complete_assets()
        existing = {"tag_name": self.tag, "draft": True, "assets": []}
        with patch.object(release, "github", side_effect=self.github_answers(existing)) as github:
            release.draft(self.out, self.tag, self.sha, self.root)
        self.assertEqual(github.call_args_list[2].args[:2], ("release", "edit"))
        existing["draft"] = False
        with patch.object(release, "github", side_effect=self.github_answers(existing)) as github:
            with self.assertRaises(release.ReleaseError):
                release.draft(self.out, self.tag, self.sha, self.root)
        self.assertEqual(github.call_count, 2)

    def test_draft_rejects_modified_artifact_before_any_network_call(self):
        self.complete_assets()
        next(self.out.glob("*.zip")).write_bytes(b"modified after checksums")
        with patch.object(release, "github") as github:
            with self.assertRaises(release.ReleaseError):
                release.draft(self.out, self.tag, self.sha, self.root)
        github.assert_not_called()

    def test_draft_rejects_a_different_remote_tag_commit(self):
        self.complete_assets()
        with patch.object(release, "github", side_effect=self.github_answers(commit="b" * 40)) as github:
            with self.assertRaises(release.ReleaseError):
                release.draft(self.out, self.tag, self.sha, self.root)
        self.assertEqual(github.call_count, 1)

    def test_draft_accepts_annotated_tag_at_matching_commit(self):
        self.complete_assets()
        answers = [json.dumps({"object": {"type": "tag", "sha": "b" * 40}}), *self.github_answers()]
        with patch.object(release, "github", side_effect=answers) as github:
            release.draft(self.out, self.tag, self.sha, self.root)
        self.assertEqual(github.call_count, 5)

    def test_missing_or_failed_notarization_evidence_blocks_finalization(self):
        self.complete_assets()
        report = next(self.out.glob("*-signing.json"))
        original = report.read_text()
        report.unlink()
        with self.assertRaises(release.ReleaseError):
            release.finalize(self.out, self.tag, self.sha, self.root)
        report.write_text(original)
        payload = json.loads(original)
        payload["notarization"]["cli"]["status"] = "Invalid"
        report.write_text(json.dumps(payload))
        with self.assertRaises(release.ReleaseError):
            release.finalize(self.out, self.tag, self.sha, self.root)

    def test_signing_evidence_requires_matching_commit_runtime_and_tickets(self):
        self.complete_assets()
        path = next(self.out.glob("*-signing.json"))
        original = path.read_text()
        changes = [lambda r: r.update(sourceCommit="b" * 40),
                   lambda r: r["app"]["sidecar"].update(hardenedRuntime=False),
                   lambda r: r["app"].update(stapled=False),
                   lambda r: r["dmg"].update(containedAppVerified=False),
                   lambda r: r["cli"].update(stapled=True)]
        for change in changes:
            report = json.loads(original)
            change(report)
            path.write_text(json.dumps(report))
            with self.assertRaises(release.ReleaseError):
                release.finalize(self.out, self.tag, self.sha, self.root)

    def test_cli_archive_must_contain_the_verified_signed_binary(self):
        self.complete_assets()
        path = next(self.out.glob("*-signing.json"))
        report = json.loads(path.read_text())
        report["cli"]["sha256"] = "0" * 64
        path.write_text(json.dumps(report))
        with self.assertRaises(release.ReleaseError):
            release.finalize(self.out, self.tag, self.sha, self.root)

    def test_resummed_artifact_does_not_bypass_signature_evidence(self):
        self.complete_assets()
        next(self.out.glob("*.dmg")).write_bytes(b"changed after notarization")
        # Even a newly computed generic checksum cannot override signed-build evidence.
        checksums = "".join(f"{release.sha256(self.out / name)}  {name}\n" for name in release.release_names(self.version))
        (self.out / "SHA256SUMS").write_text(checksums)
        with patch.object(release, "github") as github:
            with self.assertRaises(release.ReleaseError):
                release.draft(self.out, self.tag, self.sha, self.root)
        github.assert_not_called()

    def test_packaged_cli_smoke_uses_extracted_binary_and_checks_version(self):
        target = "x86_64-unknown-linux-gnu"
        self.platform_build(target)
        release.package(target, self.out, self.root)
        result = subprocess.CompletedProcess([], 0, stdout=f"conn {self.version}\n", stderr="")
        with patch.object(release.subprocess, "run", return_value=result) as run:
            release.smoke(target, self.out, self.root)
        call = run.call_args
        self.assertEqual(call.args[0][1:], ["--version"])
        self.assertNotIn(str(self.root), call.args[0][0])
        if os.name != "nt":
            self.assertEqual(call.kwargs["env"]["PATH"], "/usr/bin:/bin")
        result.stdout = "conn 99.0.0\n"
        with patch.object(release.subprocess, "run", return_value=result), self.assertRaises(release.ReleaseError):
            release.smoke(target, self.out, self.root)


if __name__ == "__main__":
    unittest.main()
