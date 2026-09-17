# Your first collaboration

English · [한국어](first-collaboration.ko.md) · [Back to Conn](../README.md)

**Change the destination yourself. Ask your agent to continue from there.**

This short exercise uses two temporary folders and one text file. Keep the conversation in your agent client and use Conn for the shared terminal.

## Before you start

Open Conn and [connect your agent](agent-integrations.md). In Conn's top-right controls, select **Autopilot** and enable **Ask before granting**. Keep this tab visible during the exercise.

Use a local Bash or Zsh profile on macOS/Linux, or a PowerShell profile on Windows. Commands for the two shell types are separate below.

## 1. Prepare two empty folders

Paste the matching block **into Conn**. It creates a fresh temporary folder each time, so you can try again without deleting anything.

<details open>
<summary>macOS / Linux · Bash or Zsh</summary>

```sh
conn_demo_dir=$(mktemp -d "${TMPDIR:-/tmp}/conn-demo.XXXXXX") &&
mkdir "$conn_demo_dir/draft" "$conn_demo_dir/workspace" &&
cd "$conn_demo_dir/draft" &&
pwd
```

</details>

<details>
<summary>Windows · PowerShell</summary>

```powershell
$connDemoDir = Join-Path ([System.IO.Path]::GetTempPath()) ('conn-demo-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $connDemoDir -ErrorAction Stop | Out-Null
New-Item -ItemType Directory -Path (Join-Path $connDemoDir 'draft'), (Join-Path $connDemoDir 'workspace') -ErrorAction Stop | Out-Null
Set-Location -LiteralPath (Join-Path $connDemoDir 'draft')
Get-Location
```

</details>

The printed path should end in `draft`.

## 2. Let your agent prepare

Send this in **your agent's chat**:

> Use Conn for this task. Read the current screen, request control with the exact command, and run `pwd` to check the current directory. Keep control and wait for my next message; do not create or change files yet.

Review and allow the request in Conn. If a separate command approval appears, review that too. Wait for the `draft` path and an empty shell prompt.

## 3. Make the correction yourself

In **Conn**, type:

```sh
cd ../workspace
pwd
```

These commands work in both shell types above. Your input takes control back. The printed path now ends in `workspace`.

If control expired before you typed, the exercise still works. To see the handover itself, repeat the read-only request when you are ready to type.

## 4. Continue from the changed screen

Send this in **your agent's chat**:

> I changed the directory in Conn. Read the current screen again, then verify the current directory. Without changing directories, create `collaboration.txt` here with the single line `We continued from your correction.` Do not overwrite an existing file. Read it back and confirm that `../draft/collaboration.txt` does not exist. Use Conn for every shell action, request control with the exact planned command, and release control when finished. Stop if I deny a request or take control again.

Review the new request and any command approvals. The agent should use the directory you selected, without returning to its earlier destination.

## What success looks like

- `workspace/collaboration.txt` contains `We continued from your correction.`
- `draft/collaboration.txt` does not exist.
- The agent checked the changed terminal before continuing and gave control back afterward.

Open **Timeline** to review the commands and handovers. Taking control prevents further agent input; it does not stop a process already running.

That's the workflow: **the conversation stays with your agent; the terminal stays shared.** Try the same handover when choosing a project folder, checking a result, or changing a command before execution.

[Connection help](agent-integrations.md) · [Install Conn](getting-started.md) · [Example and verification notes](../examples/first-collaboration/README.md)
