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
NOTES_TEMPLATE = Path(__file__).resolve().with_name("release_notes.md")
# The repository address is shared with the site and the films; see packages/brand.
REPOSITORY_SLUG = json.loads((ROOT / "packages/brand/brand.json").read_text(encoding="utf-8"))["repository"]
REPOSITORY = f"https://github.com/{REPOSITORY_SLUG}"
# Do not "fix" this one. Conn 0.6.0 to 0.8.1 only install an update whose download address starts
# with the address they were built with, and they cannot be told otherwise. GitHub keeps serving
# this address for as long as nobody creates a repository with the old name, so the update manifest
# keeps using it. Builds from 0.8.2 on trust both addresses (see src-tauri/src/updates.rs).
UPDATER_REPOSITORY = "https://github.com/eggplantiny/conn"
TARGETS = {
    "x86_64-unknown-linux-gnu": (".deb", ".AppImage"),
    "aarch64-apple-darwin": (".dmg",),
    "x86_64-pc-windows-msvc": (".exe",),
}
# Rust target -> Tauri updater platform key. updater_artifacts.py reads this table too.
PLATFORMS = {"aarch64-apple-darwin": "darwin-aarch64", "x86_64-unknown-linux-gnu": "linux-x86_64", "x86_64-pc-windows-msvc": "windows-x86_64"}
SEMVER = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?")
WORKSPACE_VERSION = "Cargo.toml: workspace package"
JSON_MANIFESTS = ("frontends/tauri/package.json", "frontends/tauri/src-tauri/tauri.conf.json", "plugin/.claude-plugin/plugin.json", "plugin/.codex-plugin/plugin.json")
# Lockfiles list every dependency; only Conn's own packages carry the release version.
LOCKED_PACKAGES = {
    "Cargo.lock": {"conn", "conn-core", "conn-frontend", "conn-browser-harness", "conn-desktop"},
}
# Where `bump` writes what `declarations` reads. Group 1 is the version; each pattern must match exactly once.
_JSON_VERSION = r'^  "version": "([^"\n]+)"'
VERSION_PATTERNS = {
    # The desktop crate inherits the workspace version, so its manifest declares none.
    "Cargo.toml": (r'^\[workspace\.package\]\n(?:(?!\[).*\n)*?version = "([^"\n]+)"', r'^conn-core = \{[^}\n]*\bversion = "([^"\n]+)"', r'^conn-frontend = \{[^}\n]*\bversion = "([^"\n]+)"'),
    **{name: (_JSON_VERSION,) for name in JSON_MANIFESTS},
    # One npm workspace lockfile at the root; the desktop UI is the only workspace that carries the release version.
    "package-lock.json": (r'^    "frontends/tauri": \{\n(?:      .*\n)*?      "version": "([^"\n]+)"',),
    ".claude-plugin/marketplace.json": (r'^      "name": "conn",\n(?:      .*\n)*?      "version": "([^"\n]+)"',),
    **{name: tuple(rf'^name = "{package}"\nversion = "([^"\n]+)"' for package in sorted(packages)) for name, packages in LOCKED_PACKAGES.items()},
}
# Prose that links to the current release. `check` rejects any other version in these places and `bump` rewrites them.
PROSE_FILES = ("README.md", "README.ko.md", "docs/getting-started.md", "docs/getting-started.ko.md", "docs/platform-support.md", "docs/platform-support.ko.md")
RELEASE_REFERENCES = [re.compile(prefix + f"(?P<version>{SEMVER.pattern})" + suffix) for prefix, suffix in (
    (r"releases/download/v", r"(?=/)"),             # .../releases/download/v0.8.2/<asset>
    (r"releases/tag/v", r""),                       # .../releases/tag/v0.8.2
    (r"conn-v", r"(?=-(?:aarch64|x86_64)-)"),       # conn-v0.8.2-aarch64-apple-darwin-desktop.dmg
    (r"CONN_VERSION=v", r""),                       # install script pin
    (r"\bv", r"(?= (?:preview|프리뷰))"),            # "v0.8.2 preview" labels
)]


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


def changelog_section(root: Path, version: str) -> str:
    """The entry under `## {version}` in CHANGELOG.md, as written. Release notes quote it."""
    path = root / "CHANGELOG.md"
    if not path.is_file():
        raise ReleaseError("CHANGELOG.md not found; release notes are generated from it")
    lines = path.read_text(encoding="utf-8").splitlines()
    headings = [index for index, line in enumerate(lines) if line.startswith("## ")]
    # `## Unreleased` and other versions never match; only the exact version does.
    found = [index for index in headings if re.fullmatch(rf"## {re.escape(version)}(\s.*)?", lines[index])]
    if not found:
        raise ReleaseError(f"CHANGELOG.md has no '## {version}' section; add the user-facing entry for this release "
                           f"(for example '## {version} — Preview · YYYY-MM-DD'). Release notes are generated from it.")
    if len(found) > 1:
        raise ReleaseError(f"CHANGELOG.md has more than one '## {version}' section")
    end = next((index for index in headings if index > found[0]), len(lines))
    body = "\n".join(lines[found[0] + 1:end]).strip()
    if not body:
        raise ReleaseError(f"CHANGELOG.md section '## {version}' is empty")
    return body


def declarations(root: Path, texts: dict[str, str] | None = None) -> dict[str, str]:
    """Every published version declaration as the tools that consume it parse it.

    `texts` overrides file contents by name, so `bump` can verify a rewrite before it touches the disk.
    """
    def text(name: str) -> str:
        return texts[name] if texts and name in texts else (root / name).read_text(encoding="utf-8")
    cargo = tomllib.loads(text("Cargo.toml"))
    found = {
        WORKSPACE_VERSION: cargo["workspace"]["package"]["version"],
        "Cargo.toml: workspace conn-core": cargo["workspace"]["dependencies"]["conn-core"]["version"],
        "Cargo.toml: workspace conn-frontend": cargo["workspace"]["dependencies"]["conn-frontend"]["version"],
    }
    for name in JSON_MANIFESTS:
        found[name] = json.loads(text(name))["version"]
    found["package-lock.json: frontends/tauri"] = json.loads(text("package-lock.json"))["packages"]["frontends/tauri"]["version"]
    plugins = [p for p in json.loads(text(".claude-plugin/marketplace.json"))["plugins"] if p["name"] == "conn"]
    if len(plugins) != 1:
        raise ReleaseError("Claude marketplace must contain exactly one Conn plugin")
    found[".claude-plugin/marketplace.json"] = plugins[0]["version"]
    for name, expected in LOCKED_PACKAGES.items():
        packages = [p for p in tomllib.loads(text(name))["package"] if p["name"] in expected]
        if {p["name"] for p in packages} != expected or len(packages) != len(expected):
            raise ReleaseError(f"{name}: missing or duplicate Conn packages")
        for package in packages:
            found[f"{name}: {package['name']}"] = package["version"]
    return found


def release_references(text: str) -> list[re.Match]:
    """Places in prose that name one release: its tag, download address, asset files or label."""
    return sorted((match for pattern in RELEASE_REFERENCES for match in pattern.finditer(text)), key=lambda match: match.start())


def stale_references(root: Path, version: str) -> list[str]:
    """Download links that still point at another release, as `file:line` messages."""
    stale = []
    for name in PROSE_FILES:
        if not (root / name).is_file():
            raise ReleaseError(f"{name} not found; it is expected to link to the current release (see PROSE_FILES)")
        text = (root / name).read_text(encoding="utf-8")
        for match in release_references(text):
            if match["version"] != version:
                stale.append(f"{name}:{text.count(chr(10), 0, match.start()) + 1}: {match[0]}")
    return stale


def check(root: Path = ROOT, tag: str | None = None) -> str:
    """All independently published Conn components must share one version.

    That version also needs its changelog section, and the READMEs and guides must link to it.
    """
    declared = declarations(root)
    version = validate_version(declared.pop(WORKSPACE_VERSION))
    if tag is not None and tag != f"v{version}":
        raise ReleaseError(f"Tag {tag!r} must match v{version}")
    mismatches = [f"{name}: {actual!r}" for name, actual in declared.items() if actual != version]
    if mismatches:
        raise ReleaseError(f"Version must be {version} everywhere:\n" + "\n".join(mismatches))
    changelog_section(root, version)
    stale = stale_references(root, version)
    if stale:
        raise ReleaseError(f"Download links and release labels must name {version}; "
                           "`release.py bump` rewrites them with the manifests:\n" + "\n".join(stale))
    return version


def version_key(version: str):
    """SemVer precedence: a prerelease sorts before its release, numeric identifiers before text."""
    match = SEMVER.fullmatch(validate_version(version))
    prerelease = [(0, int(part), "") if part.isdigit() else (1, 0, part) for part in match[4].split(".")] if match[4] else []
    return int(match[1]), int(match[2]), int(match[3]), not prerelease, prerelease


def bump(new: str, root: Path = ROOT, force: bool = False) -> list[tuple[str, int]]:
    """Rewrite every declaration and release link from the current version to `new`.

    Returns (file, replacements) for each changed file. Nothing is written unless
    the whole rewrite parses back to `new`. The changelog stays a human's job.
    """
    validate_version(new)
    declared = declarations(root)
    if len(set(declared.values())) != 1:
        raise ReleaseError("Version declarations disagree; make `release.py check` pass before bumping:\n" +
                           "\n".join(f"{name}: {actual!r}" for name, actual in declared.items()))
    old = validate_version(declared[WORKSPACE_VERSION])
    if version_key(new) <= version_key(old) and not force:
        raise ReleaseError(f"{new} is not newer than the current {old}; pass --force to set it anyway")
    rewritten, counts = {}, {}
    for name, patterns in VERSION_PATTERNS.items():
        text = (root / name).read_bytes().decode("utf-8")
        for pattern in patterns:
            spans = [match.span(1) for match in re.finditer(pattern, text, re.MULTILINE)]
            if len(spans) != 1:
                raise ReleaseError(f"{name}: expected one version declaration matching {pattern!r}, found {len(spans)}")
            text = text[:spans[0][0]] + new + text[spans[0][1]:]
        rewritten[name], counts[name] = text, len(patterns)
    for name in PROSE_FILES:
        text = (root / name).read_bytes().decode("utf-8")
        spans = [match.span("version") for match in release_references(text) if match["version"] == old]
        for first, last in reversed(spans):
            text = text[:first] + new + text[last:]
        rewritten[name], counts[name] = text, len(spans)
    # The consumers' own parsers have the last word, before anything is written.
    missed = [f"{name}: {actual!r}" for name, actual in declarations(root, rewritten).items() if actual != new]
    if missed:
        raise ReleaseError("Rewriting would leave these declarations behind; nothing was changed:\n" + "\n".join(missed))
    changed = [(name, counts[name]) for name, text in rewritten.items() if text != (root / name).read_bytes().decode("utf-8")]
    for name, _ in changed:
        (root / name).write_bytes(rewritten[name].encode("utf-8"))
    return changed


def bump_followups(root: Path, version: str) -> list[str]:
    """What `bump` leaves to a person, as lines to print."""
    lines = []
    try:
        changelog_section(root, version)
    except ReleaseError:
        lines.append(f"Next: add a '## {version} — Preview · YYYY-MM-DD' section to CHANGELOG.md. "
                     "`release.py check` fails until it exists, and the release notes quote it.")
    stale = stale_references(root, version)
    if stale:
        lines.append("These links name some other release and were left alone; fix them by hand:\n" + "\n".join(stale))
    return lines


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
    entries = {}
    for target, platform in PLATFORMS.items():
        name = update_asset(version, target)
        regular_file(artifacts / name, artifacts)
        signature = signature_text(regular_file(artifacts / (name + ".sig"), artifacts))
        if target.endswith("apple-darwin"):
            evidence = read_json(artifacts / signing_name(version, target)).get("updater", {})
            if evidence != {"archive": name, "sha256": sha256(artifacts / name), "containedAppVerified": True}:
                raise ReleaseError("macOS updater archive lacks matching notarized-app evidence")
        entries[platform] = {"url": f"{UPDATER_REPOSITORY}/releases/download/v{version}/{name}", "signature": signature}
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


def release_notes(root: Path, version: str, tag: str, sha: str | None = None, template: Path = NOTES_TEMPLATE) -> str:
    """Fill the static template; what changed comes from the changelog, never from this script."""
    changes = changelog_section(root, version)
    # A path that works inside the repository would be a dead link on the release page.
    changes = re.sub(r"\]\((?![#/]|[A-Za-z][A-Za-z0-9+.-]*:)([^)\s]+)\)", rf"]({REPOSITORY}/blob/{tag}/\1)", changes)
    names = {target: asset_names(version, target) for target in TARGETS}
    mac, windows, linux = names["aarch64-apple-darwin"], names["x86_64-pc-windows-msvc"], names["x86_64-unknown-linux-gnu"]
    values = {
        "tag": tag, "version": version, "repository": REPOSITORY, "changes": changes,
        "source_commit": f"\nSource commit: `{sha.lower()}`\n" if sha else "",
        "mac_cli": mac[0], "mac_dmg": mac[1], "windows_cli": windows[0], "windows_setup": windows[1],
        "linux_cli": linux[0], "linux_deb": linux[1], "linux_appimage": linux[2],
    }
    text = re.sub(r"\A<!--.*?-->\s*", "", template.read_text(encoding="utf-8"), flags=re.DOTALL)
    if "{changes}" not in text:
        raise ReleaseError(f"{template.name} must contain {{changes}}")

    def fill(match):
        if match[1] not in values:
            raise ReleaseError(f"{template.name}: unknown placeholder {match[0]}")
        return values[match[1]]
    # One pass over the template only: braces inside the changelog text are left alone.
    return re.sub(r"\{([a-z_]+)\}", fill, text)


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
    notes = release_notes(root, version, tag, sha)
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
    sub.add_parser("check", help="Check version consistency, the changelog section and release links").add_argument("--tag")
    raise_version = sub.add_parser("bump", help="Set a new version in every manifest, lockfile entry and download link")
    raise_version.add_argument("version")
    raise_version.add_argument("--force", action="store_true", help="Allow a version that is not newer than the current one")
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
        elif args.command == "bump":
            changed = bump(args.version, force=args.force)
            for name, count in changed:
                print(f"{name}: {count}")
            print(f"Set {args.version} in {len(changed)} files.")
            for line in bump_followups(ROOT, args.version):
                print(line)
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
