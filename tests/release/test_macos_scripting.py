"""Portable checks for the macOS dictionary and its metadata-only CI smoke."""
import importlib.util
import json
from pathlib import Path
import plistlib
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("conn_scripting_check", ROOT / "scripts/check_macos_scripting.py")
scripting = importlib.util.module_from_spec(spec)
spec.loader.exec_module(scripting)


class ScriptingTests(unittest.TestCase):
    def test_dictionary_native_adapter_and_bundle_metadata_agree(self):
        commands = scripting.validate_adapter()
        self.assertIn("create session", commands.values())
        self.assertIn("create window with default profile", commands.values())
        self.assertIn("write text", commands.values())
        self.assertIn("request state", commands.values())
        with (scripting.NATIVE / "Info.plist").open("rb") as file:
            info = plistlib.load(file)
        self.assertIs(info["NSAppleScriptEnabled"], True)
        self.assertEqual(info["OSAScriptingDefinition"], "Conn.sdef")
        config = json.loads((scripting.NATIVE / "tauri.conf.json").read_text())
        self.assertEqual(config["bundle"]["macOS"]["files"]["Resources/Conn.sdef"], "Conn.sdef")

    def test_packaged_examples_use_private_launch_and_input_contract(self):
        self.assertEqual({p.name for p in scripting.EXAMPLES}, {
            "external-connect.applescript", "external-window.applescript",
        })
        for example in scripting.EXAMPLES:
            self.assertTrue(example.is_file())
        source = (ROOT / "examples/applescript/external-connect.applescript").read_text()
        self.assertIn('write text inputLine to session sessionID', source)
        self.assertIn('{"cancelled", "failed"}', source)
        self.assertNotIn('"denied"', source)
        self.assertNotIn('detail.proposed', source)
        window = (ROOT / "examples/applescript/external-window.applescript").read_text()
        self.assertIn('create window with default profile command "/bin/sh -c', window)
        self.assertIn('__RUN_COMMAND__', window)

    def test_dictionary_preserves_legacy_intent_only_as_ignored_input(self):
        definition = scripting.ET.fromstring((scripting.NATIVE / "Conn.sdef").read_text())
        write = next(c for c in definition.findall('.//command') if c.get('name') == 'write text')
        intent = next(p for p in write.findall('parameter') if p.get('name') == 'intent')
        self.assertEqual(intent.get('code'), 'intn')
        self.assertEqual(intent.get('optional'), 'yes')
        self.assertIn('ignored and discarded', intent.get('description'))
        state = next(c for c in definition.findall('.//command') if c.get('name') == 'request state')
        self.assertIn('queued, delivering, delivered, cancelled or failed', state.get('description'))

    def test_dictionary_rejects_duplicate_or_missing_event_codes(self):
        for xml in ('<dictionary/>', '<dictionary><command name="write" code="wrte"/></dictionary>',
                    '<dictionary><command name="one" code="Connwrte"/><command name="two" code="Connwrte"/></dictionary>'):
            with self.subTest(xml=xml), self.assertRaises(scripting.ScriptingError):
                scripting.command_codes(xml)

    def test_compile_targets_exact_bundle_and_rejects_unrelated_examples(self):
        app = Path('/tmp/Conn "CI".app')
        for target in ('"Conn"', 'id "dev.eggp.conn"'):
            source = scripting.target_example(f'tell application {target}\ncreate session\nend tell\n', app)
            self.assertIn('application "/tmp/Conn \\"CI\\".app"', source)
        with self.assertRaises(scripting.ScriptingError):
            scripting.target_example('tell application "Terminal"\nactivate\nend tell', app)

    def test_smoke_checks_packaged_dictionary_and_compiles_without_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            app = Path(directory) / "Conn.app"
            resources = app / "Contents/Resources"
            resources.mkdir(parents=True)
            definition = scripting.NATIVE / "Conn.sdef"
            (resources / "Conn.sdef").write_bytes(definition.read_bytes())
            example = Path(directory) / "example.applescript"
            example.write_text('tell application "Conn"\ncreate session\nend tell\n')
            calls = []

            def run(*args):
                calls.append(args)
                if args[0] == "plutil":
                    return "true" if args[2] == "NSAppleScriptEnabled" else "Conn.sdef"
                if args[0] == "sdef":
                    return definition.read_text()
                if args[0] == "osacompile":
                    self.assertIn(str(app.resolve()), Path(args[-1]).read_text())
                    Path(args[2]).write_bytes(b"compiled script fixture")
                return ""

            with patch.object(scripting, "run", side_effect=run):
                scripting.check_bundle(app, [example, *scripting.EXAMPLES])
            self.assertEqual({args[0] for args in calls}, {"plutil", "sdef", "codesign", "osacompile"})
            self.assertEqual(sum(args[0] == "osacompile" for args in calls), 3)
            # A missing dictionary in the .app must fail before compilation.
            (resources / "Conn.sdef").write_text("stale")
            with patch.object(scripting, "run", side_effect=run), self.assertRaises(scripting.ScriptingError):
                scripting.check_bundle(app, example)

    def test_mac_ci_bundles_the_debug_app_without_release_credentials(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        self.assertIn("--debug --bundles app", workflow)
        self.assertIn("scripts/check_macos_scripting.py --app", workflow)
        self.assertIn("APPLE_SIGNING_IDENTITY: '-'", workflow)
        self.assertNotIn("secrets.", workflow)


if __name__ == "__main__":
    unittest.main()
