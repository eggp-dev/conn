"""Updater archive contracts; no credentials or native signing tools are used."""
import importlib.util
import json
import os
from pathlib import Path
import sys
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]


def load(name):
    if name in sys.modules:
        return sys.modules[name]
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


release = load("release")
signing = load("macos_sign")
updater = load("updater_artifacts")


class UpdaterArchiveTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.target = "aarch64-apple-darwin"
        self.version = "0.6.0"
        self.app = self.root / "frontends/tauri/src-tauri/target" / self.target / "release/bundle/macos/Conn.app"
        executable = self.app / "Contents/MacOS/conn-desktop"
        executable.parent.mkdir(parents=True)
        executable.write_bytes(b"synthetic executable")
        executable.chmod(0o755)
        license_file = self.app / "Contents/Resources/LICENSE"
        license_file.parent.mkdir()
        license_file.write_text("MIT License", encoding="utf-8")
        self.out = self.root / "assets"
        self.out.mkdir()
        self.report = self.out / release.signing_name(self.version, self.target)
        self.report.write_text(json.dumps({"app": {"stapled": True}}))

    def package(self, evidence):
        with patch.object(release, "check", return_value=self.version), \
             patch.object(signing, "app_evidence", side_effect=evidence), \
             patch.dict(os.environ, {"TAURI_SIGNING_PRIVATE_KEY": "not-a-key-fixture"}), \
             patch.object(updater.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as signer, \
             patch.object(release, "signature_text"):
            self.signer = signer
            updater.package(self.target, self.out, self.root)
            return signer

    def test_archive_preserves_payload_and_verifies_extracted_app_before_signing(self):
        inspected = []

        def evidence(app, _team):
            inspected.append(app)
            executable = app / "Contents/MacOS/conn-desktop"
            self.assertEqual(executable.read_bytes(), b"synthetic executable")
            self.assertEqual((app / "Contents/Resources/LICENSE").read_text(), "MIT License")
            if os.name != "nt":
                self.assertEqual(executable.stat().st_mode & 0o777, 0o755)
            return {"stapled": True}

        signer = self.package(evidence)
        self.assertEqual(len(inspected), 2)
        self.assertEqual(inspected[0], self.app)
        self.assertNotEqual(inspected[1], self.app)
        self.assertFalse(inspected[1].exists(), "Extracted verification copy must be removed")
        report = release.read_json(self.report)["updater"]
        artifact = self.out / report["archive"]
        self.assertEqual(report["sha256"], release.sha256(artifact))
        self.assertTrue(report["containedAppVerified"])
        with tarfile.open(artifact) as archive:
            self.assertIn("Conn.app/Contents/MacOS/conn-desktop", archive.getnames())
        self.assertNotIn("not-a-key-fixture", str(signer.call_args))

    def test_lost_notarization_evidence_blocks_signing(self):
        with self.assertRaisesRegex(release.ReleaseError, "Packaged updater app differs"):
            self.package([{"stapled": True}, {"stapled": False}])
        self.signer.assert_not_called()
        self.assertNotIn("updater", release.read_json(self.report))


if __name__ == "__main__":
    unittest.main()
