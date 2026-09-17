# Questions before your first collaboration

English · [한국어](faq.ko.md) · [Try it](first-collaboration.md) · [Back to Conn](../README.md)

## Do I need another AI subscription?

Conn does not run a model or sell model access. Keep using your agent client and its account. Model availability, billing and the client's permissions remain with that client.

## Where do I talk to the agent?

In Codex, Claude Code, Cursor, or GitHub Copilot. Conn supplies the shared terminal through MCP or CLI. The client can read the screen, request control and work in that shell. After you intervene, ask it to read the terminal again before continuing.

The desktop can set up those local clients for you. Codex means its desktop app, CLI, or IDE extension running a local task. Registering `~/.codex/config.toml` does not add a connection to the separate ChatGPT app; web/cloud tasks cannot reach this local stdio server. See [agent integrations](agent-integrations.md).

## Why use a shared terminal?

You can correct the working state directly: change the directory, add a missing test, or finish an interactive step. The agent can then read that changed state instead of relying on a pasted summary. It is useful when you want to stay involved in the same shell session; an unattended agent job may not need this workflow. [Read the collaboration story](collaboration-story.md).

## Does typing stop the agent's command?

Typing reclaims terminal control and prevents subsequent agent input. It does **not** kill a process already running or undo work it completed. Use normal terminal controls, such as Ctrl-C where appropriate, to interrupt that process. Conn also cannot stop an agent from using a separate tool outside Conn.

Control approval and command approval are separate: allowing an agent to take control does not bypass command policy.

## What should I try first?

[Change the working directory together](first-collaboration.md). The example uses a disposable local workspace, one small file and your existing agent. No SSH server or remote machine is needed. A successful handoff means the agent checks the new location and finishes there.

## Can the agent see passwords?

In an ordinary session, an agent allowed to take snapshots can see what is on the terminal screen, including in Observe mode. Hidden password input is not reconstructed into command history, but a program that echoes a secret can put it on screen. Masking with `*****` is the child program's behavior, not a Conn secret detector.

Explicit agent commands, intents and original request parameters can be stored in audit and timeline records. Do not put credentials in them. Restoring snapshot access can expose earlier output still visible on screen. See the [trust model](security.md).

## What gets recorded in the timeline?

Agent commands and collaboration decisions, including original requests, approvals and denials. Integrated local Bash/Zsh sessions also record commands when execution begins and their completion when available. Conn does not derive human command history from raw typing, password entry or editor input.

The timeline is not a screen recording or a backup of running processes. Commands can themselves contain sensitive arguments. Unsupported shells and conflicting shell hooks may have no human command history; there is no fallback to keystroke collection. [Shell integration](shell-integration.md)

## Does collaboration still work over SSH?

Yes, within the ordinary session's existing screen and control permissions. Conn drives the SSH client in the shared terminal. Local shell integration records the outer `ssh` command; it does not install remote hooks or automatically record the remote shell's human commands. Remote profiles require command review. [Profiles and boundaries](backends.md)

## Are external automation sessions shared too?

External private sessions have a different purpose. They are excluded from agent discovery, snapshots and control, and do not produce Conn activity history. Taking over as a human does not turn them into shared sessions. Child output, shell history and other OS storage remain separate surfaces. [External automation](external-automation.md)

## How do updates work?

From v0.6.0, Conn checks and downloads signed updates in the background. Choose **Install and restart** when ready; installing closes current shell sessions. Use **Check for updates → Update preferences** to change automatic behavior.

In-app installation supports Apple Silicon macOS, Windows x64 and Linux AppImage. `.deb` uses a new package or package manager. Older 0.5.x installs need one manual upgrade to 0.6.0. [Installation and updates](getting-started.md#updates-v060-and-later)

## How can I help?

Try the first collaboration and tell us where you got stuck or when you wanted to intervene. Include the Conn version, OS, shell and client, plus a small public example. Review screenshots and request details before sharing; a full terminal dump is rarely needed. [Report a problem](https://github.com/eggplantiny/conn/issues/new/choose) · [Contribute](../CONTRIBUTING.md)
