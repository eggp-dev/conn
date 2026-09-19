import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { cues, footageSeconds, replySeconds, closingSeconds, remoteCopy, shortParts, shortClosingSeconds } from "../src/remote/edit.ts";

// Captions for the remote-takeover film and its 15-second edition, from the same cues the film renders.
const assets = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../docs/assets");
const stamp = (seconds, sep) => { const ms = Math.round(seconds * 1000); const p = (n, w = 2) => String(n).padStart(w, "0");
  return `${p(Math.floor(ms / 3600000))}:${p(Math.floor(ms / 60000) % 60)}:${p(Math.floor(ms / 1000) % 60)}${sep}${p(ms % 1000, 3)}`; };
const write = (name, list) => {
  writeFileSync(path.join(assets, `${name}.srt`), list.map((c, i) => `${i + 1}\n${stamp(c.start, ",")} --> ${stamp(c.end, ",")}\n${c.text}\n`).join("\n"));
  writeFileSync(path.join(assets, `${name}.vtt`), "WEBVTT\n\n" + list.map((c) => `${stamp(c.start, ".")} --> ${stamp(c.end, ".")}\n${c.text}\n`).join("\n"));
  console.log(`Exported ${name}: ${list.length} cues`);
};
for (const language of ["en", "ko"]) {
  const copy = remoteCopy[language];
  // A message between the human and the agent reads "Speaker: words"; a caption keeps its second line.
  const text = (cue) => (cue.from ? `${cue.sub}: ${cue.line.replaceAll("`", "")}` : cue.sub ? `${cue.line}\n${cue.sub}` : cue.line);
  const closing = (at, length) => ({ start: at + 0.4, end: at + length, text: `${copy.headline.join(" ")}\n${copy.disclosure}` });
  const main = cues(language).map((cue) => ({ start: cue.start, end: cue.end, text: text(cue) }));
  // The closing card starts after the held last frame on which the agent's reply is shown.
  write(`conn-remote-${language}`, [...main, closing(footageSeconds + replySeconds, closingSeconds)]);
  // The short edition plays parts of the same take back to back; move each cue onto that clock.
  let at = 0; const short = [];
  for (const part of shortParts) {
    for (const cue of cues(language)) {
      const start = Math.max(cue.start, part.from), end = Math.min(cue.end, part.to);
      if (end - start > 0.25) short.push({ start: at + start - part.from, end: at + end - part.from, text: text(cue) });
    }
    at += part.to - part.from;
  }
  write(`conn-remote-short-${language}`, [...short, closing(at, shortClosingSeconds)]);
}
