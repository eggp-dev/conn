#!/usr/bin/env python3
"""Assemble reviewed, timestamped capture intervals with cuts and no speed change."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("raw", type=Path)
    parser.add_argument("edit_map", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location("capture_encoder", Path(__file__).with_name("encode-capture.py"))
    encoder = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(encoder)
    edit = json.loads(args.edit_map.read_text())
    if edit["speed"] != 1 or edit["fps"] != 30:
        parser.error("This production requires original speed and 30 fps output")
    args.output.mkdir(parents=True, exist_ok=True)

    def assemble(item):
        name, cuts = item
        segments, elapsed = [], 0
        for cut in cuts:
            directory = args.raw / cut["take"]
            capture = json.loads((directory / "capture.json").read_text())
            if capture.get("truncated") or capture.get("error"):
                raise ValueError(f"Capture needs review: {cut['take']}")
            selected, duration = encoder.selected_frames(capture, directory, cut["start"], cut["end"] - cut["start"])
            if abs(duration - (cut["end"] - cut["start"])) > 0.001:
                raise ValueError("Selected interval extends beyond recorded time")
            segments.extend(selected)
            elapsed += duration
        output = args.output / f"{name}.mp4"
        with tempfile.TemporaryDirectory(prefix="conn-cut-") as temp:
            manifest = Path(temp) / "frames.ffconcat"
            lines = ["ffconcat version 1.0"]
            for path, duration in segments:
                lines.extend([f"file {encoder.quote_concat(path)}", "option framerate 1000", f"duration {duration:.9f}"])
            lines.extend([f"file {encoder.quote_concat(segments[-1][0])}", "option framerate 1000"])
            manifest.write_text("\n".join(lines) + "\n")
            subprocess.run([
                "ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0",
                "-i", str(manifest), "-vf", "fps=30,setsar=1,format=yuv420p", "-frames:v", str(round(elapsed * 30)),
                "-c:v", "libx264", "-threads", "4", "-preset", "fast", "-crf", "18", "-pix_fmt", "yuv420p",
                "-movflags", "+faststart", "-an", str(output)
            ], check=True)
        result = {"file": output.name, "seconds": elapsed, "cuts": cuts,
                  "sha256": hashlib.sha256(output.read_bytes()).hexdigest()}
        print(json.dumps(result), flush=True)
        return result

    with ThreadPoolExecutor(max_workers=2) as pool:
        outputs = list(pool.map(assemble, edit["clips"].items()))
    (args.output / "edit-map.json").write_text(json.dumps(edit, indent=2) + "\n")
    (args.output / "clip-hashes.json").write_text(json.dumps(outputs, indent=2) + "\n")


if __name__ == "__main__":
    main()
