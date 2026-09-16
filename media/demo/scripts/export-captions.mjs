import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { copy, closingCopy } from "../src/copy.ts";
import { clipOrder, clips, closingDurationInFrames, fps } from "../src/footage.ts";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputs = [path.join(root, "out"), path.resolve(root, "../../docs/assets")];
for (const output of outputs) mkdirSync(output, { recursive: true });
const timestamp = (milliseconds, separator) => {
  const total = Math.round(milliseconds);
  return `${String(Math.floor(total / 3600000)).padStart(2, "0")}:${String(Math.floor(total / 60000) % 60).padStart(2, "0")}:${String(Math.floor(total / 1000) % 60).padStart(2, "0")}${separator}${String(total % 1000).padStart(3, "0")}`;
};
for (const language of ["en", "ko"]) {
  const scenes = clipOrder.map((id) => ({ text: copy[language][id].caption, duration: clips[id].durationInFrames }));
  scenes.push({ text: `${closingCopy[language].title}\n${closingCopy[language].disclosure}`, duration: closingDurationInFrames });
  let current = 0;
  /** @type {import("@remotion/captions").Caption[]} */
  const captions = scenes.map(({ text, duration }) => {
    const startMs = current / fps * 1000;
    current += duration;
    return { text, startMs, endMs: current / fps * 1000, timestampMs: null, confidence: null };
  });
  for (const output of outputs) {
    writeFileSync(path.join(output, `conn-demo-${language}.srt`), captions.map(({ startMs, endMs, text }, index) => `${index + 1}\n${timestamp(startMs, ",")} --> ${timestamp(endMs, ",")}\n${text}\n`).join("\n"));
    writeFileSync(path.join(output, `conn-demo-${language}.vtt`), "WEBVTT\n\n" + captions.map(({ startMs, endMs, text }) => `${timestamp(startMs, ".")} --> ${timestamp(endMs, ".")}\n${text}\n`).join("\n"));
  }
  console.log(`Exported ${language} captions: ${current / fps}s`);
}
