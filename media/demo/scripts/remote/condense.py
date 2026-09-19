#!/usr/bin/env python3
"""Turn one continuous take into footage: shorten spans in which the screen does not change.

Nothing is reordered and no on-screen change is removed: every frame in which the picture
changes is kept at its real pace. Only still spans (the agent thinking, a command waiting)
are shortened: to --hold while the agent is thinking, and to a longer, readable pause
while a card is on screen waiting for the human. Cursor blinking does not count as change.

  condense.py TAKE_DIR OUT_DIR [--hold 0.55] [--fps 30]
Writes OUT_DIR/take.mp4 and OUT_DIR/timeline.json (events and clicks on the new clock).
"""
import argparse, json, pathlib, subprocess, sys
from PIL import Image, ImageChops

p = argparse.ArgumentParser(); p.add_argument("take"); p.add_argument("out"); p.add_argument("--hold", type=float, default=0.55); p.add_argument("--fps", type=int, default=30)
p.add_argument("--keep", type=float, default=0.35, help="seconds of real pace kept after each change")
a = p.parse_args()
take, out = pathlib.Path(a.take), pathlib.Path(a.out); out.mkdir(parents=True, exist_ok=True)
frames = json.loads((take / "frames.json").read_text()); events = json.loads((take / "events.json").read_text())
start = next(e for e in events if e["event"] == "record_start"); offset = start["wall"] - start["t"]   # frame ts -> event clock

def small(name):
    im = Image.open(take / "frames" / name); im.draft("L", (400, 225)); return im.convert("L").resize((240, 135))
prev = None; changed = []
for f in frames:
    cur = small(f["name"])
    if prev is None: changed.append(True)
    else:
        diff = ImageChops.difference(prev, cur).point(lambda v: 255 if v > 24 else 0)
        box = diff.getbbox()
        # A blinking cursor or the pulsing status dot touches a few pixels; real output touches many.
        area = 0 if box is None else sum(diff.crop(box).histogram()[1:])
        changed.append(area > 14)
    prev = cur
times = [f["ts"] - offset for f in frames]
# A card waiting for the human is the point of the film; leave the viewer time to read it.
reading = []
for i, e in enumerate(events):
    if e["event"] == "card" or (e["event"] == "beat" and e.get("name") == "join_card"):
        click = next((c for c in events[i + 1:] if c["event"] == "click"), None)
        if click: reading.append((e["t"], click["t"], 3.0 if "rm " in e.get("cmd", "") else 1.7 if e["event"] == "card" else 3.2))
def hold_at(t):
    for a0, a1, h in reading:
        if a0 <= t <= a1: return h
    return a.hold
active_until = -1.0; new_t = []; clock = 0.0; last_real = times[0]
for t, ch in zip(times, changed):
    gap = t - last_real; last_real = t
    if ch: active_until = t + a.keep
    clock += gap if (t <= active_until or ch) else 0.0          # still spans advance only through the hold below
    new_t.append(clock)
# Still spans collapsed to zero above; give each one a readable hold so nothing feels cut.
out_t = [new_t[0]]; idle_run = 0.0
for i in range(1, len(new_t)):
    step = new_t[i] - new_t[i - 1]; real = times[i] - times[i - 1]
    if step == 0.0:
        give = min(real, max(0.0, hold_at(times[i]) - idle_run)); idle_run += give; step = give
    else: idle_run = 0.0
    out_t.append(out_t[-1] + step)
def remap(t):
    if t <= times[0]: return 0.0
    for i in range(1, len(times)):
        if t <= times[i]:
            span = times[i] - times[i - 1]; k = 0 if span == 0 else (t - times[i - 1]) / span
            return out_t[i - 1] + k * (out_t[i] - out_t[i - 1])
    return out_t[-1]
concat = out / "frames.ffconcat"
with concat.open("w") as fh:
    fh.write("ffconcat version 1.0\n")
    for i, f in enumerate(frames):
        dur = (out_t[i + 1] - out_t[i]) if i + 1 < len(frames) else 1.0
        if dur <= 0: continue
        fh.write(f"file '{(take / 'frames' / f['name']).resolve()}'\nduration {dur:.4f}\n")
subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "concat", "-safe", "0", "-i", str(concat), "-vf", f"fps={a.fps},format=yuv420p", "-c:v", "libx264", "-preset", "slow", "-crf", "15", "-movflags", "+faststart", str(out / "take.mp4")], check=True)
timeline = [dict(e, t=round(remap(e["t"]), 3), realT=round(e["t"], 3)) for e in events if e["event"] in ("beat", "card", "click", "human_type", "agent_prompt", "agent_turn_done", "logged_in", "result")]
(out / "timeline.json").write_text(json.dumps({"realSeconds": round(times[-1] - times[0], 2), "filmSeconds": round(out_t[-1], 2), "hold": a.hold, "events": timeline}, indent=1, ensure_ascii=False))
print(f"real {times[-1]-times[0]:.1f}s -> film {out_t[-1]:.1f}s; changed frames {sum(changed)}/{len(frames)}")
