#!/usr/bin/env python3
"""Validate and assemble Conn releases. Python 3.11+, standard library only.

Builds are performed by CI; this script never executes downloaded code or signs
an artifact. Each package contains one compiled executable and public docs only.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = "https://github.com/eggplantiny/conn"
REPOSITORY_SLUG = "eggplantiny/conn"
TARGETS = {
    "x86_64-unknown-linux-gnu": (".deb", ".AppImage"),
    "aarch64-apple-darwin": (".dmg",),
    "x86_64-apple-darwin": (".dmg",),
    "x86_64-pc-windows-msvc": (".exe",),
}
SEMVER = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?")


class ReleaseError(ValueError):
    """A release invariant failed; do not publish the output."""


def read_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def read_toml(path: Path):
    return tomllib.loads(path.read_text(encoding="utf-8"))


def validate_version(version: str) -> str:
    match = SEMVER.fullmatch(version) if isinstance(version, str) else None
    if not match:
        raise ReleaseError(f"Invalid release version: {version!r}")
    if match[4] and any(p.isdigit() and len(p) > 1 and p.startswith("0") for p in match[4].split(".")):
        raise ReleaseError(f"Invalid numeric prerelease identifier: {version!r}")
    return version


def check(root: Path = ROOT, tag: str | None = None) -> str:
    """All independently published Conn components must share one version."""
    cargo = read_toml(root / "Cargo.toml")
    version = validate_version(cargo["workspace"]["package"]["version"])
    if tag is not None and tag != f"v{version}":
        raise ReleaseError(f"Tag {tag!r} must match v{version}")
    declarations = {
        "Cargo.toml: workspace conn-core": cargo["workspace"]["dependencies"]["conn-core"]["version"],
        "frontends/tauri/src-tauri/Cargo.toml": read_toml(root / "frontends/tauri/src-tauri/Cargo.toml")["package"]["version"],
    }
    for name in ("frontends/tauri/package.json", "frontends/tauri/src-tauri/tauri.conf.json", "plugin/.claude-plugin/plugin.json", "plugin/.codex-plugin/plugin.json"):
        declarations[name] = read_json(root / name)["version"]
    npm_lock = read_json(root / "frontends/tauri/package-lock.json")
    declarations["package-lock.json: root"] = npm_lock["version"]
    declarations["package-lock.json: packages root"] = npm_lock["packages"][""]["version"]
    marketplace = read_json(root / ".claude-plugin/marketplace.json")
    plugins = [p for p in marketplace["plugins"] if p["name"] == "conn"]
    if len(plugins) != 1:
        raise ReleaseError("Claude marketplace must contain exactly one Conn plugin")
    declarations[".claude-plugin/marketplace.json"] = plugins[0]["version"]
    expected_packages = {
        "Cargo.lock": {"conn", "conn-core", "conn-frontend", "conn-browser-harness"},
        "frontends/tauri/src-tauri/Cargo.lock": {"conn-desktop", "conn-core", "conn-frontend"},
    }
    for name, expected in expected_packages.items():
        packages = [p for p in read_toml(root / name)["package"] if p["name"] in expected]
        if {p["name"] for p in packages} != expected or len(packages) != len(expected):
            raise ReleaseError(f"{name}: missing or duplicate Conn packages")
        for package in packages:
            declarations[f"{name}: {package['name']}"] = package["version"]
    mismatches = [f"{name}: {actual!r}" for name, actual in declarations.items() if actual != version]
    if mismatches:
        raise ReleaseError(f"Version must be {version} everywhere:\n" + "\n".join(mismatches))
    return version


def asset_names(version: str, target: str) -> list[str]:
    validate_version(version)
    if target not in TARGETS:
        raise ReleaseError(f"Unsupported target: {target!r}")
    base = f"conn-v{version}-{target}"
    archive = ".zip" if "windows" in target else ".tar.gz"
    return [f"{base}-cli{archive}"] + [f"{base}-{'setup' if ext == '.exe' else 'desktop'}{ext}" for ext in TARGETS[target]]


def regular_file(path: Path, boundary: Path) -> Path:
    if path.is_symlink() or not path.is_file():
        raise ReleaseError(f"Expected a regular file: {path}")
    if not path.resolve().is_relative_to(boundary.resolve()):
        raise ReleaseError(f"File escapes its expected directory: {path}")
    if path.stat().st_size == 0:
        raise ReleaseError(f"Empty artifact: {path}")
    return path


def cli_readme(version: str, target: str) -> bytes:
    return f"""Conn v{version} — {target}

This archive contains the Conn CLI and MIT license. The desktop application is
a separate release asset. Put conn (conn.exe on Windows) on your PATH, then run
conn --help. The MCP server uses `conn mcp` and needs a running Conn session.

Getting started: {REPOSITORY}/blob/v{version}/docs/getting-started.md
Korean guide: {REPOSITORY}/blob/v{version}/docs/getting-started.ko.md
Platform policy: {REPOSITORY}/blob/v{version}/docs/platform-support.md
Security: {REPOSITORY}/blob/v{version}/SECURITY.md

Conn is preview software. A shell command runs with your account's permissions;
Conn's approval and policy controls are not an operating-system sandbox. Review
commands before approval. An executed entry means the input was sent to the
shell; it does not guarantee that the command succeeded.
""".encode("utf-8")


def write_cli_archive(destination: Path, executable: Path, license_path: Path, readme: bytes):
    # Fixed metadata and a fixed allowlist avoid timestamps and private files.
    members = [(executable.name, executable.read_bytes(), 0o755), ("LICENSE", license_path.read_bytes(), 0o644), ("README.txt", readme, 0o644)]
    if destination.suffix == ".zip":
        with zipfile.ZipFile(destination, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
            for name, data, mode in members:
                info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
                info.create_system = 3
                info.external_attr = (0o100000 | mode) << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                archive.writestr(info, data)
    else:
        with destination.open("wb") as stream, gzip.GzipFile(filename="", mode="wb", fileobj=stream, mtime=0) as compressed, tarfile.open(fileobj=compressed, mode="w") as archive:
            for name, data, mode in members:
                info = tarfile.TarInfo(name)
                info.size, info.mode, info.mtime = len(data), mode, 0
                archive.addfile(info, io.BytesIO(data))


def package(target: str, out: Path, root: Path = ROOT) -> list[Path]:
    version = check(root)
    names = asset_names(version, target)
    executable = root / "target" / target / "release" / ("conn.exe" if "windows" in target else "conn")
    regular_file(executable, root / "target")
    license_path = regular_file(root / "LICENSE", root)
    bundle_dir = root / "frontends/tauri/src-tauri/target" / target / "release/bundle"
    bundles = []
    for extension in TARGETS[target]:
        # Never mistake the bare desktop .exe for the NSIS installer.
        candidates = list((bundle_dir / "nsis").glob("*.exe")) if extension == ".exe" else list(bundle_dir.rglob(f"*{extension}"))
        if len(candidates) != 1:
            raise ReleaseError(f"Expected exactly one {extension} bundle for {target}, found {len(candidates)} in {bundle_dir}")
        bundles.append(regular_file(candidates[0], bundle_dir))
    out = out.resolve()
    for source in [executable, license_path, *bundles]:
        if source.resolve().is_relative_to(out):
            raise ReleaseError("Output directory must not contain release build inputs")
    if out.exists() and any((out / name).exists() for name in names):
        raise ReleaseError("Output assets already exist; use a fresh output directory")
    out.mkdir(parents=True, exist_ok=True)
    write_cli_archive(out / names[0], executable, license_path, cli_readme(version, target))
    for source, name in zip(bundles, names[1:]):
        shutil.copy2(source, out / name)
    return [out / name for name in names]


def sha256(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def finalize(artifacts: Path, tag: str, sha: str | None = None, root: Path = ROOT) -> list[Path]:
    version = check(root, tag)
    if sha is not None and re.fullmatch(r"[0-9a-fA-F]{40}", sha) is None:
        raise ReleaseError("--sha must be a full 40-character Git commit SHA")
    expected = sorted(name for target in TARGETS for name in asset_names(version, target))
    if not artifacts.is_dir():
        raise ReleaseError(f"Artifact directory not found: {artifacts}")
    found = {p.name for p in artifacts.iterdir()}
    extras = found - set(expected) - {"SHA256SUMS", "release-notes.md"}
    missing = set(expected) - found
    if extras or missing:
        raise ReleaseError(f"Release asset mismatch; missing={sorted(missing)}, unexpected={sorted(extras)}")
    paths = [regular_file(artifacts / name, artifacts) for name in expected]
    checksums = "".join(f"{sha256(path)}  {path.name}\n" for path in paths)
    for output_name in ("SHA256SUMS", "release-notes.md"):
        if (artifacts / output_name).is_symlink():
            raise ReleaseError(f"Refusing to overwrite a symlink: {output_name}")
    (artifacts / "SHA256SUMS").write_text(checksums, encoding="utf-8", newline="\n")
    source = f"\nSource commit: `{sha.lower()}`\n" if sha else ""
    notes = f"""# Conn {tag}

**Preview / prerelease.** Conn gives a human and an agent one shared shell with
visible control handoff, command approval, and a combined collaboration timeline.
These binaries are built by CI; a green build is not a claim of full manual
validation on every operating system.
{source}
## Install

- [English guide]({REPOSITORY}/blob/{tag}/docs/getting-started.md)
- [한국어 사용법]({REPOSITORY}/blob/{tag}/docs/getting-started.ko.md)
- [What changed]({REPOSITORY}/blob/{tag}/CHANGELOG.md)

Choose the desktop installer for your platform, or the `-cli` archive for a CLI
and MCP workflow. Apple Silicon uses `aarch64-apple-darwin`; Intel Macs use
`x86_64-apple-darwin`. Linux x64 assets are built on Ubuntu 24.04 and target
Ubuntu 24.04/26.04; verify these exact assets on both systems before publishing.
Windows targets x64 and is intentionally unsigned for this preview; a certificate
is not a release prerequisite. SmartScreen or unknown-publisher prompts may appear.
macOS packages are currently ad-hoc signed, not notarized: keep this draft
unpublished until the Apple signing handoff and native checks are complete.
Update these notes to match the resulting signatures before public distribution.
See the [platform policy]({REPOSITORY}/blob/{tag}/docs/platform-support.md).
Follow the installation guide for platform trust prompts; never disable system
protection globally.

## Verify the download

Download `SHA256SUMS` with your selected asset. On Linux, run
`sha256sum --ignore-missing -c SHA256SUMS` from that directory. On macOS, compare
`shasum -a 256 <asset>` to the matching line; on Windows use
`Get-FileHash <asset> -Algorithm SHA256`. SHA-256 detects download corruption;
the checksum file is not a code-signing signature.

## Before giving an agent control

Commands use your account's permissions. Conn's policy and approval UI are not
an operating-system sandbox. An “executed” record means input reached the shell,
not that the command completed successfully. Start with a disposable project.
Read the [security model]({REPOSITORY}/blob/{tag}/docs/security.md) and
[report security issues privately]({REPOSITORY}/security/advisories/new).

한국어: 이 릴리스는 프리뷰입니다. 명령은 사용자 계정 권한으로 실행되며, 승인 기능은
운영체제 샌드박스가 아닙니다. 실행 기록은 입력 전달을 뜻하며 명령의 성공을 보장하지
않습니다. macOS 패키지는 임시 서명(ad-hoc)을 사용하며 공증되지 않았습니다.
Windows 프리뷰는 무서명으로 배포하며 인증서가 필수 조건은 아닙니다. Linux는
Ubuntu 24.04 빌드를 24.04·26.04에서 검증합니다. Mac 공개 전 서명·공증과 네이티브
검증을 완료하고 실제 서명 상태에 맞게 이 초안 안내를 수정하세요.
"""
    (artifacts / "release-notes.md").write_text(notes, encoding="utf-8", newline="\n")
    return paths + [artifacts / "SHA256SUMS"]


def github(*args: str):
    """Structured subprocess arguments only; never interpolate into a shell."""
    command = ["gh", *args]
    if args[0] == "api":
        command.extend(["--hostname", "github.com"])
    result = subprocess.run(command, text=True, capture_output=True, check=False)
    if result.returncode != 0:
        # gh's errors can contain environment data; keep failed API logs minimal.
        raise ReleaseError(f"GitHub CLI failed for {args[0]!r} (exit {result.returncode})")
    return result.stdout


def draft(artifacts: Path, tag: str, sha: str, root: Path = ROOT) -> str:
    """Upload only to a draft, never publish or replace a public release."""
    version = check(root, tag)
    if re.fullmatch(r"[0-9a-fA-F]{40}", sha) is None:
        raise ReleaseError("--sha must be a full 40-character Git commit SHA")
    expected = sorted(name for target in TARGETS for name in asset_names(version, target))
    if not artifacts.is_dir() or {p.name for p in artifacts.iterdir()} != set(expected) | {"SHA256SUMS", "release-notes.md"}:
        raise ReleaseError("Expected only the complete finalized asset set; run finalize first")
    assets = [regular_file(artifacts / name, artifacts) for name in expected]
    checksums = regular_file(artifacts / "SHA256SUMS", artifacts)
    if checksums.read_text(encoding="utf-8") != "".join(f"{sha256(path)}  {path.name}\n" for path in assets):
        raise ReleaseError("SHA256SUMS does not match the artifacts; refusing upload")
    notes = regular_file(artifacts / "release-notes.md", artifacts)
    obj = json.loads(github("api", f"repos/{REPOSITORY_SLUG}/git/ref/tags/{tag}"))["object"]
    # Peel annotated tags; a tag must resolve to this build's exact commit.
    for _ in range(4):
        if obj["type"] != "tag":
            break
        if re.fullmatch(r"[0-9a-fA-F]{40}", obj["sha"]) is None:
            raise ReleaseError("Remote tag contains an invalid object SHA")
        obj = json.loads(github("api", f"repos/{REPOSITORY_SLUG}/git/tags/{obj['sha']}"))["object"]
    if obj["type"] != "commit" or obj["sha"].lower() != sha.lower():
        raise ReleaseError("Remote release tag does not resolve to this build's commit")
    pages = json.loads(github("api", f"repos/{REPOSITORY_SLUG}/releases", "--paginate", "--slurp"))
    matches = [release for page in pages for release in page if release["tag_name"] == tag]
    if len(matches) > 1:
        raise ReleaseError("Multiple matching releases; inspect the repository manually")
    if matches and not matches[0]["draft"]:
        raise ReleaseError("The release is already public; refusing any changes")
    allowed_uploads = set(expected) | {"SHA256SUMS"}
    if matches and {a["name"] for a in matches[0].get("assets", [])} - allowed_uploads:
        raise ReleaseError("Existing draft contains unexpected assets; review them manually")
    action = "edit" if matches else "create"
    arguments = ["release", action, tag, "--repo", REPOSITORY_SLUG, "--draft", "--prerelease", "--title", f"Conn {tag} (preview)", "--notes-file", str(notes.resolve())]
    if not matches:
        arguments.append("--verify-tag")
    github(*arguments)
    github("release", "upload", tag, "--repo", REPOSITORY_SLUG, "--clobber", *(str(path.resolve()) for path in [*assets, checksums]))
    return f"{REPOSITORY}/releases (draft {tag}; publishing requires manual review)"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("check", help="Check version consistency").add_argument("--tag")
    pack = sub.add_parser("package", help="Collect one platform's prebuilt release assets")
    pack.add_argument("--target", required=True, choices=TARGETS)
    pack.add_argument("--out", type=Path, required=True)
    finish = sub.add_parser("finalize", help="Validate the complete platform matrix and make checksums/notes")
    finish.add_argument("--artifacts", type=Path, required=True)
    finish.add_argument("--tag", required=True)
    finish.add_argument("--sha")
    publish = sub.add_parser("draft", help="Upload verified assets to an unpublished GitHub prerelease draft")
    publish.add_argument("--artifacts", type=Path, required=True)
    publish.add_argument("--tag", required=True)
    publish.add_argument("--sha", required=True)
    args = parser.parse_args(argv)
    try:
        if args.command == "check":
            print(f"Version declarations agree: {check(tag=args.tag)}")
        elif args.command == "package":
            for path in package(args.target, args.out):
                print(path)
        elif args.command == "finalize":
            for path in finalize(args.artifacts, args.tag, args.sha):
                print(path)
        else:
            print(draft(args.artifacts, args.tag, args.sha))
        return 0
    except (ReleaseError, OSError, KeyError, ValueError, tomllib.TOMLDecodeError) as error:
        print(f"Release validation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
