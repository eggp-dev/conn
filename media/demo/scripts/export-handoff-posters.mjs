import { spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const output = path.resolve(root, "../../docs/assets");
mkdirSync(output, { recursive: true });
for (const language of ["en", "ko"]) {
  for (const kind of ["poster", "social"]) {
    const basename = `conn-handoff-${language}-${kind}`;
    const result = spawnSync("ffmpeg", ["-hide_banner", "-loglevel", "error", "-y", "-i", path.join(root, "out", `${basename}.png`), "-c:v", "libwebp", "-quality", "90", path.join(output, `${basename}.webp`)], { stdio: "inherit" });
    if (result.status !== 0) process.exit(result.status || 1);
    console.log(`Exported ${basename}.webp`);
  }
}
