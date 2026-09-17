# A correction in the shared terminal

[Try it in English](../../docs/first-collaboration.md) · [한국어로 해보기](../../docs/first-collaboration.ko.md)

The walkthrough is self-contained: users paste a short setup block into Conn,
so they do not need to clone this repository or install a sample project.

```text
<fresh temporary directory>/
├── draft/                    ← agent checks the original location
└── workspace/                ← human changes to this directory
    └── collaboration.txt     ← agent writes only after observing the change
```

The result is one line: `We continued from your correction.`

## Recording contract

1. Start with two empty directories and a visible shell prompt in `draft`.
2. The real agent reads Conn, requests control, runs `pwd`, and waits with an
   empty input line. No file has been created.
3. Human input changes the directory with `cd ../workspace`, then `pwd`.
4. The agent reads a new snapshot and verifies the working directory before
   creating the file, without changing directories or overwriting a file.
5. The agent reads the file, checks the original destination remains empty,
   and releases control.

Do not substitute a prerecorded agent answer or a reconstructed terminal for
the actual run. If human-role input is automated, disclose that in the recording
notes. The exercise deliberately uses a preparation checkpoint; it does not
claim that the model spontaneously chose a wrong directory.

## Safe reference commands

The agent should choose commands for the active shell. These are reference
commands for the one-file write and result check, submitted separately through
Conn's normal control and command review flow.

### Bash / Zsh

Exclusive creation fails if `collaboration.txt` already exists:

```sh
(set -C; printf '%s\n' 'We continued from your correction.' > collaboration.txt)
```

Read the result and check the original destination:

```sh
cat collaboration.txt
test ! -e ../draft/collaboration.txt && printf '%s\n' 'Original destination is empty.'
```

### PowerShell

`New-Item` without `-Force` refuses to replace an existing file. See Microsoft's
[New-Item reference](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.management/new-item)
for file creation and the overwrite option.

```powershell
New-Item -ItemType File -Path collaboration.txt -Value "We continued from your correction.`n" -ErrorAction Stop | Out-Null
```

```powershell
Get-Content -LiteralPath collaboration.txt
if (Test-Path -LiteralPath ../draft/collaboration.txt) { throw 'A file was created in the original destination.' } else { 'Original destination is empty.' }
```

## Fixture verification

From the repository root:

```sh
sh examples/first-collaboration/verify.sh
```

The check creates its own temporary directory, exercises the correction and
write commands, verifies the exact result and original destination, and attempts
a second write to verify that the original file is preserved. It prints the
temporary path and leaves it available for inspection.

This is a shell-fixture check, not a substitute for observing the agent's Conn
tool calls and the human handover. The POSIX fixture was checked on Linux with
Bash and Dash. Native macOS/Zsh and Windows/PowerShell walkthroughs still need
platform-specific hands-on verification.
