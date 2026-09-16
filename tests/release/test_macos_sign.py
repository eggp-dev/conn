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
            imported = next(args for args, _ in self.calls if args[:2] == ["security", "import"])
            self.assertEqual(imported[imported.index("-f") + 1], "pkcs12")
            self.assertNotIn("-A", imported)
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

    def test_dmg_license_prompt_is_answered_without_mutating_image_and_mount_is_detached(self):
        dmg = Path(self.temporary.name) / "Conn.dmg"
        dmg.write_bytes(b"signed-dmg-fixture")
        expected = {key: {"cdhash": "1" * 40} for key in ("bundle", "desktop", "sidecar")}

        def native_command(args, **kwargs):
            if args[:2] == ["hdiutil", "attach"]:
                # A licensed DMG refuses an unattended attach without an answer.
                if kwargs.get("input") != "Y\n":
                    return subprocess.CompletedProcess(args, 1, "License prompt", "")
                self.assertIn("-plist", args)
                self.assertIn("-readonly", args)
                mount = Path(args[args.index("-mountpoint") + 1])
                (mount / "Conn.app").mkdir()
            else:
                self.assertEqual(args[:2], ["hdiutil", "detach"])
                self.assertIsNone(kwargs.get("input"))
            return subprocess.CompletedProcess(args, 0, "", "")

        for invalid in (False, True):
            with self.subTest(invalid_app=invalid):
                received = {**expected, "sidecar": {"cdhash": ("2" if invalid else "1") * 40}}
                with patch.object(signing.subprocess, "run", side_effect=native_command) as run, \
                        patch.object(signing, "app_evidence", return_value=received):
                    if invalid:
                        with self.assertRaisesRegex(release.ReleaseError, "different app or sidecar"):
                            signing.inspect_dmg(dmg, TEAM, expected)
                    else:
                        self.assertTrue(signing.inspect_dmg(dmg, TEAM, expected))
                self.assertEqual([call.args[0][:2] for call in run.call_args_list],
                                 [["hdiutil", "attach"], ["hdiutil", "detach"]])
                self.assertEqual(dmg.read_bytes(), b"signed-dmg-fixture")

    def test_failed_command_does_not_expose_args_stdout_stderr_or_apple_environment(self):
        result = subprocess.CompletedProcess([], 1, "private stdout", "private stderr")
        with patch.dict(signing.os.environ, {"APPLE_PASSWORD": "private-env"}), patch.object(signing.subprocess, "run", return_value=result) as run:
            with self.assertRaises(release.ReleaseError) as failure:
                signing.command(["security", "-p", "private-argument"], "Fixed stage")
        self.assertEqual(str(failure.exception), "Fixed stage failed (exit 1); no release artifacts will be uploaded")
        self.assertNotIn("APPLE_PASSWORD", run.call_args.kwargs["env"])

    def test_import_errors_expose_only_fixed_diagnostic_categories(self):
        cases = {
            "MAC verification failed during PKCS12 import (wrong password?)":
                "pkcs12-verification (password, container integrity or algorithm compatibility)",
            "Unknown format in import": "pkcs12-format-or-decoding",
            "Import/Export format unsupported": "pkcs12-format-or-decoding",
            "Unable to decode the provided data": "pkcs12-format-or-decoding",
            "User interaction is not allowed": "keychain-access",
            "Write permissions error": "keychain-access",
            "The specified keychain could not be found": "keychain-access",
            "Unrecognized native failure": "unclassified-import-error",
        }
        for native_message, category in cases.items():
            with self.subTest(category=category):
                result = subprocess.CompletedProcess([], 1, "private stdout", f"private-prefix {native_message} private-suffix")
                with patch.object(signing.subprocess, "run", return_value=result):
                    with self.assertRaises(release.ReleaseError) as failure:
                        signing.command(["security", "import", "private-path", "-P", "private-password"], "Import signing certificate")
                self.assertEqual(str(failure.exception),
                                 f"Import signing certificate failed (exit 1); category: {category}; no release artifacts will be uploaded")

    def test_pipeline_scopes_secrets_to_macos_and_always_cleans_up(self):
        workflow = (ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("if: always() && runner.os == 'macOS'", workflow)
        self.assertNotIn("APPLE_SIGNING_IDENTITY: '-'", workflow)
        self.assertNotIn("secrets.KEYCHAIN_PASSWORD", workflow)
        self.assertIn("scripts/release.py smoke --target", workflow)
        self.assertEqual(signing.MAC_TARGETS, ("aarch64-apple-darwin",))
        self.assertNotIn("x86_64-apple-darwin", workflow)
        self.assertNotIn("macos-15-intel", workflow)
        self.assertLess(workflow.index("macos_sign.py cleanup"), workflow.index("actions/upload-artifact"))


if __name__ == "__main__":
    unittest.main()
