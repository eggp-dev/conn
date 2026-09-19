#!/usr/bin/env python3
"""Validate and assemble Conn releases. Python 3.11+, standard library only.

Builds are performed by CI; this script never executes downloaded code or signs
an artifact. Each package contains one compiled executable and public docs only.
"""
from __future__ import annotations

import argparse
import base64
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import uuid
import zipfile

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = "https://github.com/eggplantiny/conn"
REPOSITORY_SLUG = "eggplantiny/conn"
TARGETS = {
    "x86_64-unknown-linux-gnu": (".deb", ".AppImage"),
    "aarch64-apple-darwin": (".dmg",),
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


def signing_name(version: str, target: str) -> str:
    asset_names(version, target)
    if not target.endswith("apple-darwin"):
        raise ReleaseError("Signing evidence is required only for macOS targets")
    return f"conn-v{version}-{target}-signing.json"


def update_asset(version: str, target: str) -> str:
    names = asset_names(version, target)
    if target.endswith("apple-darwin"):
        return f"conn-v{version}-{target}-desktop.app.tar.gz"
    return next(n for n in names if n.endswith((".AppImage", "-setup.exe")))


def updater_names(version: str, target: str) -> list[str]:
    name = update_asset(version, target)
    return ([name] if target.endswith("apple-darwin") else []) + [name + ".sig"]


def signature_text(path: Path) -> str:
    text = path.read_text(encoding="utf-8").strip()
    try:
        lines = base64.b64decode(text, validate=True).decode().splitlines()
        if len(lines) != 4 or not lines[0].startswith("untrusted comment:") or not lines[2].startswith("trusted comment:"):
            raise ValueError()
        if len(base64.b64decode(lines[1], validate=True)) != 74 or len(base64.b64decode(lines[3], validate=True)) != 64:
            raise ValueError()
    except (ValueError, UnicodeError):
        raise ReleaseError("Invalid updater signature format") from None
    return text


def release_names(version: str) -> list[str]:
    return sorted([name for target in TARGETS for name in asset_names(version, target) + updater_names(version, target)] +
                  [signing_name(version, target) for target in TARGETS if target.endswith("apple-darwin")] + ["latest.json"])


def updater_manifest(artifacts: Path, version: str):
    platforms = {"aarch64-apple-darwin": "darwin-aarch64", "x86_64-unknown-linux-gnu": "linux-x86_64", "x86_64-pc-windows-msvc": "windows-x86_64"}
    entries = {}
    for target, platform in platforms.items():
        name = update_asset(version, target)
        regular_file(artifacts / name, artifacts)
        signature = signature_text(regular_file(artifacts / (name + ".sig"), artifacts))
        if target.endswith("apple-darwin"):
            evidence = read_json(artifacts / signing_name(version, target)).get("updater", {})
            if evidence != {"archive": name, "sha256": sha256(artifacts / name), "containedAppVerified": True}:
                raise ReleaseError("macOS updater archive lacks matching notarized-app evidence")
        entries[platform] = {"url": f"{REPOSITORY}/releases/download/v{version}/{name}", "signature": signature}
    return {"version": version, "platforms": entries}


def validate_signing_report(path: Path, version: str, target: str, artifacts: Path, sha: str | None = None):
    """Validate native CI evidence and bind it to the exact uploaded bytes.

    The report records macOS runner checks; it is not an independent signature or
    attestation. Apple code signatures and notarization tickets remain in binaries.
    """
    report = read_json(regular_file(path, artifacts))
    if report.get("schemaVersion") != 1 or report.get("version") != version or report.get("target") != target:
        raise ReleaseError("Signing evidence version/target does not match the release")
    if re.fullmatch(r"[0-9a-f]{40}", report.get("sourceCommit", "")) is None or (sha and report["sourceCommit"] != sha.lower()):
        raise ReleaseError("Signing evidence does not match the source commit")
    def signature(value, runtime=True):
        if not isinstance(value, dict) or any(value.get(key) is not True for key in ("developerId", "teamVerified", "secureTimestamp")):
            raise ReleaseError("Missing Developer ID, Team or timestamp verification")
        if value.get("hardenedRuntime") is not runtime or re.fullmatch(r"[0-9a-f]{40,64}", value.get("cdhash", "")) is None:
            raise ReleaseError("Missing hardened-runtime or CodeDirectory verification")
    app, cli, dmg = report.get("app", {}), report.get("cli", {}), report.get("dmg", {})
    for part in ("bundle", "desktop", "sidecar"):
        signature(app.get(part))
    signature(cli)
    signature(dmg, runtime=False)
    if any(app.get(key) is not True for key in ("stapled", "gatekeeperAccepted")) or any(dmg.get(key) is not True for key in ("stapled", "gatekeeperAccepted", "containedAppVerified")):
        raise ReleaseError("Missing app/DMG ticket or Gatekeeper verification")
    if cli.get("stapled") is not False:
        raise ReleaseError("Standalone CLI evidence must not claim unsupported stapling")
    for kind in ("cli", "dmg"):
        submission = report.get("notarization", {}).get(kind, {})
        try:
            uuid.UUID(submission["id"])
        except (KeyError, ValueError, TypeError, AttributeError):
            raise ReleaseError("Missing notarization submission ID") from None
        if submission.get("status") != "Accepted":
            raise ReleaseError("Apple notarization must be Accepted for CLI and DMG")
    expected = asset_names(version, target)
    if not isinstance(report.get("assets"), dict) or set(report["assets"]) != set(expected):
        raise ReleaseError("Signing evidence must cover exactly this target's release assets")
    for name in expected:
        if report["assets"][name] != sha256(regular_file(artifacts / name, artifacts)):
            raise ReleaseError("Release bytes changed after signing/notarization verification")
    # The signed/notarized standalone binary must be the one inside the tarball.
    with tarfile.open(artifacts / expected[0], "r:gz") as archive:
        if archive.getnames() != ["conn", "LICENSE", "README.txt"] or not archive.getmember("conn").isfile():
            raise ReleaseError("Unexpected macOS CLI archive contents")
        stream = archive.extractfile("conn")
        if stream is None or hashlib.file_digest(stream, "sha256").hexdigest() != cli.get("sha256"):
            raise ReleaseError("CLI archive does not contain the verified signed executable")
    return report


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


def smoke(target: str, artifacts: Path, root: Path = ROOT):
    """Run only the extracted CLI, outside the checkout and without toolchain PATH."""
    version = check(root)
    archive = regular_file(artifacts / asset_names(version, target)[0], artifacts)
    filename = "conn.exe" if "windows" in target else "conn"
    with tempfile.TemporaryDirectory(prefix="conn-installed-cli-") as directory:
        executable = Path(directory) / filename
        if archive.suffix == ".zip":
            with zipfile.ZipFile(archive) as zipped:
                if zipped.namelist() != [filename, "LICENSE", "README.txt"]:
                    raise ReleaseError("Unexpected CLI archive contents")
                executable.write_bytes(zipped.read(filename))
        else:
            with tarfile.open(archive, "r:gz") as packed:
                if packed.getnames() != [filename, "LICENSE", "README.txt"] or not packed.getmember(filename).isfile():
                    raise ReleaseError("Unexpected CLI archive contents")
                stream = packed.extractfile(filename)
                if stream is None:
                    raise ReleaseError("CLI archive has no executable")
                executable.write_bytes(stream.read())
        executable.chmod(0o755)
        env = {key: value for key, value in os.environ.items()
               if not key.startswith(("CARGO", "RUST", "NODE", "NPM", "APPLE_"))}
        system_root = env.get("SystemRoot", r"C:\Windows")
        env["PATH"] = os.pathsep.join([str(Path(system_root) / "System32"), system_root]) if os.name == "nt" else "/usr/bin:/bin"
        result = subprocess.run([str(executable), "--version"], cwd=directory, env=env,
                                capture_output=True, text=True, timeout=30, check=False)
        if result.returncode != 0 or result.stdout.strip() != f"conn {version}":
            raise ReleaseError("Standalone packaged CLI did not report the release version")


def finalize(artifacts: Path, tag: str, sha: str | None = None, root: Path = ROOT) -> list[Path]:
    version = check(root, tag)
    if sha is not None and re.fullmatch(r"[0-9a-fA-F]{40}", sha) is None:
        raise ReleaseError("--sha must be a full 40-character Git commit SHA")
    expected = release_names(version)
    if not artifacts.is_dir():
        raise ReleaseError(f"Artifact directory not found: {artifacts}")
    found = {p.name for p in artifacts.iterdir()}
    extras = found - set(expected) - {"SHA256SUMS", "release-notes.md"}
    missing = set(expected) - {"latest.json"} - found
    if extras or missing:
        raise ReleaseError(f"Release asset mismatch; missing={sorted(missing)}, unexpected={sorted(extras)}")
    manifest = updater_manifest(artifacts, version)
    if (artifacts / "latest.json").is_symlink():
        raise ReleaseError("Refusing a symlink updater manifest")
    (artifacts / "latest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    paths = [regular_file(artifacts / name, artifacts) for name in expected]
    for target in TARGETS:
        if target.endswith("apple-darwin"):
            validate_signing_report(artifacts / signing_name(version, target), version, target, artifacts, sha)
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
## Download the desktop app

| Your computer | Download |
|---|---|
| Mac — Apple Silicon (M1 or newer) | [Conn for Apple Silicon]({REPOSITORY}/releases/download/{tag}/{asset_names(version, 'aarch64-apple-darwin')[1]}) |
| Windows — x64 | [Windows installer]({REPOSITORY}/releases/download/{tag}/{asset_names(version, 'x86_64-pc-windows-msvc')[1]}) |
| Ubuntu — x64 | [Ubuntu .deb]({REPOSITORY}/releases/download/{tag}/{asset_names(version, 'x86_64-unknown-linux-gnu')[1]}) · [Linux AppImage]({REPOSITORY}/releases/download/{tag}/{asset_names(version, 'x86_64-unknown-linux-gnu')[2]}) |

Installers include the app and its Conn CLI sidecar; Rust and Node.js are not
required to use these binaries. Windows preview installers are unsigned, so a
SmartScreen or unknown-publisher prompt may appear. Native CLI startup is checked
in CI; full interactive GUI installation/collaboration checks remain pending.

## New in this release

- **Fixed in 0.8.1:** typing while a command approval was showing took control back but left the approval alive, with your keystrokes appended to the agent's typed line; approving afterwards could run a line that differed from the one on the card. Your input now denies the pending approval and clears that line first. If you run 0.8.0, update.
- **Changed from 0.7.0:** agents observe the current terminal screen of their shared session as text, parsed from its output. Window focus, the visible tab, minimizing or covering the window no longer pause reading or writing. There is no screenshot, scrollback or scroll position in a snapshot, and the `unattended` / `suspended` errors are gone.
- Conn now asks once before a new agent connection joins (**Allow / Deny**). Until you allow it, the connection learns nothing about your sessions. The answer applies to that live connection only; you can turn the question off in Settings → Agents.
- Control is one revocable lease per connection: your typing still takes it back at once, and an agent that moves to another tab gives up what it held in the tab it left.
- Text hidden with ANSI conceal or equal foreground/background colors stays out of snapshots, and an agent cannot submit a line that contains it. Passwords typed without echo never appear; secrets a program prints remain visible to participants.
- A terminated agent's delayed command never runs, and an agent whose shell closed can move to another permitted tab.

**Upgrade together:** protocol v2 requires the matching app and CLI/MCP adapter.
After installing, restart your MCP clients. Existing files, profiles and saved activity
remain; running shell sessions close during app restart and are not migrated.
Agents reconnecting with the same name do not inherit explicitly selected access.

v0.6.0, v0.7.0 and v0.8.0 installations can discover this preview through the signed updater. Older
clients need a manual install. Updates support Apple Silicon macOS, Windows x64
and Linux AppImage; Debian packages use package-manager/manual updates. Installation
and restart remain your choice. Full installed-app upgrade coverage on every target
is not claimed by artifact signature checks.

한국어: 0.8.1은 승인 카드가 떠 있을 때 사람이 입력하면 승인이 살아남아, 이후 승인 시 카드와 다른 명령이 실행될 수 있던 문제를 고칩니다. 이제 사람 입력은 대기 중인 승인을 거부하고 입력 줄을 비웁니다. 0.8.0 사용자는 업데이트하세요. 0.7.0과 달라진 점입니다. 에이전트는 공유된 세션의 현재 터미널 화면을 텍스트로
관찰하며, 창 포커스·보이는 탭·최소화 여부는 더 이상 읽기와 쓰기를 멈추지 않습니다.
새 에이전트 연결은 Conn 창에서 한 번 허용해야 참여하고, 허용 전에는 세션에 대해 아무것도
알 수 없습니다(설정 → 에이전트에서 끌 수 있음). 제어권은 연결당 하나이며 사람이 입력하면
즉시 돌아옵니다. 숨김 처리된 글자는 스냅샷에서 빠지고, 에코 없이 입력한 암호는 나타나지
않습니다. 앱·CLI를 함께 업데이트하고 MCP 클라이언트를 재시작하세요. 파일·설정·저장 기록은
유지하며 실행 중인 셸은 앱 재시작 시 종료됩니다. v0.6.0·v0.7.0·v0.8.0은 서명 업데이트를 사용할
수 있고 이전 버전은 직접 설치하세요.

## Native validation and remaining coverage

The actual Apple Silicon development app passed the join prompt, reading and writing
while minimized, unfocused or on another tab, hidden/masked synthetic authentication,
AppleScript launch, same-SSH sharing, late external writes being refused and the
one-lease-per-connection rule. A Linux run against a real OpenSSH server found no
credential in snapshots, replies or saved data after sharing. See the
[macOS acceptance report]({REPOSITORY}/blob/{tag}/docs/shared-surface-macos-validation-results.md).
Native WebKit completion fixtures passed; a real OpenAI request remains unverified.
The real-user provider path stays opt-in. Development-app checks are separate from
this release's CI signing/notarization and updater artifact-signature verification.
Windows interactive GUI and full installed-app upgrades remain unverified.

AppleScript is disabled by default. Enable it for selected local profiles in Settings
and consult the [automation guide]({REPOSITORY}/blob/{tag}/docs/external-automation.md).

## Getting started

- [English guide]({REPOSITORY}/blob/{tag}/docs/getting-started.md)
- [한국어 사용법]({REPOSITORY}/blob/{tag}/docs/getting-started.ko.md)
- [What changed]({REPOSITORY}/blob/{tag}/CHANGELOG.md)

For a separate MCP connector, download the matching CLI for
[Apple Silicon]({REPOSITORY}/releases/download/{tag}/{asset_names(version, 'aarch64-apple-darwin')[0]}),
[Windows x64]({REPOSITORY}/releases/download/{tag}/{asset_names(version, 'x86_64-pc-windows-msvc')[0]}) or
[Linux x64]({REPOSITORY}/releases/download/{tag}/{asset_names(version, 'x86_64-unknown-linux-gnu')[0]}).
Linux x64 assets are built on Ubuntu 24.04 and target
Ubuntu 24.04/26.04; runtime coverage of these exact assets is separate from CI.
Windows targets x64 and is intentionally unsigned for this preview; a certificate
is not a release prerequisite. SmartScreen or unknown-publisher prompts may appear.
macOS apps, their embedded CLI sidecars, and standalone CLIs are Developer ID
signed with hardened runtime and secure timestamps. Apple notarization is
Accepted for the final DMGs and standalone CLI submissions; app and DMG tickets
are stapled and verified. Apple does not support stapling a standalone CLI or
its archive, so its notarization ticket is retrieved online when needed.
The Apple Silicon `-signing.json` asset records native runner checks and final asset hashes;
it is evidence of this build, not an independent cryptographic attestation.
Intel Mac packages are paused for new releases; previously published assets remain available.
Native interactive installation and collaboration coverage remains limited for this preview.
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

한국어: AppleScript는 기본적으로 꺼져 있습니다. 실제 Mac 개발 앱의 인증·공유·제어 전환을 검증했으며 배포 파일 서명·공증과는 구분합니다. Windows GUI와 모든 플랫폼의 실제 설치 상태 업그레이드, 실제 OpenAI 호출은 추가 검증 대상입니다.
이 릴리스는 프리뷰입니다. 명령은 사용자 계정 권한으로 실행되며, 승인 기능은
운영체제 샌드박스가 아닙니다. 실행 기록은 입력 전달을 뜻하며 명령의 성공을 보장하지
않습니다. macOS 앱·내장 CLI·별도 CLI는 Developer ID로 서명하며 hardened runtime과
보안 타임스탬프를 검증합니다. DMG와 별도 CLI의 Apple 공증이 승인되었고 앱·DMG에는
티켓을 첨부했습니다. 별도 CLI와 아카이브에는 티켓을 첨부할 수 없어 필요 시 온라인으로
조회합니다. Apple Silicon 서명 보고서는 해당 빌드의 검증 기록이며 독립적인 암호학적 증명은 아닙니다.
새 릴리스의 Intel Mac 배포는 중단하며 기존 공개 파일은 유지합니다.
Windows 프리뷰는 무서명으로 배포하며 인증서가 필수 조건은 아닙니다. Linux는
Ubuntu 24.04 빌드를 24.04·26.04 대상으로 제공합니다. CI에서 각 운영체제의 별도 CLI
시작을 확인하며, 네이티브 GUI 설치·협업의 전체 대화형 검증은 아직 완료되지 않았습니다.
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
    expected = release_names(version)
    if not artifacts.is_dir() or {p.name for p in artifacts.iterdir()} != set(expected) | {"SHA256SUMS", "release-notes.md"}:
        raise ReleaseError("Expected only the complete finalized asset set; run finalize first")
    assets = [regular_file(artifacts / name, artifacts) for name in expected]
    for target in TARGETS:
        if target.endswith("apple-darwin"):
            validate_signing_report(artifacts / signing_name(version, target), version, target, artifacts, sha)
    if read_json(artifacts / "latest.json") != updater_manifest(artifacts, version):
        raise ReleaseError("Updater manifest does not match the finalized release")
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
    native = sub.add_parser("smoke", help="Run the standalone packaged CLI outside the checkout")
    native.add_argument("--target", required=True, choices=TARGETS)
    native.add_argument("--artifacts", type=Path, required=True)
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
        elif args.command == "smoke":
            smoke(args.target, args.artifacts)
            print(f"Standalone CLI smoke check passed: {args.target}")
        else:
            print(draft(args.artifacts, args.tag, args.sha))
        return 0
    except (ReleaseError, OSError, KeyError, ValueError, tomllib.TOMLDecodeError, tarfile.TarError, subprocess.TimeoutExpired) as error:
        print(f"Release validation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
