# Recording the Conn collaboration demo

## What is real, and what is staged

Recorded on Ubuntu on September 16, 2026, using **Codex CLI 0.153.0, gpt-6-astra, medium reasoning**, connected to Conn’s real MCP stdio server. Codex generated its own commands, read the resulting terminal state and adapted after another test was added. File inspection, edits and test commands in the task went through Conn MCP; Codex’s built-in execution tool was not used for them.

The left pane is the real CLI’s PTY rendered with xterm. The right pane is Conn’s existing Svelte UI connected to its Rust Engine and native PTY through the browser test adapter. The capture wrapper arranges those two surfaces; it does not implement another Conn UI. `conn mcp --tools static` exposes a stable tool list; the client’s normal initialization supplies its agent identity.

The operator scripted the scenario and automated the **human role**: entering prompts in Codex, approving requests, and typing into Conn. No independent person was recorded. This is a live demonstration with an external model, not an unscripted usability study. The film shortens waiting and may omit setup detours, including the actual first attempt to use unavailable `python` followed by recovery with `python3`. The final edit holds a fixed view, with no zooms or pans.

The [six source clips](public/footage/collaboration/) and their [provenance](public/footage/collaboration/provenance.json) identify the footage used. Original JPEG frames, timestamp manifests and private CLI/RPC logs remain outside public assets. Do not publish logs or `connection.json`: they can contain private text or a local transport token.

## Reproduce the isolated session

Requirements: Conn’s build dependencies, installed frontend dependencies, Python 3, Git, FFmpeg/FFprobe, and an installed, authenticated Codex CLI. These capture helpers use Unix PTYs and run on Linux/macOS. Use a fresh state directory, not an existing personal shell session. Run from the repository root:

```sh
cargo build -p conn -p conn-browser-harness --locked
export CONN_FILM_REPO="$PWD"
export CONN_FILM_STATE="$(mktemp -d /tmp/conn-film-XXXXXX)"
python3 - <<'PY'
import json, os, pathlib, shutil, subprocess
state = pathlib.Path(os.environ['CONN_FILM_STATE'])
repo = pathlib.Path(os.environ['CONN_FILM_REPO'])
workspace = state / 'workspace'
shutil.copytree(repo / 'media/demo/fixture', workspace)
(state / 'operator').mkdir()
profile = {
    'version': 1, 'revision': 0, 'defaultProfile': 'film',
    'profiles': [{
        'id': 'film', 'name': 'checkout-demo', 'enabled': True,
        'backend': 'local', 'shell': 'posix', 'program': '/bin/bash',
        'args': ['--noprofile', '--norc', '-i'], 'cwd': str(workspace),
        'env': {'PS1': 'demo $ ', 'HISTFILE': '/dev/null',
                'PYTHONDONTWRITEBYTECODE': '1', 'GIT_PAGER': 'cat', 'PAGER': 'cat'}
    }]
}
defaults = {'mode': 'autopilot', 'gate': True,
    'pacing': {'minWriteIntervalMs': 40, 'enterGraceMs': 2000,
               'leaseTtlSecs': 120, 'approvalTtlSecs': 300}}
(state / 'profiles.json').write_text(json.dumps(profile, indent=2))
(state / 'app.json').write_text(json.dumps(defaults, indent=2))
subprocess.run(['git', 'init', '--quiet', str(workspace)], check=True)
subprocess.run(['git', '-C', str(workspace), 'add', '.'], check=True)
subprocess.run(['git', '-C', str(workspace), '-c', 'user.name=Conn Demo',
    '-c', 'user.email=demo@example.invalid', 'commit', '-qm', 'Add checkout fixture'], check=True)
print(f'CONN_FILM_STATE={state}')
PY
CONN_TEST_PORT=1433 CONN_TEST_ORIGIN=http://127.0.0.1:1435 \
  target/debug/conn-browser-harness "$CONN_FILM_STATE"
```

In a second terminal, set `CONN_FILM_REPO` and `CONN_FILM_STATE` to the same values. Start the unchanged frontend:

```sh
cd "$CONN_FILM_REPO/frontends/tauri"
CONN_TEST_STATE="$CONN_FILM_STATE" \
  node node_modules/vite/bin/vite.js --mode browser-test --host 127.0.0.1 --port 1431
```

In a third terminal, set those same variables. Write instructions that route the task through Conn without prescribing the fix:

```sh
cat > "$CONN_FILM_STATE/agent-instructions.md" <<'PROMPT'
For this demonstration, perform task file inspection, edits and commands only through Conn MCP. Work in the shared terminal's current workspace, not this CLI's operator directory. Do not use built-in shell execution or direct file editing for the task.
Read the current terminal snapshot first. Request control with the exact planned command and a clear reason. Wait for approval; then type the command and submit Enter through Conn. Read the resulting snapshot: delivery of Enter does not prove command success.
Stop on denial or human takeover. When the user asks you to continue, read the terminal and changed files again before deciding what to do. After the initial two tests pass, pause and keep the lease so the user can take over by typing. Do not inspect cases/ before the user adds another test. On the follow-up, finish the task and release control. Keep responses concise and in English.
PROMPT
python3 - <<'PY'
import json, os, pathlib
state = pathlib.Path(os.environ['CONN_FILM_STATE'])
# Arguments are passed as a list; no shell interpolation of instructions or paths.
launcher = '''import json, os, pathlib, shutil
state = pathlib.Path(os.environ['CONN_FILM_STATE'])
repo = pathlib.Path(os.environ['CONN_FILM_REPO'])
cli = shutil.which('codex')
if not cli: raise SystemExit('Install and authenticate Codex CLI first')
args = [cli, '--no-alt-screen', '-C', str(state / 'operator'),
        '-s', 'read-only', '-a', 'on-request', '-m', 'gpt-6-astra']
config = {
    'model_reasoning_effort': 'medium',
    'mcp_servers.conn.command': str(repo / 'target/debug/conn'),
    'mcp_servers.conn.args': ['--socket', str(state / 'conn.sock'), 'mcp', '--tools', 'static'],
    'mcp_servers.conn.tool_timeout_sec': 600,
    'developer_instructions': (state / 'agent-instructions.md').read_text(),
    'check_for_update_on_startup': False,
}
for key, value in config.items(): args.extend(['-c', key + '=' + json.dumps(value)])
os.execv(cli, args)
'''
(state / 'launch-codex.py').write_text(launcher)
PY
cd "$CONN_FILM_REPO"
python3 media/demo/scripts/cli-capture-server.py --proxy-conn \
  --cwd "$CONN_FILM_STATE/operator" --record "$CONN_FILM_STATE/codex-pty.jsonl" \
  -- python3 "$CONN_FILM_STATE/launch-codex.py"
```

Open `http://127.0.0.1:1435/`. The bridge proxies required Vite assets from port 1431 so both panes share an origin; Conn’s WebSocket connects to port 1433, whose permitted Origin must be port **1435**. All three listeners are loopback-only. The bridge starts one CLI process, forwards browser input and terminal size, and retains bounded output for reconnects. Its log is created with mode `0600` and refuses to overwrite an existing file. Stop/relaunch with a fresh log when starting another take.

The `-c` settings apply to this Codex process; they do not rewrite global MCP configuration. Codex may record its normal workspace trust decision. Select an available model if `gpt-6-astra` is unavailable; generated commands and wording will vary. Instructions route this demonstration through Conn, but do not form a security sandbox or prevent another tool from being used. The isolated fixture protects real work by scope, not by filesystem confinement.

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
