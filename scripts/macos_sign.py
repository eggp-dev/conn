#!/usr/bin/env python3
"""Developer ID signing gates for trusted macOS release runners.

Secrets are consumed only from the process environment; command output containing
credentials or identities is never printed. Public evidence contains verification
results and hashes, not certificate material, account identifiers or notary logs.
Tauri signs/notarizes/staples the app; this script verifies it, notarizes the
standalone CLI and final DMG, then packages the exact verified bytes.
"""
from __future__ import annotations

import argparse
import base64
import binascii
import json
import os
from pathlib import Path
import plistlib
import re
import secrets
import shlex
import shutil
import subprocess
import sys
import tempfile
import uuid

import release

MAC_TARGETS = tuple(target for target in release.TARGETS if target.endswith("apple-darwin"))
SECRET_NAMES = ("APPLE_CERTIFICATE", "APPLE_CERTIFICATE_PASSWORD", "APPLE_SIGNING_IDENTITY",
                "APPLE_ID", "APPLE_PASSWORD", "APPLE_TEAM_ID")
PROFILE = "conn-release-notary"


def require_runner(env=os.environ):
    if sys.platform != "darwin" or env.get("RUNNER_OS") != "macOS":
        raise release.ReleaseError("Signing is restricted to a macOS release runner")
    if env.get("GITHUB_ACTIONS") != "true" or env.get("GITHUB_REPOSITORY") != release.REPOSITORY_SLUG:
        raise release.ReleaseError("Signing requires this repository's trusted Actions runner")
    event, ref = env.get("GITHUB_EVENT_NAME"), env.get("GITHUB_REF", "")
    if not (event == "push" and ref.startswith("refs/tags/v") or
            event == "workflow_dispatch" and ref == "refs/heads/main"):
        raise release.ReleaseError("Signing is allowed only for version tags or a main-branch manual release")


def require_secrets(env=os.environ):
    missing = [name for name in SECRET_NAMES if not env.get(name, "").strip()]
    if missing:
        raise release.ReleaseError("Missing required secrets: " + ", ".join(missing))
    validate_identity(env["APPLE_SIGNING_IDENTITY"], env["APPLE_TEAM_ID"])


def validate_identity(identity: str, team: str):
    match = re.fullmatch(r"Developer ID Application: [^\r\n]+ \(([A-Z0-9]{10})\)", identity)
    if re.fullmatch(r"[A-Z0-9]{10}", team) is None or not match or match[1] != team:
        raise release.ReleaseError("A Developer ID Application identity matching APPLE_TEAM_ID is required; ad-hoc signing is forbidden")


def import_failure_category(stderr: str) -> str:
    # Match only known Security.framework messages and emit a fixed label. Never
    # echo stderr, which may also include certificate subjects or private paths.
    if "MAC verification failed during PKCS12 import" in stderr:
        return "pkcs12-verification (password, container integrity or algorithm compatibility)"
    if any(message in stderr for message in ("Unknown format in import", "Import/Export format unsupported",
                                             "Unable to decode the provided data")):
        return "pkcs12-format-or-decoding"
    if any(message in stderr for message in ("User interaction is not allowed", "Write permissions error",
                                             "The specified keychain could not be found")):
        return "keychain-access"
    return "unclassified-import-error"


def command(args, label, timeout=120, *, input_text=None):
    # Do not echo args or captured output: security/notarytool arguments can carry
    # secrets. All errors name only the fixed stage and numeric exit status.
    env = {key: value for key, value in os.environ.items() if not key.startswith("APPLE_")}
    try:
        result = subprocess.run([str(arg) for arg in args], capture_output=True, text=True,
                                check=False, timeout=timeout, env=env, input=input_text)
    except (OSError, subprocess.TimeoutExpired):
        raise release.ReleaseError(f"{label} could not complete") from None
    if result.returncode != 0:
        category = f"; category: {import_failure_category(result.stderr)}" if args[:2] == ["security", "import"] else ""
        raise release.ReleaseError(f"{label} failed (exit {result.returncode}){category}; no release artifacts will be uploaded")
    return result


def state_path(path: Path, env=os.environ) -> Path:
    temporary = env.get("RUNNER_TEMP")
    if not temporary or path.is_symlink():
        raise release.ReleaseError("Signing state must be a private directory under RUNNER_TEMP")
    resolved = path.resolve()
    if resolved == Path(temporary).resolve() or not resolved.is_relative_to(Path(temporary).resolve()):
        raise release.ReleaseError("Signing state must be a child of RUNNER_TEMP")
    return resolved


def write_private(path: Path, value):
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
        json.dump(value, stream)


def prepare(path: Path, env=os.environ):
    require_runner(env)
    require_secrets(env)
    path = state_path(path, env)
    if path.exists():
        raise release.ReleaseError("Signing state already exists; clean up the earlier attempt first")
    path.mkdir(mode=0o700)
    keychain = path / "release.keychain-db"
    certificate = path / "certificate.p12"
    original = shlex.split(command(["security", "list-keychains", "-d", "user"], "Read keychain search list").stdout)
    state = {"keychain": str(keychain), "originalKeychains": original, "profile": PROFILE}
    write_private(path / "state.json", state)
    password = secrets.token_urlsafe(48)
    try:
        try:
            decoded = base64.b64decode("".join(env["APPLE_CERTIFICATE"].split()), validate=True)
        except (ValueError, binascii.Error):
            raise release.ReleaseError("APPLE_CERTIFICATE must contain a base64-encoded PKCS#12 certificate") from None
        if not decoded:
            raise release.ReleaseError("The signing certificate is empty")
        descriptor = os.open(certificate, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(decoded)
        command(["security", "create-keychain", "-p", password, keychain], "Create temporary keychain")
        command(["security", "set-keychain-settings", "-lut", "21600", keychain], "Configure temporary keychain")
        command(["security", "unlock-keychain", "-p", password, keychain], "Unlock temporary keychain")
        # Explicit PKCS12 selects SecPKCS12Import on macOS 15; autodetection uses
        # the older SecKeychainItemImport path, which can reject modern PKCS12.
        command(["security", "import", certificate, "-f", "pkcs12", "-k", keychain, "-P", env["APPLE_CERTIFICATE_PASSWORD"],
                 "-T", "/usr/bin/codesign", "-T", "/usr/bin/security"], "Import signing certificate")
        command(["security", "set-key-partition-list", "-S", "apple-tool:,apple:,codesign:",
                 "-s", "-k", password, keychain], "Authorize signing tools")
        command(["security", "list-keychains", "-d", "user", "-s", keychain, *original], "Select temporary keychain")
        identities = command(["security", "find-identity", "-v", "-p", "codesigning", keychain], "Validate signing identity").stdout
        matches = re.findall(r'\b([A-Fa-f0-9]{40})\s+"([^"\r\n]+)"', identities)
        fingerprints = [digest for digest, identity in matches if identity == env["APPLE_SIGNING_IDENTITY"]]
        if len(fingerprints) != 1:
            raise release.ReleaseError("The imported keychain must contain exactly one valid requested Developer ID identity")
        state["fingerprint"] = fingerprints[0]
        write_private(path / "state.json", state)
        command(["xcrun", "notarytool", "store-credentials", PROFILE, "--keychain", keychain,
                 "--apple-id", env["APPLE_ID"], "--password", env["APPLE_PASSWORD"],
                 "--team-id", env["APPLE_TEAM_ID"]], "Validate and store notarization credentials")
    finally:
        certificate.unlink(missing_ok=True)


def read_state(path: Path, env=os.environ):
    path = state_path(path, env)
    state = release.read_json(release.regular_file(path / "state.json", path))
    if state.get("keychain") != str(path / "release.keychain-db") or state.get("profile") != PROFILE:
        raise release.ReleaseError("Invalid signing state")
    return path, state


def cleanup(path: Path, env=os.environ):
    require_runner(env)
    path = state_path(path, env)
    if not path.exists():
        return
    errors = []
    try:
        if (path / "state.json").exists():
            _, state = read_state(path, env)
            try:
                command(["security", "list-keychains", "-d", "user", "-s", *state["originalKeychains"]], "Restore keychain search list")
            except release.ReleaseError as error:
                errors.append(str(error))
        keychain = path / "release.keychain-db"
        if keychain.exists():
            try:
                command(["security", "delete-keychain", keychain], "Delete temporary signing keychain")
            except release.ReleaseError as error:
                errors.append(str(error))
    finally:
        shutil.rmtree(path)
    if errors:
        raise release.ReleaseError("; ".join(errors))


def parse_signature(text: str, team: str, runtime=True):
    if f"TeamIdentifier={team}" not in text.splitlines():
        raise release.ReleaseError("Signature TeamIdentifier does not match the release team")
    if not any(line.startswith("Authority=Developer ID Application: ") for line in text.splitlines()):
        raise release.ReleaseError("Signature is not issued by a Developer ID Application certificate")
    if not re.search(r"^Timestamp=.+$", text, re.MULTILINE):
        raise release.ReleaseError("Signature does not contain a secure timestamp")
    if runtime and not re.search(r"^CodeDirectory .*flags=0x[0-9a-f]+\([^\n)]*\bruntime\b", text, re.MULTILINE):
        raise release.ReleaseError("Executable signature does not enable hardened runtime")
    match = re.search(r"^CDHash=([0-9a-fA-F]{40,64})$", text, re.MULTILINE)
    if not match:
        raise release.ReleaseError("Signature has no CodeDirectory hash")
    return {"developerId": True, "teamVerified": True, "secureTimestamp": True,
            "hardenedRuntime": runtime, "cdhash": match[1].lower()}


def verify_signature(path: Path, team: str, runtime=True, deep=False):
    command(["codesign", "--verify", "--strict", *( ["--deep"] if deep else []), path], "Verify code signature")
    shown = command(["codesign", "--display", "--verbose=4", path], "Inspect code signature")
    return parse_signature(shown.stdout + shown.stderr, team, runtime)


def app_evidence(app: Path, team: str):
    if app.is_symlink() or not app.is_dir():
        raise release.ReleaseError("Expected a regular Conn app bundle")
    info = plistlib.loads(release.regular_file(app / "Contents/Info.plist", app).read_bytes())
    name = info.get("CFBundleExecutable")
    if not isinstance(name, str) or Path(name).name != name:
        raise release.ReleaseError("Invalid app executable name")
    executable = release.regular_file(app / "Contents/MacOS" / name, app)
    sidecar = release.regular_file(app / "Contents/MacOS/conn", app)
    if executable == sidecar:
        raise release.ReleaseError("Expected distinct desktop executable and Conn sidecar")
    result = {"bundle": verify_signature(app, team, deep=True),
              "desktop": verify_signature(executable, team), "sidecar": verify_signature(sidecar, team)}
    command(["xcrun", "stapler", "validate", app], "Validate app notarization ticket")
    command(["spctl", "--assess", "--type", "execute", app], "Assess app with Gatekeeper")
    result.update({"stapled": True, "gatekeeperAccepted": True})
    return result


def accepted_submission(payload):
    if not isinstance(payload, dict) or payload.get("status") != "Accepted":
        raise release.ReleaseError("Apple notarization was not Accepted")
    try:
        identifier = str(uuid.UUID(payload["id"]))
    except (KeyError, ValueError, TypeError, AttributeError):
        raise release.ReleaseError("Notarization result has no valid submission ID") from None
    return {"id": identifier, "status": "Accepted"}


def notarize(path: Path, state):
    response = command(["xcrun", "notarytool", "submit", path, "--keychain-profile", state["profile"],
                        "--keychain", state["keychain"], "--wait", "--timeout", "30m", "--output-format", "json"],
                       "Apple notarization submission", timeout=1900)
    try:
        return accepted_submission(json.loads(response.stdout))
    except json.JSONDecodeError:
        raise release.ReleaseError("Notarization response was not valid JSON") from None


def inspect_dmg(dmg: Path, team: str, expected_app):
    # Verify what a user actually receives, including the app ticket inside the
    # read-only mounted image. Mount into our own temporary directory only.
    with tempfile.TemporaryDirectory(prefix="conn-dmg-") as directory:
        mount = Path(directory) / "volume"
        mount.mkdir()
        # Mount with closed stdin: a license agreement must fail rather than be accepted.
        command(["hdiutil", "attach", dmg, "-plist", "-readonly", "-nobrowse", "-mountpoint", mount],
                "Mount final DMG", input_text="")
        try:
            apps = list(mount.glob("*.app"))
            if len(apps) != 1:
                raise release.ReleaseError("Final DMG must contain exactly one app")
            if not (apps[0] / "Contents/Resources/LICENSE").is_file():
                raise release.ReleaseError("Final app is missing the bundled license")
            if not (mount / "Applications").is_symlink() or os.readlink(mount / "Applications") != "/Applications":
                raise release.ReleaseError("Final DMG is missing the Applications destination link")
            received = app_evidence(apps[0], team)
            for key in ("bundle", "desktop", "sidecar"):
                if received[key]["cdhash"] != expected_app[key]["cdhash"]:
                    raise release.ReleaseError("Final DMG contains a different app or sidecar")
        finally:
            command(["hdiutil", "detach", mount], "Unmount final DMG")
    return True


def finish(target: str, path: Path, out: Path, root=release.ROOT, env=os.environ):
    require_runner(env)
    if target not in MAC_TARGETS:
        raise release.ReleaseError("Unsupported macOS target")
    validate_identity(env.get("APPLE_SIGNING_IDENTITY", ""), env.get("APPLE_TEAM_ID", ""))
    path, state = read_state(path, env)
    if not re.fullmatch(r"[0-9a-fA-F]{40}", state.get("fingerprint", "")):
        raise release.ReleaseError("Signing state has no validated certificate fingerprint")
    version = release.check(root)
    source_commit = env.get("RELEASE_SHA", "").lower()
    if re.fullmatch(r"[0-9a-f]{40}", source_commit) is None:
        raise release.ReleaseError("RELEASE_SHA must identify the validated source commit")
    team = env["APPLE_TEAM_ID"]
    executable = release.regular_file(root / "target" / target / "release/conn", root / "target")
    bundle = root / "frontends/tauri/src-tauri/target" / target / "release/bundle"
    apps, dmgs = list((bundle / "macos").glob("*.app")), list((bundle / "dmg").glob("*.dmg"))
    if len(apps) != 1 or len(dmgs) != 1:
        raise release.ReleaseError("Expected exactly one Tauri app and one DMG")
    dmg = release.regular_file(dmgs[0], bundle)
    app = app_evidence(apps[0], team)
    command(["codesign", "--force", "--sign", state["fingerprint"], "--keychain", state["keychain"],
             "--options", "runtime", "--timestamp", executable], "Sign standalone CLI")
    cli = verify_signature(executable, team)
    cli["sha256"] = release.sha256(executable)
    cli_zip = path / "conn-cli-notarization.zip"
    command(["ditto", "-c", "-k", "--keepParent", executable, cli_zip], "Prepare CLI notarization ZIP")
    try:
        cli_notary = notarize(cli_zip, state)
    finally:
        cli_zip.unlink(missing_ok=True)
    # Never staple the bare CLI or its archive: Apple issues binary tickets but
    # does not support attaching a ticket to a standalone executable or ZIP.
    cli["stapled"] = False
    command(["codesign", "--force", "--sign", state["fingerprint"], "--keychain", state["keychain"],
             "--timestamp", dmg], "Sign final DMG")
    dmg_signature = verify_signature(dmg, team, runtime=False)
    dmg_notary = notarize(dmg, state)
    command(["xcrun", "stapler", "staple", dmg], "Staple DMG notarization ticket")
    command(["xcrun", "stapler", "validate", dmg], "Validate DMG notarization ticket")
    command(["spctl", "--assess", "--type", "open", "--context", "context:primary-signature", dmg], "Assess DMG with Gatekeeper")
    verify_signature(dmg, team, runtime=False)
    inspect_dmg(dmg, team, app)
    assets = release.package(target, out, root)
    report = {"schemaVersion": 1, "version": version, "target": target, "sourceCommit": source_commit,
              "app": app, "cli": cli,
              "dmg": {**dmg_signature, "stapled": True, "gatekeeperAccepted": True, "containedAppVerified": True},
              "notarization": {"cli": cli_notary, "dmg": dmg_notary},
              "assets": {asset.name: release.sha256(asset) for asset in assets}}
    evidence = out / release.signing_name(version, target)
    if evidence.exists():
        raise release.ReleaseError("Signing evidence already exists; use a fresh artifact directory")
    evidence.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    release.validate_signing_report(evidence, version, target, out)
    return evidence


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    for action in ("prepare", "cleanup", "finish"):
        sub = commands.add_parser(action)
        sub.add_argument("--state", type=Path, required=True)
        if action == "finish":
            sub.add_argument("--target", choices=MAC_TARGETS, required=True)
            sub.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.action == "prepare":
            prepare(args.state)
        elif args.action == "cleanup":
            cleanup(args.state)
        else:
            finish(args.target, args.state, args.out)
        print(f"macOS signing stage passed: {args.action}")
        return 0
    except release.ReleaseError as error:
        print(f"macOS signing stage failed: {args.action}: {error}", file=sys.stderr)
        return 1
    except (OSError, ValueError, KeyError):
        # Do not serialize exception values from credential-bearing library calls.
        print(f"macOS signing stage failed: {args.action}; artifacts are blocked", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
