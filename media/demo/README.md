# Conn collaboration film

Editable English and Korean versions of a **62-second, 1920×1080 / 30 fps** demonstration: an actual Codex CLI session and a person’s role share a Conn terminal, then the agent continues after that role adds a new requirement.

Codex CLI generates the requests through Conn’s MCP server. Its output appears on the left; Conn’s actual Svelte interface and Rust backend appear on the right. The recording operator automates the human-role prompts, clicks and terminal input; **there is no independent human participant**. Waiting is shortened. The edit adds titles and captions to reserved gutters, with a fixed full view and **no zooms or pans**. It does not recreate product UI or invent terminal output.

## Edit and render

Requires Node.js 22.6+, FFmpeg and FFprobe. From this directory:

```sh
npm ci
npm run check
npm run dev
```

Open the Studio URL. `ConnDemoEN` and `ConnDemoKO` share footage and timing; the fonts are bundled locally.

```sh
npm run preflight
npm run render:en
npm run render:ko
npm run poster
npm run captions
ffmpeg -y -i out/conn-demo-poster.png -c:v libwebp -quality 88 ../../docs/assets/conn-demo-poster.webp
```

Videos and SRT/WebVTT captions go to `../../docs/assets/`. Outputs are silent H.264 at 30 fps, CRF 19 and 4:2:0 color. The first render may download Remotion’s headless Chromium; an installed browser can be selected with `--browser-executable` on a direct Remotion command.

## Source and story

Actual source clips are in [`public/footage/collaboration/`](public/footage/collaboration/). Their [provenance](public/footage/collaboration/provenance.json) records the recording and selected intervals. [RECORDING.md](RECORDING.md) explains the real session, reproducible setup and limitations.

| Clip | Duration | What happens |
|---|---:|---|
| `intro.mp4` | 8 s | Ask the existing Codex CLI to help with the failure visible in Conn |
| `request.mp4` | 9 s | Codex requests access through MCP; the human role reviews it |
| `fix.mp4` | 11 s | Codex corrects the calculation; two tests pass |
| `reclaim.mp4` | 11 s | The human role adds a 125% discount test directly in Conn; it fails |
| `resume.mp4` | 13 s | Codex reads the new state, handles the additional case; three tests pass |
| `timeline.mp4` | 6 s | Review the shared activity and request details |
| Closing card | 4 s | Product message and recording disclosure |

`src/footage.ts` defines the 1,860-frame edit. `src/copy.ts` contains both languages. The composition shows the complete captured frame at a constant scale. Long waits and setup detours may be omitted, preserving the order of requests, decisions and results. A hold may retain an actual recorded frame; it must not fabricate a later state.

The earlier `public/footage/*.mp4` clips and `scripts/agent-driver.py` belong to the first scripted RPC demo. **They are not used by this edit.**

## Review before publication

- Compare the requested commands and results against the source captures and provenance.
- Confirm the external client is actual Codex CLI and the new test appears before the second fix.
- Watch both languages at full size and a 960px playback width; captions must not cover either terminal.
- Keep the automated-human-role and shortened-waits disclosure in the film and documentation.
- Review raw recordings for personal information; do not publish local authentication or transport credentials.

## Fonts and tooling

`public/conn-icon.svg` comes from Conn. `public/fonts/ConnNoto-*.woff2` are Noto Sans CJK KR subsets under SIL Open Font License 1.1; see `public/fonts/NOTO-LICENSE.txt`. After changing copy, run `python3 scripts/subset-fonts.py` with `fonttools` and `brotli` installed; `--regular` and `--bold` select the source font collections.

This directory is optional production tooling and adds no dependencies to the application. Remotion’s license applies separately.
