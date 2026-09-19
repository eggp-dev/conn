# Remote-takeover recording — 2026-09-19

Source behind `ConnRemoteEN`, `ConnRemoteKO` and their 15-second editions. Earlier films are documented in [RECORDING-HANDOFF.md](RECORDING-HANDOFF.md) and [RECORDING.md](RECORDING.md). The plan this follows is [SCENARIO-remote-takeover.ko.md](SCENARIO-remote-takeover.ko.md).

## What is real

- **One continuous take.** The terminal you see is never cut: no frame in which the picture changes was removed or reordered. Only spans in which the screen did not change were shortened (0.45s while the agent was thinking, up to about 3s while a card waited for the human). The take ran 65s and plays in 47s. After it ends on a still screen, the last frame is held for a few seconds while the agent's reply is shown. The 15-second edition shows three parts of the same take joined with dissolves.
- **Real product.** Conn 0.8.1: the native Rust backend and the actual Svelte frontend through the browser test adapter, default policy, local Bash shell integration active. Because `ssh` is in the foreground, every agent command gets the generic review card, exactly as it does for a user.
- **Real login.** `ssh staging` reaches an OpenSSH server in a disposable local container bound to `127.0.0.1`, with a synthetic password typed at the hidden prompt. The capture checks that the password never appears on screen.
- **Real agent.** Claude Code 2.1.278, headless, one session for the whole take (so one connection and one "wants to join"), talking to Conn only through its MCP server. Its tool calls, wording and the reasons on the cards are its own.

## What is staged

- The operator plays the human through real browser key and mouse events: types `ssh staging` and the password, clicks **Allow**, **Approve** and **Deny** on the actual cards, types `./api.sh unlock`. Click rings and the camera move are editorial overlays; product pixels are untouched.
- The message cards under the window ("You → Claude Code", and the agent's reply at the end) are editorial graphics, not a recording of the agent client. They exist to show that the agent lives in its own app. The English text is verbatim: the prompts as sent and the reply as the agent wrote it, read from the take. The Korean film shows translations of the same messages.
- The agent received a prepared brief listing the plan, including clearing the cache with `rm -rf cache/` when it found the stale lock, and telling it to stop when denied or when the human takes the keyboard. The wrong turn is staged so that it happens on camera.
- `staging`, `api.sh`, the log and the lock are fixtures. `ssh` resolves the alias through a wrapper on the film shell's `PATH`.

Visible prompts, the five cards with the agent's stated reasons, frame counts and the clip hash are in [`public/footage/remote/provenance.json`](public/footage/remote/provenance.json).

## Reproduce

Requires the repository built (`cargo build -p conn -p conn-browser-harness`), frontend dependencies, Google Chrome, Docker with an image that has `sshd` and `python3`, FFmpeg, Pillow, and an authenticated `claude` CLI. Raw frames, the agent transcript and the synthetic password stay outside the repository.

```sh
node scripts/remote/direct.mjs /path/to/repo /tmp/conn-film take        # one take; fails if the story did not complete
python3 scripts/remote/condense.py /tmp/conn-film/take/take /tmp/conn-film/edit --hold 0.45
cp /tmp/conn-film/edit/take.mp4 public/footage/remote/take.mp4
cp /tmp/conn-film/edit/timeline.json src/remote/timeline.json
python3 scripts/subset-fonts.py                                          # after changing copy
npx remotion render ConnRemoteEN ../../docs/assets/conn-remote-en.mp4    # also KO, ShortEN, ShortKO
node --experimental-strip-types scripts/export-remote-captions.mjs
```

Captions, camera and the agent's reply follow the events recorded in `timeline.json`, so a new take needs no manual retiming; after a retake only the Korean translation of the reply in `src/remote/edit.ts` needs a look. Editorial copy lives there too.

The repository address and the list of agent clients in the closing card come from [`src/brand.ts`](src/brand.ts). If the repository moves, change that one line and re-render; every film and poster in this project reads it from there.

## Limits

A local Ubuntu capture through the browser adapter. Native macOS and Windows windows, other agent clients and other shells are separate checks. The film shows one prepared scenario, not a usability study. Taking the keyboard blocks further agent input; it does not stop a process that is already running.
