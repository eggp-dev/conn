#!/usr/bin/env python3
"""Decide how much CI a change needs. Python 3.11+, standard library only.

Reads changed paths (one per line) on stdin and prints the runner lists for the
Rust and desktop jobs as JSON. Only paths known to be documentation or web
frontend are treated lightly; anything unrecognized gets the full native matrix,
and so does any run without a reliable diff (releases, manual runs, errors).
"""

from __future__ import annotations

import argparse
import json
import sys

LINUX = "ubuntu-24.04"
ALL = [LINUX, "macos-15", "windows-2022"]

# Read by people only. `plugin/**/*.md` is compiled into the binaries and is NOT documentation here.
# `site/` is the website (conn.eggp.dev); the always-on job builds it.
DOCS_PREFIXES = ("docs/", "media/", "site/", ".github/ISSUE_TEMPLATE/")
DOCS_ROOT_FILES = ("LICENSE", "NOTICE")
# Not part of any build. The always-on job checks the installer (`sh -n`); Dependabot's
# configuration is read by GitHub, not by a workflow, so it needs no native runner either.
LIGHT_FILES = ("scripts/install.sh", ".github/dependabot.yml")
# Shared Svelte UI, thin native/web adapters and browser integration tests.
# The native shell under src-tauri is excluded below.
FRONTEND_PREFIXES = ("packages/ui/", "frontends/tauri/", "frontends/web/", "tests/collaboration/")
# The npm workspace root: one lockfile for the UI, the site and the films. No Rust reads it.
FRONTEND_ROOT_FILES = ("package.json", "package-lock.json", ".npmrc")
NATIVE_SHELL_PREFIX = "frontends/tauri/src-tauri/"
# Platform-specific packaging, scripting and CI itself: worth every desktop runner before merging.
PLATFORM_PREFIXES = (NATIVE_SHELL_PREFIX, ".github/", "crates/frontend/src/automation", "scripts/check_macos_scripting.py", "scripts/macos_")


def classify(path: str) -> str:
    """Return docs, frontend, platform or native for one repository path."""
    if path.startswith(PLATFORM_PREFIXES) and not path.startswith(DOCS_PREFIXES) and path not in LIGHT_FILES:
        return "platform"
    if path.startswith(DOCS_PREFIXES) or path in DOCS_ROOT_FILES or path in LIGHT_FILES or ("/" not in path and path.endswith(".md")):
        return "docs"
    if path.startswith(FRONTEND_PREFIXES) or path in FRONTEND_ROOT_FILES:
        return "frontend"
    return "native"


def scope(event: str, paths: list[str] | None, full: bool = False) -> dict[str, list[str]]:
    """Runner lists per job. `paths=None` means the diff is unknown."""
    if full or paths is None or event not in ("pull_request", "push"):
        return {"rust": ALL, "desktop": ALL}
    kinds = {classify(p) for p in paths if p.strip()}
    if not kinds or kinds <= {"docs"}:
        return {"rust": [], "desktop": []}
    if kinds <= {"docs", "frontend"}:
        # The UI is identical on every platform; one native build proves it still bundles.
        return {"rust": [], "desktop": [LINUX]}
    if event == "push" or "platform" in kinds:
        return {"rust": ALL, "desktop": ALL}
    # A pull request touching Rust: test everywhere, build the desktop once.
    # The merge to main and every release still build all three desktops.
    return {"rust": ALL, "desktop": [LINUX]}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--event", required=True)
    parser.add_argument("--full", action="store_true", help="Force the full matrix (label, release, manual run)")
    parser.add_argument("--unknown-diff", action="store_true", help="The changed paths could not be determined")
    args = parser.parse_args(argv)
    paths = None if args.unknown_diff else [line.rstrip("\n") for line in sys.stdin]
    result = scope(args.event, paths, args.full)
    for job, runners in result.items():
        print(f"{job}={json.dumps(runners)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
