#!/usr/bin/env python3
"""Check the built macOS scripting bundle and compile its PAM example.

This does not execute AppleScript, launch Conn, change TCC permissions, install
the app, or use release credentials. The CI app is signed ad-hoc only.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
NATIVE = ROOT / "frontends/tauri/src-tauri"


class ScriptingError(ValueError):
    pass


def command_codes(xml: str) -> dict[str, str]:
    commands = ET.fromstring(xml).findall(".//command")
    result = {}
    names = set()
    for command in commands:
        code, name = command.get("code", ""), command.get("name", "")
        if len(code) != 8 or not name or code in result or name in names:
            raise ScriptingError("Scripting commands need unique eight-character codes and names")
        result[code] = name
        names.add(name)
    if not result:
        raise ScriptingError("Scripting dictionary has no commands")
    return result


def validate_adapter(definition: Path = NATIVE / "Conn.sdef", adapter: Path = NATIVE / "src/macos/scripting.m"):
    commands = command_codes(definition.read_text(encoding="utf-8"))
    native_codes = set(re.findall(r"case\s+'([^']{4})'\s*:", adapter.read_text(encoding="utf-8")))
    if {code[:4] for code in commands} != {"Conn"} or {code[4:] for code in commands} != native_codes:
        raise ScriptingError("Conn.sdef command codes and the native adapter do not match")
    return commands


def target_example(source: str, app: Path) -> str:
    # Compile against this exact bundle, never an installed app with the same ID.
    target = str(app.resolve()).replace("\\", "\\\\").replace('"', '\\"')
    pattern = r'\b(tell|using terms from) application (?:"Conn"|id "dev\.eggp\.conn")'
    source, count = re.subn(pattern, lambda m: f'{m[1]} application "{target}"', source)
    if not count:
        raise ScriptingError("The example must target Conn by name or bundle ID")
    return source


def run(*args: str) -> str:
    return subprocess.run(args, check=True, text=True, capture_output=True, timeout=60).stdout.strip()


def check_bundle(app: Path, example: Path, definition: Path = NATIVE / "Conn.sdef"):
    app = app.resolve(strict=True)
    plist = app / "Contents/Info.plist"
    if run("plutil", "-extract", "NSAppleScriptEnabled", "raw", "-o", "-", str(plist)) != "true":
        raise ScriptingError("Built app does not enable AppleScript")
    if run("plutil", "-extract", "OSAScriptingDefinition", "raw", "-o", "-", str(plist)) != "Conn.sdef":
        raise ScriptingError("Built app does not reference Conn.sdef")
    if (app / "Contents/Resources/Conn.sdef").read_bytes() != definition.read_bytes():
        raise ScriptingError("Built app does not contain the current scripting dictionary")
    if command_codes(run("sdef", str(app))) != command_codes(definition.read_text(encoding="utf-8")):
        raise ScriptingError("macOS did not discover the packaged Conn dictionary")
    run("codesign", "--verify", "--deep", "--strict", str(app))
    with tempfile.TemporaryDirectory(prefix="conn-applescript-compile-") as directory:
        source = Path(directory) / "pam-connect.applescript"
        compiled = Path(directory) / "pam-connect.scpt"
        source.write_text(target_example(example.read_text(encoding="utf-8"), app), encoding="utf-8")
        run("osacompile", "-o", str(compiled), str(source))
        if not compiled.is_file() or not compiled.stat().st_size:
            raise ScriptingError("osacompile did not produce a compiled example")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--example", type=Path, default=ROOT / "examples/applescript/pam-connect.applescript")
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("The bundle smoke check requires macOS")
    validate_adapter()
    check_bundle(args.app, args.example)
    print("macOS scripting metadata, bundle signature, and PAM example compilation passed; no script executed.")


if __name__ == "__main__":
    main()
