# Conn launch kit

English · [한국어](launch-kit.ko.md) · [Launch plan](launch-plan.md)

Ready-to-review copy. These drafts have not been posted. Replace a local media reference with its verified public attachment only after upload; do not invent an attachment URL.

## Message

**Keep your agent. Share your terminal.**

Work in the same shell as your AI agent. Step in, make a correction, and ask it to continue from what you changed.

Primary action: [Download Conn](https://github.com/eggplantiny/conn#download). Next: [Try the first collaboration](first-collaboration.md).

## Short introduction

> My agent chose a working directory. I wanted a different one, so I took the keyboard, changed it, and said: “Read the terminal and continue from here.”
>
> That's the interaction behind Conn: a shared terminal for you and the agent you already use. The conversation stays in your client. You can see the same shell, step in directly, then ask the agent to read your changes and resume.
>
> Conn is an open-source desktop preview for Apple Silicon macOS, Windows x64 and Ubuntu x64. It includes setup for Codex, Claude Code, Cursor and GitHub Copilot. Try the small local example, and tell me where the handoff felt confusing.
>
> https://github.com/eggplantiny/conn

Suggested companion: the short directory-correction clip. Keep the video disclosure nearby: **Actual Codex + Conn; prepared scenario, automated user role, shortened waits.**

## Show HN draft

**Title:** Show HN: Conn — share a terminal with your AI agent and take over by typing

**Destination:** https://github.com/eggplantiny/conn

**Introductory comment:**

> I built Conn because I wanted to work alongside my agent in the same shell.
>
> In a recent session, it was about to use a directory I didn't want. I took control, changed the path myself, and asked it to read the screen again. It continued from the corrected location. That small correction is the experience I want Conn to make easy.
>
> Conn is a Rust/Tauri terminal with MCP and CLI connections. You keep your existing agent client and model account. Agent control requests and command policy are separate, direct typing returns terminal control to you, and a timeline brings commands and collaboration decisions together.
>
> It's a preview, not an OS sandbox. Taking control prevents later agent input; it doesn't stop an already running process. Ordinary terminal snapshots can include whatever is visible on screen.
>
> The README has native downloads and a local first-collaboration example. The demo uses an actual Codex session with the user role automated to recreate the interaction; waits are edited. I'd appreciate feedback on whether the first connection and handoff make sense without explanation.
>
> When did you last want to take the keyboard during an agent's terminal task?

Check the [Show HN guidelines](https://news.ycombinator.com/showhn.html) at posting time. Use a real working download, identify yourself as the maker, and be available to discuss the product. Do not ask friends to vote or seed comments.

## Release / project update blurb

> **A small correction, without starting over.** The new Conn walkthrough shows an agent preparing work, a user changing the destination, and the agent reading the updated terminal before continuing. The English and Korean README now lead to a small local example you can try with your own agent.
>
> Download the desktop preview, connect your client in Settings → Agents, and try the handoff. Existing v0.6.0 downloads remain the application release; this update refreshes the walkthrough and documentation.

Use only after reviewing the final assets. This copy does not announce an unbuilt application version.

## A case-study opening

**Title:** I changed the directory. My agent continued from there.

> During a remote setup session, the agent started preparing a project in a different location from the one I wanted. Explaining another path in chat was possible. Instead, I typed in the shared terminal, took control and put the shell in the right place. Then I asked the agent to read the terminal again.
>
> It saw the new state and continued. The useful part wasn't the command itself. It was being able to participate in the work directly, without reconstructing the session in a message.

Continue with three public screenshots: before the change, direct intervention, and the verified result. Use a disposable recreation rather than revealing the original account, host, private path or authentication screen. Separate what worked from unresolved parts of the original remote experiment. The [recorded observations](collaboration-backlog/2026-09-16-remote-codex.md) are the factual source, not a claim of universal compatibility.

## Replies to common questions

**“Why not my existing terminal or agent tool?”**

> Keep them if they already fit your workflow. Conn focuses on using your existing agent alongside you in the same shell: you can change the working state directly, then ask it to read that state and continue. Try the small example to see whether that helps your work.

**“Is it safe to give the agent my terminal?”**

> Conn's review and takeover controls help coordinate a trusted agent; they aren't OS isolation. Commands use your user account, and permitted snapshots can include visible terminal output. The trust model explains the boundaries. Use a disposable example first.

**“Does it work with my client?”**

> Setup is provided for local Codex desktop/CLI/IDE tasks, Claude Code, Cursor and GitHub Copilot. It is not a connection for the separate ChatGPT app or cloud tasks. The lead recording uses Codex on Ubuntu; it doesn't establish every client/OS combination. Please include your client, version and OS if setup fails.

**“I tried it and got stuck.”**

> At which step: installing, connecting, granting control, taking over, or continuing? A small public example plus Conn/client versions, OS and shell will help. Please review screenshots and request text before sharing; credentials and full terminal dumps aren't needed.

## Five-person first-use review

Ask each participant, with their consent:

1. Show the short clip without narration: “What does Conn let you do?”
2. Give them the README: “Install it and connect the agent you already use.”
3. Ask them to follow [the first collaboration](first-collaboration.md), without coaching.
4. Ask: “What did typing change? Did you expect it to stop a running command?”
5. After one week, ask whether they used Conn for real work, and what happened.

Record only the step, elapsed time, assistance and voluntary feedback. Do not collect terminal contents or credentials. Separate observed completion from the participant saying they understood. The sample is a practical usability check, not a performance benchmark.

## Assets and final checks

- [English main film](assets/conn-handoff-en.mp4) · [Korean main film](assets/conn-handoff-ko.mp4)
- [English short](assets/conn-handoff-short-en.mp4) · [Korean short](assets/conn-handoff-short-ko.mp4)
- [English poster](assets/conn-handoff-en-poster.webp) · [Korean poster](assets/conn-handoff-ko-poster.webp)
- [English social cover](assets/conn-handoff-en-social.webp) · [Korean social cover](assets/conn-handoff-ko-social.webp)
- [Recording notes](demo.md) · [Editable media project](../media/demo/README.md)
- Verify final download links, captions, disclosure and location/result continuity.
- Keep browser-adapter footage distinct from native installer verification.
- Check the target community's current rules and tailor one post to its audience.
- Publish only when someone can answer questions. No bulk unsolicited outreach or manufactured engagement.

### Actual capture stills

- [Directory correction](assets/conn-collaboration-correction.webp)
- [Verified result](assets/conn-collaboration-result.webp)
- [Timeline](assets/conn-collaboration-timeline.webp)
