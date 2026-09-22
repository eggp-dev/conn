# Recording the Conn collaboration demo

## What is real, and what is staged

Recorded on Ubuntu on September 16, 2026, using **Codex CLI 0.153.0, gpt-6-astra, medium reasoning**, connected to Conn’s real MCP stdio server. Codex generated its own commands, read the resulting terminal state and adapted after another test was added. File inspection, edits and test commands in the task went through Conn MCP; Codex’s built-in execution tool was not used for them.

The left pane is the real CLI’s PTY rendered with xterm. The right pane is Conn’s existing Svelte UI connected to its Rust Engine and native PTY through the browser test adapter. The capture wrapper arranges those two surfaces; it does not implement another Conn UI. `conn mcp --tools static` exposes a stable tool list; the client’s normal initialization supplies its agent identity.

The operator scripted the scenario and automated the **human role**: entering prompts in Codex, approving requests, and typing into Conn. No independent person was recorded. This is a live demonstration with an external model, not an unscripted usability study. The film shortens waiting and may omit setup detours, including the actual first attempt to use unavailable `python` followed by recovery with `python3`. The final edit holds a fixed view, with no zooms or pans.

The [six source clips](public/footage/collaboration/) and their [provenance](public/footage/collaboration/provenance.json) identify the footage used. Original JPEG frames, timestamp manifests and private CLI/RPC logs remain outside public assets. Do not publish logs or `connection.json`: they can contain private text or a local transport token.

## Reproduce the isolated session

The recorded film used the pre-refactor browser adapter. That adapter has been removed. Current recordings must use the [production web host](../../docs/browser-testing.md), its compiled shared UI and the matching persistent `conn mcp` process. The fixture and human-role script below remain useful; the old Vite proxy commands are historical and are no longer an executable setup guide.

Build `cargo build -p conn -p conn-web --locked` and `npm run build:web` at the repository root. Use a fresh state directory, copy `fixture/` into its workspace and select that workspace through a local shell profile. Start `conn-web serve --state-dir <state> --ui-dir frontends/web/dist`, authenticate the owner using its private connection file, and configure Codex with `target/debug/conn --socket <state>/conn.sock mcp --tools static`.

The split capture helper can embed the web host's own origin; do not use `--proxy-conn` to introduce a second owner transport. Enter the connection code in the embedded Conn UI. Record the source commit, executable versions and actual origin for a new take. Existing film provenance describes the original footage and must not be rewritten as evidence for the refactor.

## Perform the actual collaboration

Use normal visible UI input in the two panes. Approve only the requests belonging to this disposable fixture.

| Step | Interaction and evidence |
|---|---|
| Start | In Conn, run `python3 -m unittest -v`. Two tests run; the percentage test fails (`144.0 != 96`). |
| Ask | In Codex: “Help me fix the failing discount test in Conn.” |
| First fix | Review its actual request. The recorded agent changed addition to subtraction and verified two passing tests. Do not inject a prewritten command stream as the agent. |
| Human addition | Type `cp cases/test_cap.py .` directly in Conn, then `python3 -m unittest -v`. The new 125% discount case exposes a negative price. |
| Continue | In Codex: “I added an edge case. Read the terminal and continue.” Do not supply the implementation answer. |
| Adapt | The recorded agent read the snapshot and new test, clamped the calculation at zero, verified three passing tests, then released control. |
| Review | Open Conn’s timeline to inspect the shared activity and original request details. |

`fixture/cases/test_cap.py` starts outside unittest’s top-level discovery. The direct copy is the human role’s real workspace change; the second Codex turn must observe it before making the additional fix. The selected fixture is nondestructive. The former deletion/denial vignette is not part of this film.

## Capture, edit and verify

Capture the whole split view at **1920×1080**, keeping both panes visible and the top/bottom gutters clear for editorial titles. This recording used a 1920×1080 CSS viewport at device scale 1; verify the captured JPEG dimensions before recording. Use Conn’s Settings for its font size; the bridge renders CLI text at 22 CSS pixels. Do not change viewport or zoom within a take.

The recording uses the browser’s `Page.startScreencast` CDP API, JPEG quality 94, `maxWidth: 1920`, `maxHeight: 1080`, and every frame. Acknowledge each `Page.screencastFrame`, preserve `metadata.timestamp`, and save a `capture.json` manifest with `startedAt`, `endedAt`, `truncated` and `frames` (`path`, `timestamp`, `receivedAt`). Keep frame collection and UI actions awaited within the same active capture invocation. An unchanged display can produce few events; retaining its actual image over that interval is not generated footage.

Assemble the reviewed multi-cut edit with the included helper and published edit map:

```sh
python3 media/demo/scripts/assemble-capture.py /path/to/raw \
  media/demo/public/footage/collaboration/edit-map.json \
  media/demo/public/footage/collaboration
```

For a single selected real interval:

```sh
python3 media/demo/scripts/encode-capture.py /path/to/take \
  media/demo/public/footage/collaboration/request.mp4 \
  --start 0 --duration 9 --width 1920 --height 1080
```

Choose the interval from the recorded timestamps. `--overwrite` replaces an intentional export. The helper uses timestamps, exports H.264/30 fps, preserves aspect ratio and limits a final-frame hold to the chosen interval. Selected cuts may remove waiting or setup recovery, but must not reorder a decision before its request or a passing result before its command.

Regenerate provenance and hashes whenever clips change. Verify first/last frames, requests, the newly copied test, both test results, and CLI continuation against the source. Run `npm run preflight` and the checks in [README.md](README.md), decode both finished videos, and inspect English/Korean captions at playback size. The final edit is 62 seconds with fixed framing, not a continuous wall-clock recording.
