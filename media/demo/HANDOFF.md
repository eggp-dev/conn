# Shared terminal, human correction

The primary Conn film shows a directory correction in one shared shell. The agent checks `draft` and waits; the human role switches to `workspace`; the agent observes the changed state before creating and reading `collaboration.txt` there. The existing `ConnDemoEN` / `ConnDemoKO` test-repair film remains available as a separate example.

## Edit contract

- Main: 70 seconds, 1920 × 1080, 30 fps, English and Korean.
- Short: 15 seconds, same source and languages.
- Stable split view, Codex CLI on the left, Conn on the right. No camera zooms or pans.
- Titles use the source's 72px top gutter. Captions use the 80px bottom gutter. Neither covers terminal output.
- The hook repeats a real correction moment, then explicitly returns to “A moment earlier.”
- Human correction, fresh observation and continuation stay in order. Cuts that shorten waiting must be recorded in `public/footage/handoff/edit-map.json`.
- All product pixels and terminal output come from actual recordings. The human role is automated and disclosed in both languages.

| Main interval | Source | Purpose |
|---|---|---|
| 0–6 s | Actual correction excerpt | Lead with the value |
| 6–25 s | `request.mp4` | Existing agent reads the initial shell |
| 25–55 s | `handoff.mp4` | Human changes directory; agent rereads and continues |
| 55–64 s | `result.mp4` | Confirm the actual file and destination |
| 64–70 s | Editorial closing | Download link and recording disclosure |

`src/handoff/edit.ts` is the bilingual timing/copy source. The short film shows the first six seconds of the human correction in `handoff.mp4`, then cuts to six seconds of the actual final answer and output in `result.mp4`, followed by its three-second closing card. The longer film shows the intervening observation and approvals. Waiting is shortened in both versions and disclosed; no terminal events are invented.

## Prepare and render

```sh
npm ci
npm run check
python3 scripts/subset-fonts.py
npm run preflight:handoff
npm run dev
```

Open the printed Studio URL and select `ConnHandoffEN` or `ConnHandoffKO`. Preflight deliberately fails until fresh clips, provenance, edit map and poster frame are available. It never falls back to older footage.

```sh
npm run render:handoff:en
npm run render:handoff:ko
npm run render:handoff:short:en
npm run render:handoff:short:ko
npm run poster:handoff
npm run captions:handoff
```

For an installed Chromium, append `-- --browser-executable=/path/to/browser` to individual render scripts. Main and short H.264 files plus SRT/VTT captions go to `docs/assets/`. Posters/social images render first to `out/` and convert to WebP in the same assets folder.

## Verification

Watch both complete films and the short cuts. Compare every state claim with the captured source and retained event map. Inspect full resolution and a 960px playback width; verify Korean glyph coverage and terminal readability, and ensure subtitles never cover the product. This is an automated-role reconstruction of an actual collaboration experience, not a user study or an independent human test.
