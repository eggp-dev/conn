#!/usr/bin/env python3
"""Encode timestamped browser screencast frames without altering source footage.

Input: a capture directory containing capture.json with startedAt/endedAt in
epoch milliseconds and frames [{path, timestamp}] in epoch seconds. --start
and --duration select seconds within the recorded interval. Frames before that
interval establish its initial image; the last image holds only until its end.
"""

import argparse
import json
import math
import subprocess
import sys
import tempfile
from pathlib import Path


def quote_concat(path):
    return "'" + str(path).replace("'", "'\\''") + "'"


def selected_frames(capture, directory, start, duration):
    began = float(capture["startedAt"]) / 1000
    ended = float(capture["endedAt"]) / 1000
    if not all(math.isfinite(value) for value in (began, ended, start)):
        raise ValueError("Capture and trim timestamps must be finite")
    if start < 0 or ended <= began or began + start >= ended:
        raise ValueError("--start must fall inside the capture interval")
    left = began + start
    right = ended
    if duration is not None:
        if not math.isfinite(duration) or duration <= 0:
            raise ValueError("--duration must be finite and positive")
        right = min(ended, left + duration)
    frames = []
    for frame in capture["frames"]:
        stamp = float(frame["timestamp"])
        if not math.isfinite(stamp):
            raise ValueError("Frame timestamps must be finite")
        path = Path(frame["path"])
        if not path.is_absolute():
            path = directory / path
        path = path.resolve()
        if "\n" in str(path) or "\r" in str(path) or not path.is_file():
            raise ValueError(f"Invalid frame path: {path}")
        frames.append((stamp, path))
    if not frames:
        raise ValueError("Capture has no frames")
    frames.sort(key=lambda frame: frame[0])
    initial = frames[0][1]
    for stamp, path in frames:
        if stamp > left:
            break
        initial = path
    selected = [(left, initial)]
    for stamp, path in frames:
        if left < stamp < right:
            if stamp == selected[-1][0]:
                selected[-1] = (stamp, path)
            elif path != selected[-1][1]:
                selected.append((stamp, path))
    segments = []
    for index, (stamp, path) in enumerate(selected):
        following = selected[index + 1][0] if index + 1 < len(selected) else right
        if following > stamp:
            segments.append((path, following - stamp))
    return segments, right - left


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture_dir", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--start", type=float, default=0)
    parser.add_argument("--duration", type=float)
    parser.add_argument("--crf", type=int, default=18, choices=range(0, 52), metavar="0..51")
    parser.add_argument("--width", type=int, default=1600)
    parser.add_argument("--height", type=int, default=900)
    parser.add_argument("--overwrite", action="store_true")
    args = parser.parse_args()
    if args.width <= 0 or args.height <= 0 or args.width % 2 or args.height % 2:
        parser.error("--width and --height must be positive even dimensions")
    if args.output.exists() and not args.overwrite:
        parser.error("Output exists; pass --overwrite to replace this export")
    try:
        capture = json.loads((args.capture_dir / "capture.json").read_text(encoding="utf-8"))
        if capture.get("truncated"):
            print("Warning: capture reports dropped/truncated events.", file=sys.stderr)
        segments, elapsed = selected_frames(capture, args.capture_dir, args.start, args.duration)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="conn-encode-") as temporary:
            manifest = Path(temporary) / "frames.ffconcat"
            lines = ["ffconcat version 1.0"]
            for path, duration in segments:
                lines.extend([f"file {quote_concat(path)}", "option framerate 1000",
                              f"duration {duration:.9f}"])
            # Concat needs the last file repeated to honor its final duration.
            # Explicit -t below prevents this sentinel adding extra playback.
            lines.extend([f"file {quote_concat(segments[-1][0])}", "option framerate 1000"])
            manifest.write_text("\n".join(lines) + "\n", encoding="utf-8")
            filters = (f"fps=30,scale={args.width}:{args.height}:force_original_aspect_ratio=decrease,"
                       f"pad={args.width}:{args.height}:(ow-iw)/2:(oh-ih)/2:color=0x080b11,"
                       "setsar=1,format=yuv420p")
            subprocess.run([
                "ffmpeg", "-hide_banner", "-loglevel", "warning", "-y" if args.overwrite else "-n",
                "-f", "concat", "-safe", "0", "-i", str(manifest), "-vf", filters,
                "-t", f"{elapsed:.9f}", "-c:v", "libx264", "-preset", "medium",
                "-crf", str(args.crf), "-pix_fmt", "yuv420p", "-movflags", "+faststart",
                "-an", str(args.output),
            ], check=True)
        print(json.dumps({"output": str(args.output.resolve()), "duration": elapsed,
                          "frames": len(segments), "width": args.width,
                          "height": args.height, "fps": 30}))
        return 0
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f"Capture export failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
