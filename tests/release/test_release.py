"""Release contract tests; no network or platform toolchain required."""
import importlib.util
import json
import os
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
    "Cargo.toml", "Cargo.lock", "frontends/tauri/src-tauri/Cargo.toml",
    "frontends/tauri/src-tauri/Cargo.lock", "frontends/tauri/package.json",
    "frontends/tauri/package-lock.json", "frontends/tauri/src-tauri/tauri.conf.json",
    "plugin/.claude-plugin/plugin.json", "plugin/.codex-plugin/plugin.json",
    ".claude-plugin/marketplace.json",
]


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
        self.out.mkdir()
        for target in release.TARGETS:
            for name in release.asset_names(self.version, target):
                (self.out / name).write_bytes(name.encode())
        release.finalize(self.out, self.tag, self.sha, self.root)

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

    def test_numeric_semver_and_path_traversal_rejected(self):
        for version in ("01.2.3", "1.2", "1.2.3-01", "1.2.3/../../secret", "1.2.3\n", "1.2.3\u0661"):
            with self.subTest(version=version), self.assertRaises(release.ReleaseError):
                release.validate_version(version)
        with self.assertRaises(release.ReleaseError):
            release.asset_names(self.version, "../../secret")

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
        self.assertEqual(len(manifest.splitlines()), 9)
        for line in manifest.splitlines():
            digest, filename = line.split("  ")
            self.assertEqual(digest, release.sha256(self.out / filename))
        notes = (self.out / "release-notes.md").read_text()
        self.assertIn("Preview / prerelease", notes)
        self.assertIn("getting-started.ko.md", notes)
        self.assertNotIn("release-notes.md", manifest)
        (self.out / "private.txt").write_text("do not publish")
        with self.assertRaises(release.ReleaseError):
            release.finalize(self.out, self.tag, self.sha, self.root)
        (self.out / "private.txt").unlink()
        next(self.out.glob("*.dmg")).unlink()
        with self.assertRaises(release.ReleaseError):
            release.finalize(self.out, self.tag, self.sha, self.root)

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
        self.assertEqual(len([arg for arg in upload if arg.startswith(str(self.out))]), 10)

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


if __name__ == "__main__":
    unittest.main()
