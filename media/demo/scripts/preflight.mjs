import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { clipOrder, clips, fps } from "../src/footage.ts";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
let failures = 0;
for (const name of clipOrder) {
  const clip = clips[name];
  const filename = path.join(root, "public", clip.file);
  if (!existsSync(filename)) {
    console.error(`Missing actual screencast: ${filename}`);
    failures++;
    continue;
  }
  const probe = spawnSync("ffprobe", ["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height,duration,codec_name", "-of", "json", filename], { encoding: "utf8" });
  if (probe.status !== 0) {
    console.error(`Cannot inspect ${name}: ${probe.stderr || probe.error?.message}`);
    failures++;
    continue;
  }
  const stream = JSON.parse(probe.stdout).streams?.[0];
  if (!stream || stream.width < 1920 || Math.abs(stream.width / stream.height - 16 / 9) > 0.02) {
    console.error(`${name}: expected an actual Codex CLI + Conn capture of at least 1920px wide with a 16:9 aspect ratio.`);
    failures++;
  } else {
    console.log(`${name}: ${stream.width}×${stream.height}, ${Number(stream.duration).toFixed(2)}s, ${stream.codec_name}`);
    const required = clip.trimBeforeSeconds + clip.durationInFrames / fps * clip.playbackRate;
    if (!Number.isFinite(Number(stream.duration)) || Number(stream.duration) + 1 / fps < required) {
      console.error(`${name}: source ends before the edit (${required.toFixed(2)}s required).`);
      failures++;
    }
  }
}
for (const font of ["ConnNoto-Regular.woff2", "ConnNoto-Bold.woff2"]) {
  if (!existsSync(path.join(root, "public", "fonts", font))) { console.error(`Missing font: ${font}`); failures++; }
}
if (failures) {
  console.error("Refusing to render an incomplete demo. Supply the actual Conn recordings first.");
  process.exit(1);
}
