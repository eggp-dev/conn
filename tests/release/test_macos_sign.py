"""Native signing contracts tested without invoking macOS tools or credentials."""
import base64
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("release", ROOT / "scripts/release.py")
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)
sys.modules.setdefault("release", release)
spec = importlib.util.spec_from_file_location("macos_sign", ROOT / "scripts/macos_sign.py")
signing = importlib.util.module_from_spec(spec)
spec.loader.exec_module(signing)

TEAM = "ABCDE12345"
IDENTITY = f"Developer ID Application: Example Fixture ({TEAM})"
FINGERPRINT = "A" * 40
SIGNATURE = f"""Executable=/sample/conn
CodeDirectory v=20500 size=123 flags=0x10000(runtime) hashes=24+7 location=embedded
CDHash={'1' * 40}
Authority={IDENTITY}
Authority=Developer ID Certification Authority
Timestamp=Sep 16, 2026 at 12:00:00
TeamIdentifier={TEAM}
"""


class SigningTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.path = Path(self.temporary.name) / "signing"
        self.env = {
            "RUNNER_TEMP": self.temporary.name, "RUNNER_OS": "macOS", "GITHUB_ACTIONS": "true",
            "GITHUB_REPOSITORY": release.REPOSITORY_SLUG, "GITHUB_EVENT_NAME": "push", "GITHUB_REF": "refs/tags/v0.3.0",
            "APPLE_CERTIFICATE": base64.b64encode(b"fixture-p12-not-a-real-key").decode(),
            "APPLE_CERTIFICATE_PASSWORD": "fixture-certificate-password", "APPLE_SIGNING_IDENTITY": IDENTITY,
            "APPLE_ID": "fixture@example.invalid", "APPLE_PASSWORD": "fixture-notary-password", "APPLE_TEAM_ID": TEAM,
        }
        self.calls = []

    def command(self, args, label, timeout=120):
        self.calls.append((args, label))
        if args[:2] == ["security", "create-keychain"]:
            Path(args[-1]).write_text("fixture-keychain")
        if args[:2] == ["security", "find-identity"]:
            return SimpleNamespace(stdout=f' 1) {FINGERPRINT} "{IDENTITY}"\n', stderr="")
        if args == ["security", "list-keychains", "-d", "user"]:
            return SimpleNamespace(stdout='"/sample/login.keychain-db"\n', stderr="")
        return SimpleNamespace(stdout="", stderr="")

    def test_secrets_identity_and_matching_team_are_mandatory(self):
        for name in signing.SECRET_NAMES:
            with self.subTest(name=name), self.assertRaises(release.ReleaseError):
                signing.require_secrets({**self.env, name: ""})
        for identity in ("-", "Apple Development: Fixture (ABCDE12345)", "Developer ID Application: Fixture (ZZZZZ99999)"):
            with self.subTest(identity=identity), self.assertRaises(release.ReleaseError):
                signing.validate_identity(identity, TEAM)
        signing.require_secrets(self.env)

    def test_only_trusted_mac_tag_or_main_dispatch_can_use_secrets(self):
        with patch.object(signing.sys, "platform", "darwin"):
            signing.require_runner(self.env)
            signing.require_runner({**self.env, "GITHUB_EVENT_NAME": "workflow_dispatch", "GITHUB_REF": "refs/heads/main"})
            for change in ({"GITHUB_EVENT_NAME": "pull_request"}, {"GITHUB_REPOSITORY": "fork/conn"},
                           {"RUNNER_OS": "Linux"}, {"GITHUB_ACTIONS": "false"},
                           {"GITHUB_EVENT_NAME": "workflow_dispatch", "GITHUB_REF": "refs/heads/feature"}):
                with self.subTest(change=change), self.assertRaises(release.ReleaseError):
                    signing.require_runner({**self.env, **change})

    def test_temporary_keychain_password_and_certificate_cleanup(self):
        with patch.object(signing.sys, "platform", "darwin"), patch.object(signing, "command", side_effect=self.command):
            signing.prepare(self.path, self.env)
            self.assertFalse((self.path / "certificate.p12").exists())
            self.assertEqual((self.path / "state.json").stat().st_mode & 0o777, 0o600)
            state = json.loads((self.path / "state.json").read_text())
            self.assertEqual(state["fingerprint"], FINGERPRINT)
            create = next(args for args, _ in self.calls if args[:2] == ["security", "create-keychain"])
            self.assertGreaterEqual(len(create[3]), 48)
            self.assertNotIn(create[3], self.env.values())
            self.assertNotIn(create[3], (self.path / "state.json").read_text())
            signing.cleanup(self.path, self.env)
        self.assertFalse(self.path.exists())
        self.assertTrue(any(args[:2] == ["security", "delete-keychain"] for args, _ in self.calls))
        self.assertIn((["security", "list-keychains", "-d", "user", "-s", "/sample/login.keychain-db"], "Restore keychain search list"), self.calls)

    def test_failed_import_still_removes_p12_and_cleanup_removes_keychain(self):
        def fail_import(args, label, timeout=120):
            if args[:2] == ["security", "import"]:
                raise release.ReleaseError("Import failed")
            return self.command(args, label, timeout)
        with patch.object(signing.sys, "platform", "darwin"), patch.object(signing, "command", side_effect=fail_import):
            with self.assertRaises(release.ReleaseError):
                signing.prepare(self.path, self.env)
            self.assertFalse((self.path / "certificate.p12").exists())
            signing.cleanup(self.path, self.env)
        self.assertFalse(self.path.exists())

    def test_cleanup_refuses_paths_outside_runner_temp(self):
        with patch.object(signing.sys, "platform", "darwin"), self.assertRaises(release.ReleaseError):
            signing.cleanup(Path(self.temporary.name).parent, self.env)
        self.assertTrue(Path(self.temporary.name).exists())

    def test_signature_requires_developer_id_team_runtime_and_secure_timestamp(self):
        self.assertTrue(signing.parse_signature(SIGNATURE, TEAM)["hardenedRuntime"])
        changes = [SIGNATURE.replace(TEAM, "ZZZZZ99999"), SIGNATURE.replace("Authority=Developer ID Application", "Authority=Apple Development"),
                   SIGNATURE.replace("flags=0x10000(runtime)", "flags=0x0(none)"),
                   SIGNATURE.replace("Timestamp=", "Signed Time="), SIGNATURE.replace("CDHash=", "Missing=")]
        for text in changes:
            with self.subTest(text=text), self.assertRaises(release.ReleaseError):
                signing.parse_signature(text, TEAM)
        dmg = SIGNATURE.replace("flags=0x10000(runtime)", "flags=0x0(none)")
        self.assertFalse(signing.parse_signature(dmg, TEAM, runtime=False)["hardenedRuntime"])

    def test_notarization_requires_accepted_status_and_submission_uuid(self):
        valid = {"status": "Accepted", "id": "12345678-1234-1234-1234-123456789abc"}
        self.assertEqual(signing.accepted_submission(valid), valid)
        for invalid in ({**valid, "status": "Invalid"}, {**valid, "status": "In Progress"}, {**valid, "id": "missing"}, {}):
            with self.assertRaises(release.ReleaseError):
                signing.accepted_submission(invalid)

    def test_failed_command_does_not_expose_args_stdout_stderr_or_apple_environment(self):
        result = subprocess.CompletedProcess([], 1, "private stdout", "private stderr")
        with patch.dict(signing.os.environ, {"APPLE_PASSWORD": "private-env"}), patch.object(signing.subprocess, "run", return_value=result) as run:
            with self.assertRaises(release.ReleaseError) as failure:
                signing.command(["security", "-p", "private-argument"], "Fixed stage")
        self.assertEqual(str(failure.exception), "Fixed stage failed (exit 1); no release artifacts will be uploaded")
        self.assertNotIn("APPLE_PASSWORD", run.call_args.kwargs["env"])

    def test_pipeline_scopes_secrets_to_macos_and_always_cleans_up(self):
        workflow = (ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("if: always() && runner.os == 'macOS'", workflow)
        self.assertNotIn("APPLE_SIGNING_IDENTITY: '-'", workflow)
        self.assertNotIn("secrets.KEYCHAIN_PASSWORD", workflow)
        self.assertIn("scripts/release.py smoke --target", workflow)
        self.assertLess(workflow.index("macos_sign.py cleanup"), workflow.index("actions/upload-artifact"))


if __name__ == "__main__":
    unittest.main()
