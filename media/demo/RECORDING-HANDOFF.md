# Directory-correction recording — 2026-09-17

This records the source behind `ConnHandoffEN`, `ConnHandoffKO` and their short editions. The previous test-repair session remains documented in [RECORDING.md](RECORDING.md).

## Environment and scope

- Ubuntu; native Conn Rust backend with the actual Svelte frontend through the local browser test adapter.
- Source revision: `b23a92a7b3854a5c94f1043e1f5dcb562eb50906`. `git diff v0.6.0 -- crates frontends/tauri/src` was empty; the release tag resolves to `7421968c1b9670a007107cef693c97d270632eab`.
- Codex CLI 0.153.0, gpt-6-astra, medium reasoning, Conn MCP stdio. The capture process uses the installed client's normal authentication; credentials are not copied into the project or video assets.
- Isolated disposable app state and two sibling directories, `draft` and `workspace`. See the [public exercise](../../examples/first-collaboration/README.md).
- Fixed 1920×1080 split view: external Codex on the left, Conn on the right, 72px top and 80px bottom editorial gutters. Terminal font sizes: Codex 22px, Conn 20px.
- Autopilot with the control gate enabled; 2s execution grace, 40ms typing interval, 300s lease.

The operator automates the human role. This is a reconstruction of an observed collaboration, not a recording of an independent user or a usability study. Product pixels, model output and terminal output are real.

## Task and evidence

The capture instructions tell Codex to use Conn only, run `pwd`, retain control and wait. After the human correction, they define an exclusive one-file task and the checks below. They do not queue agent commands.

Visible prompts:

1. `Use Conn to check our current directory. Keep control and wait before making changes.`
2. `I changed the directory. Read the terminal and continue here.`

The second prompt relies on the prepared continuation task. The public first-collaboration guide includes that task explicitly for fresh sessions.

The operator clicks the actual control and command-approval buttons and types `cd ../workspace`, then `pwd`, in Conn. Codex reads a fresh snapshot, requests control, chooses Python 3 exclusive file creation, reads the file, checks the original directory and releases control. Both control permission and policy approval occur in the actual backend.

Independent post-session checks:

- `workspace/collaboration.txt` bytes equal `b"We continued from your correction.\n"`.
- `draft/collaboration.txt` does not exist.
- The actual MCP result for `terminal_release_control` is `{"released": true}`.

These are local checks of this scenario, not proof of every client or OS path. Taking over blocks future writes; it does not terminate an already running process. The timeline stores commands and control events, not terminal output.

## Capture and edit

The [existing capture setup](RECORDING.md) supplies the native browser harness, Vite frontend and `scripts/cli-capture-server.py`. Use the first-collaboration fixture and prompts above instead of the previous discount exercise. Configure the test origin to the capture bridge's origin. Set the Conn font before connecting MCP; reloading the frontend can invalidate a connection.

This recording uses a dedicated Chromium and CDP `Page.startScreencast` at JPEG quality 94. The private take contains 22,848 frames over 571.126s, including initial setup/reconnection that is excluded from the film. The encoder manifest stores `startedAt` and `endedAt` in milliseconds and individual frame timestamps in seconds.

The public `public/footage/handoff/edit-map.json` records these original-time selections, at normal speed:

| Clip | Selected intervals in the private take | Duration |
|---|---|---:|
| request | 158–164s; 175–188s | 19s |
| handoff | 222.892–236.892s; 239.1–243.1s; 310.813–315.813s; 397.943–404.943s | 30s |
| result | 510–519s | 9s |

The first approval click precedes the selected request clip. The later control request and file-creation approval are visible. The handoff selections preserve correction → fresh observation → renewed control → approved creation. The result shows the actual final response and terminal after the checks and release.

The main edit previews the correction as a hook, labels its return to “A moment earlier,” then proceeds in order. The short cut shows the first six seconds of the correction, six seconds of the real result, and a three-second closing card. Both disclose shortened waits and the automated user role. No camera zoom, recreated UI, fabricated output or simulated model response is used.

From this directory, assemble a retained private take with:

```sh
python3 scripts/assemble-capture.py /path/to/private-take public/footage/handoff/edit-map.json public/footage/handoff
npm run preflight:handoff
```

The source directory includes `provenance.json`, `clip-hashes.json` and an actual result frame as `poster.png`. Editorial timing and bilingual captions live in `src/handoff/edit.ts`. See [HANDOFF.md](HANDOFF.md) for final rendering.

## Private material and limits

Raw frames, raw model transcripts, socket paths and transport configuration remain outside the repository. Only selected reviewed clips and public provenance belong in the release package. Never publish the local connection token, client authentication, account headers or raw terminal logs.

This is a local Ubuntu capture using the browser adapter and native backend. Native macOS/Windows installation and remote interactive-shell compatibility are separate checks. The original remote experiment's unresolved `input_pending` case is documented in the public collaboration story; this film does not claim to resolve it.
