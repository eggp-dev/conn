import { existsSync, readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { handoffClips, handoffCopy, handoffDuration, handoffFps, shortDuration, hookTrim, shortTrim } from "../src/handoff/edit.ts";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const errors = [];
for (const [name, clip] of Object.entries(handoffClips)) {
  const filename = path.join(root, "public", clip.file);
  if (!existsSync(filename)) { errors.push(`Missing actual capture: ${clip.file}`); continue; }
  const probe = spawnSync("ffprobe", ["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height,duration", "-of", "json", filename], { encoding: "utf8" });
  if (probe.status !== 0) { errors.push(`Cannot probe ${clip.file}`); continue; }
  const stream = JSON.parse(probe.stdout).streams?.[0];
  if (stream?.width !== 1920 || stream?.height !== 1080) errors.push(`${name} must be 1920 × 1080`);
  const required = name === "handoff" ? Math.max(clip.trim + clip.duration, hookTrim + 6, shortTrim + 6) : clip.trim + clip.duration;
  if (!(Number(stream?.duration) + 1 / handoffFps >= required)) errors.push(`${name} needs ${required}s of real source footage`);
  console.log(`${name}: ${stream?.width}×${stream?.height}, ${stream?.duration}s`);
}
for (const filename of ["footage/handoff/provenance.json", "footage/handoff/edit-map.json", "footage/handoff/poster.png", "fonts/ConnNoto-Regular.woff2", "fonts/ConnNoto-Bold.woff2"]) {
  if (!existsSync(path.join(root, "public", filename))) errors.push(`Missing ${filename}`);
}
const metadata = path.join(root, "public/footage/handoff/provenance.json");
if (existsSync(metadata)) {
  try { JSON.parse(readFileSync(metadata, "utf8")); } catch { errors.push("Provenance must be valid JSON"); }
}
for (const language of ["en", "ko"]) {
  for (const [kind, end] of [["main", 64], ["short", 12]]) {
    const cues = handoffCopy[language][kind];
    let cursor = 0;
    for (const cue of cues) {
      if (cue.start !== cursor || cue.end <= cue.start || !cue.title || !cue.caption) errors.push(`${language}/${kind}: discontinuous or empty caption`);
      cursor = cue.end;
    }
    if (cursor !== end) errors.push(`${language}/${kind}: caption coverage must end at ${end}s`);
  }
}
if (handoffDuration !== 70 * handoffFps || shortDuration !== 15 * handoffFps) errors.push("Composition durations do not match the approved edit");
if (errors.length) {
  console.error(errors.join("\n"));
  console.error("Refusing to render: real source footage and provenance are required.");
  process.exit(1);
}
console.log("Handoff sources, caption timing, fonts and provenance are ready.");
