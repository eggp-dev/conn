import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { handoffCopy } from "../src/handoff/edit.ts";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputs = [path.join(root, "out"), path.resolve(root, "../../docs/assets")];
for (const output of outputs) mkdirSync(output, { recursive: true });
const timestamp = (seconds, separator) => {
  const milliseconds = Math.round(seconds * 1000);
  return `${String(Math.floor(milliseconds / 3600000)).padStart(2, "0")}:${String(Math.floor(milliseconds / 60000) % 60).padStart(2, "0")}:${String(Math.floor(milliseconds / 1000) % 60).padStart(2, "0")}${separator}${String(milliseconds % 1000).padStart(3, "0")}`;
};
for (const language of ["en", "ko"]) {
  const copy = handoffCopy[language];
  for (const kind of ["main", "short"]) {
    const short = kind === "short";
    const cues = copy[kind].map(({ start, end, caption }) => ({ start, end, text: caption }));
    cues.push({ start: short ? 12 : 64, end: short ? 15 : 70, text: `${copy.headline.join(" ")}\n${copy.disclosure}` });
    const name = `conn-handoff-${short ? "short-" : ""}${language}`;
    for (const output of outputs) {
      writeFileSync(path.join(output, `${name}.srt`), cues.map(({ start, end, text }, index) => `${index + 1}\n${timestamp(start, ",")} --> ${timestamp(end, ",")}\n${text}\n`).join("\n"));
      writeFileSync(path.join(output, `${name}.vtt`), "WEBVTT\n\n" + cues.map(({ start, end, text }) => `${timestamp(start, ".")} --> ${timestamp(end, ".")}\n${text}\n`).join("\n"));
    }
    console.log(`Exported ${name} captions: ${short ? 15 : 70}s`);
  }
}
