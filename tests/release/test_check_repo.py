import importlib.util
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("conn_check_repo", ROOT / "scripts/check_repo.py")
check_repo = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check_repo)


class RepositoryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_private_paths_and_safe_environment_templates(self):
        for path in (".env", ".env.local", "target/debug/conn", "a/node_modules/b.js", "test-file.md", "SETUP.local.md", "audit.jsonl", "keys/signing.pem", "../outside"):
            with self.subTest(path=path):
                self.assertIsNotNone(check_repo.private_path(path))
        for path in (".env.example", "docs/security.md", "tests/core.rs", ".github/workflows/release.yml"):
            self.assertIsNone(check_repo.private_path(path))

    def test_markdown_links_skip_command_examples_but_check_real_destinations(self):
        readme = self.root / "README.md"
        guide = self.root / "guide.md"
        guide.write_text("# Guide")
        source = """[Guide](guide.md#start) [Web](https://example.com) [Email](mailto:a@example.com)
`[example](missing.md)`
```sh
[example](another-missing.md)
```
[broken](absent.md)
[reference]: missing-reference.md
"""
        errors = check_repo.markdown_errors(readme, self.root, source)
        self.assertEqual(len(errors), 2)
        self.assertTrue(any("absent.md" in error for error in errors))
        self.assertTrue(any("missing-reference.md" in error for error in errors))

    def test_relative_links_cannot_escape_source_or_link_private_files(self):
        readme = self.root / "README.md"
        local = self.root / "SETUP.local.md"
        local.write_text("private")
        errors = check_repo.markdown_errors(readme, self.root, "[outside](../outside.md) [local](SETUP.local.md)", set())
        self.assertEqual(len(errors), 2)

    def test_encoded_and_angle_bracket_paths_work(self):
        (self.root / "My Guide.md").write_text("guide")
        errors = check_repo.markdown_errors(self.root / "README.md", self.root, "[a](My%20Guide.md) [b](<My Guide.md>)")
        self.assertEqual(errors, [])

    def test_html_links_and_images_are_checked_except_in_code_examples(self):
        (self.root / "guide.md").write_text("guide")
        source = '''<a href="guide.md">Guide</a><img src="missing.svg" />
<a href="absent.md">Missing</a><img src="https://example.com/image.svg">
```html
<img src="example-only.svg">
```
'''
        errors = check_repo.markdown_errors(self.root / "README.md", self.root, source)
        self.assertEqual(len(errors), 2)
        self.assertTrue(any("missing.svg" in error for error in errors))
        self.assertTrue(any("absent.md" in error for error in errors))

    def test_source_walk_excludes_local_data_and_detects_secret_without_printing_it(self):
        (self.root / "README.md").write_text("# Public")
        (self.root / "test-file.md").write_text("private test")
        (self.root / ".env").write_text("TOKEN=local")
        token = "ghp_" + "a" * 36
        (self.root / "config.json").write_text('{"token":"' + token + '"}')
        errors, mode, count = check_repo.check(self.root)
        self.assertIn("source-walk", mode)
        self.assertEqual(count, 2)
        self.assertEqual(len(errors), 1)
        self.assertNotIn(token, errors[0])

    @unittest.skipUnless(shutil.which("git"), "Git unavailable")
    def test_git_index_including_staged_files_rejects_private_sources(self):
        subprocess.run(["git", "init", "--quiet", str(self.root)], check=True)
        (self.root / "README.md").write_text("# Public")
        (self.root / ".env").write_text("TOKEN=local")
        (self.root / "test-file.md").write_text("private")
        subprocess.run(["git", "-C", str(self.root), "add", "README.md", ".env", "test-file.md"], check=True)
        errors, mode, count = check_repo.check(self.root)
        self.assertEqual(mode, "git-index")
        self.assertEqual(count, 3)
        self.assertEqual(len(errors), 2)


if __name__ == "__main__":
    unittest.main()
