import importlib.util
from pathlib import Path
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("conn_ci_scope", ROOT / "scripts/ci_scope.py")
ci_scope = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci_scope)
ALL, LINUX = ci_scope.ALL, ci_scope.LINUX


class CiScopeTests(unittest.TestCase):
    def test_documentation_only_changes_skip_native_work(self):
        for event in ("pull_request", "push"):
            self.assertEqual(ci_scope.scope(event, ["README.md", "docs/faq.ko.md", "docs/assets/demo.webp", "media/demo/README.md"]), {"rust": [], "desktop": []})

    def test_the_install_script_is_checked_by_the_light_job_only(self):
        self.assertEqual(ci_scope.scope("pull_request", ["scripts/install.sh", "README.md"]), {"rust": [], "desktop": []})
        self.assertEqual(ci_scope.scope("push", ["site/src/components/Landing.astro", "site/package-lock.json"]), {"rust": [], "desktop": []})
        self.assertEqual(ci_scope.scope("pull_request", ["scripts/release.py"])["rust"], ALL)

    def test_markdown_compiled_into_binaries_is_not_documentation(self):
        self.assertEqual(ci_scope.classify("plugin/skills/conn/SKILL.md"), "native")
        self.assertEqual(ci_scope.scope("pull_request", ["plugin/skills/conn/SKILL.md"])["rust"], ALL)

    def test_frontend_only_changes_build_one_desktop_and_skip_rust(self):
        self.assertEqual(ci_scope.scope("pull_request", ["frontends/tauri/src/App.svelte", "docs/faq.md"]), {"rust": [], "desktop": [LINUX]})

    def test_the_npm_workspace_root_is_frontend_but_shared_packages_are_not_assumed_light(self):
        self.assertEqual(ci_scope.scope("pull_request", ["package-lock.json", "package.json", ".npmrc"]), {"rust": [], "desktop": [LINUX]})
        self.assertEqual(ci_scope.classify("packages/themes/builtin-themes.json"), "native")

    def test_rust_pull_requests_test_everywhere_but_build_one_desktop(self):
        self.assertEqual(ci_scope.scope("pull_request", ["crates/core/src/ipc.rs"]), {"rust": ALL, "desktop": [LINUX]})

    def test_platform_specific_paths_get_every_desktop_before_merging(self):
        for path in ("frontends/tauri/src-tauri/src/lib.rs", ".github/workflows/ci.yml", "crates/frontend/src/automation.rs", "scripts/check_macos_scripting.py"):
            self.assertEqual(ci_scope.scope("pull_request", [path]), {"rust": ALL, "desktop": ALL}, path)

    def test_code_merged_to_main_always_gets_the_full_matrix(self):
        self.assertEqual(ci_scope.scope("push", ["crates/core/src/ipc.rs"]), {"rust": ALL, "desktop": ALL})
        self.assertEqual(ci_scope.scope("push", ["Cargo.lock"]), {"rust": ALL, "desktop": ALL})

    def test_unknown_paths_unknown_diffs_releases_and_the_label_are_never_light(self):
        full = {"rust": ALL, "desktop": ALL}
        self.assertEqual(ci_scope.scope("pull_request", ["some/new/top-level/thing"])["rust"], ALL)
        self.assertEqual(ci_scope.scope("pull_request", None), full)
        self.assertEqual(ci_scope.scope("workflow_call", ["README.md"]), full)
        self.assertEqual(ci_scope.scope("workflow_dispatch", ["README.md"]), full)
        self.assertEqual(ci_scope.scope("pull_request", ["README.md"], full=True), full)

    def test_command_line_prints_github_outputs(self):
        run = subprocess.run([sys.executable, str(ROOT / "scripts/ci_scope.py"), "--event", "pull_request"], input="README.md\n", text=True, capture_output=True, check=True)
        self.assertEqual(run.stdout.splitlines(), ["rust=[]", "desktop=[]"])


if __name__ == "__main__":
    unittest.main()
