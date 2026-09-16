#!/usr/bin/env python3
"""Package final signed app updates. Keys are supplied only by protected CI secrets."""
import argparse
import base64
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import release

PLATFORMS = {"aarch64-apple-darwin": "darwin-aarch64", "x86_64-unknown-linux-gnu": "linux-x86_64", "x86_64-pc-windows-msvc": "windows-x86_64"}


def config(out: Path):
    public = os.environ.get("TAURI_SIGNING_PUBLIC_KEY", "").strip()
    # Tauri public keys are base64 text wrapping a minisign public-key file.
    try:
        decoded = base64.b64decode(public, validate=True).decode()
        if "untrusted comment:" not in decoded or len(decoded.splitlines()) < 2:
            raise ValueError()
    except (ValueError, UnicodeError):
        raise release.ReleaseError("Configure TAURI_SIGNING_PUBLIC_KEY before building a release") from None
    out.write_text(json.dumps({"plugins": {"updater": {"pubkey": public}}}) + "\n", encoding="utf-8")


def package(target: str, out: Path, root=release.ROOT):
    version = release.check(root)
    out = out.resolve()
    names = release.updater_names(version, target)
    if target.endswith("apple-darwin"):
        import macos_sign
        bundle = root / "frontends/tauri/src-tauri/target" / target / "release/bundle/macos"
        apps = list(bundle.glob("*.app"))
        if len(apps) != 1:
            raise release.ReleaseError("Expected one notarized updater app")
        report_path = out / release.signing_name(version, target)
        report = release.read_json(report_path)
        evidence = macos_sign.app_evidence(apps[0], os.environ.get("APPLE_TEAM_ID", ""))
        if evidence != report["app"]:
            raise release.ReleaseError("Updater app differs from the verified installed app")
        artifact = out / names[0]
        with tarfile.open(artifact, "w:gz", dereference=False) as archive:
            archive.add(apps[0], arcname=apps[0].name)
        # Validate the archive users will actually extract, including its stapled ticket.
        with tempfile.TemporaryDirectory(prefix="conn-update-verify-") as directory:
            with tarfile.open(artifact, "r:gz") as archive:
                archive.extractall(directory, filter="data")
            if macos_sign.app_evidence(Path(directory) / apps[0].name, os.environ.get("APPLE_TEAM_ID", "")) != evidence:
                raise release.ReleaseError("Packaged updater app differs from the notarized app")
        report["updater"] = {"archive": artifact.name, "sha256": release.sha256(artifact), "containedAppVerified": True}
        report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    else:
        artifact = out / release.update_asset(version, target)
    release.regular_file(artifact, out)
    if not os.environ.get("TAURI_SIGNING_PRIVATE_KEY"):
        raise release.ReleaseError("Missing updater signing key; refusing unsigned release")
    # Never put keys/passwords in argv or emit signer output.
    result = subprocess.run(["node", str(root / "frontends/tauri/scripts/tauri.mjs"), "signer", "sign", str(artifact)],
                            cwd=root / "frontends/tauri", capture_output=True, timeout=60, check=False)
    if result.returncode:
        raise release.ReleaseError("Updater signing failed; output withheld")
    release.signature_text(artifact.with_name(artifact.name + ".sig"))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    sub = p.add_subparsers(dest="action", required=True)
    c = sub.add_parser("config"); c.add_argument("--out", type=Path, required=True)
    a = sub.add_parser("package"); a.add_argument("--target", choices=PLATFORMS, required=True); a.add_argument("--out", type=Path, required=True)
    args = p.parse_args()
    try:
        config(args.out) if args.action == "config" else package(args.target, args.out)
        print("Updater artifact stage passed:", args.action)
    except (release.ReleaseError, OSError, ValueError) as error:
        print(str(error)); return 1
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
