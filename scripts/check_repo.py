#!/usr/bin/env python3
"""Check public-source hygiene and relative Markdown links (Python 3.11+).

In a Git checkout, only tracked files are checked, including staged additions.
Without Git, public source is walked with local/build artifacts excluded; this
fallback is useful for an extracted source archive but cannot certify Git's index.
"""
from __future__ import annotations

import argparse
from html.parser import HTMLParser
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
PRUNED_DIRS = {".git", ".local-setup", "node_modules", "target", "dist", "__pycache__", ".pytest_cache", ".venv", "venv", "coverage", "release-artifacts", "artifacts"}
LOCAL_FILES = {"SETUP.local.md", "design-qa.md", "test-file.md", "testfile.md", ".DS_Store", "Thumbs.db"}
GENERATED = {"frontends/tauri/src-tauri/binaries", "frontends/tauri/src-tauri/gen"}
SECRET_PATTERNS = [
    re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----"),
    re.compile(r"\bgh[pousr]_[A-Za-z0-9]{36,}\b"),
    re.compile(r"\bgithub_pat_[A-Za-z0-9_]{60,}\b"),
    re.compile(r"\bAKIA[0-9A-Z]{16}\b"),
]
TEXT_SUFFIXES = {".md", ".py", ".rs", ".ts", ".js", ".mjs", ".svelte", ".json", ".toml", ".yaml", ".yml", ".sh", ".txt", ".html", ".css", ".svg", ".lock"}


def private_path(relative: str) -> str | None:
    """Paths that must never enter the public source archive."""
    path = PurePosixPath(relative)
    if path.is_absolute() or ".." in path.parts:
        return "path escapes the repository"
    if any(part in PRUNED_DIRS for part in path.parts):
        return "machine-local or generated directory"
    if path.name in LOCAL_FILES or path.name.endswith((".log", ".sock", ".socket", ".pid", ".sqlite", ".sqlite3", ".db", ".pem", ".key", ".p12", ".pfx")):
        return "machine-local state or private key"
    if path.name == ".env" or path.name.startswith(".env.") and not path.name.endswith((".example", ".template", ".sample")):
        return "environment secrets file"
    if path.name.startswith("audit") and path.suffix == ".jsonl":
        return "private shell audit history"
    if any(relative == prefix or relative.startswith(prefix + "/") for prefix in GENERATED):
        return "generated sidecar or Tauri metadata"
    return None


def source_files(root: Path) -> tuple[list[Path], str]:
    try:
        result = subprocess.run(["git", "rev-parse", "--show-toplevel"], cwd=root, text=True, capture_output=True, check=False)
        # Do not accidentally audit a parent checkout's unrelated index.
        if result.returncode == 0 and Path(result.stdout.strip()).resolve() == root.resolve():
            tracked = subprocess.run(["git", "ls-files", "-z", "--cached"], cwd=root, capture_output=True, check=True).stdout
            names = sorted({name.decode("utf-8") for name in tracked.split(b"\0") if name})
            return [root / name for name in names], "git-index"
    except (OSError, subprocess.CalledProcessError):
        pass
    result = []
    def walk(directory: Path):
        for path in sorted(directory.iterdir()):
            relative = path.relative_to(root).as_posix()
            if private_path(relative):
                continue
            if path.is_symlink() or path.is_file():
                result.append(path)
            elif path.is_dir():
                walk(path)
    walk(root)
    return result, "source-walk (local/build files excluded; Git index unavailable)"


def prose(markdown: str) -> str:
    """Ignore fenced and inline code: paths in command examples are not links."""
    lines = []
    fence = None
    for line in markdown.splitlines():
        marker = re.match(r"^\s*(`{3,}|~{3,})", line)
        if marker:
            if fence is None:
                fence = (marker[1][0], len(marker[1]))
            elif marker[1][0] == fence[0] and len(marker[1]) >= fence[1]:
                fence = None
            lines.append("")
        else:
            lines.append(line if fence is None else "")
    return re.sub(r"(`+).*?\1", "", "\n".join(lines))


def markdown_targets(markdown: str) -> list[str]:
    text = prose(markdown)
    # Markdown destinations may contain spaces when enclosed in angle brackets.
    inline = re.findall(r"!?\[[^\]\n]*\]\(\s*(<[^>\n]+>|[^\s)]+)(?:\s+['\"][^\n]*?['\"])?\s*\)", text)
    reference = re.findall(r"^\s*\[[^\]\n]+\]:\s*(<[^>\n]+>|\S+)", text, re.MULTILINE)
    class HTMLLinks(HTMLParser):
        def __init__(self):
            super().__init__(convert_charrefs=True)
            self.targets = []

        def handle_starttag(self, tag, attrs):
            self.targets.extend(value for name, value in attrs if name in {"href", "src"} and value is not None)

    html = HTMLLinks()
    html.feed(text)
    return [target.strip("<>") for target in inline + reference] + html.targets


def markdown_errors(path: Path, root: Path, markdown: str, public_paths: set[Path] | None = None) -> list[str]:
    errors = []
    for target in markdown_targets(markdown):
        if target.startswith(("#", "/")) or urlsplit(target).scheme:
            continue
        destination = unquote(urlsplit(target).path)
        if not destination:
            continue
        resolved = (path.parent / destination).resolve()
        if not resolved.is_relative_to(root.resolve()):
            errors.append(f"relative link escapes source tree: {target}")
        elif not resolved.exists():
            errors.append(f"broken relative link: {target}")
        elif resolved.is_file() and public_paths is not None and resolved not in public_paths:
            errors.append(f"relative link points to an untracked/private file: {target}")
    return errors


def check(root: Path = ROOT) -> tuple[list[str], str, int]:
    root = root.resolve()
    files, mode = source_files(root)
    public_paths = {path.resolve() for path in files if private_path(path.relative_to(root).as_posix()) is None}
    errors = []
    for path in files:
        relative = path.relative_to(root).as_posix()
        reason = private_path(relative)
        if reason:
            errors.append(f"{relative}: {reason}")
            continue
        if path.is_symlink():
            errors.append(f"{relative}: source symlinks are not allowed in the release inventory")
            continue
        if not path.is_file():
            errors.append(f"{relative}: tracked file is missing from the working tree")
            continue
        if path.suffix.lower() not in TEXT_SUFFIXES and path.name not in {"LICENSE", ".gitignore", ".gitattributes"}:
            continue
        try:
            content = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            errors.append(f"{relative}: expected UTF-8 text")
            continue
        if any(pattern.search(content) for pattern in SECRET_PATTERNS):
            # Never include a matched credential in logs.
            errors.append(f"{relative}: possible credential or private key; inspect locally")
        if path.suffix.lower() == ".md":
            errors.extend(f"{relative}: {error}" for error in markdown_errors(path, root, content, public_paths))
    return errors, mode, len(files)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args(argv)
    try:
        errors, mode, count = check(args.root)
    except (OSError, ValueError) as error:
        print(f"Repository check failed: {error}", file=sys.stderr)
        return 1
    print(f"Checked {count} source files via {mode}")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Relative Markdown links and public-source hygiene passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
