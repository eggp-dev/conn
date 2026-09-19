# Shell command history

[한국어](shell-integration.ko.md) · Available since v0.6.0

Ordinary local Bash and Zsh sessions can put human commands back in the shared
timeline. Commands are observed at the shell's execution boundary. Conn does not
reconstruct them from keyboard input, Enter presses, terminal text or prompt patterns.

## Use

Select a local Bash or Zsh profile in Settings → Profiles and open a new tab.
Run a command, then open Timeline to see its entry. Existing sessions need a new
tab; user startup files are not edited.

One entry moves from **Running** to **Completed**, **Failed**, or **Completion
unconfirmed**. Expand it for the working directory, exit code and elapsed time.
The exit code describes the outer shell command/list, not every pipeline process
or a background job's eventual result. An agent command is joined to its existing
request/approval entry by ID. Uncertain attribution is labelled **Shell**.

| Action | Timeline | Agent screen/control |
|---|---|---|
| Run a command at the integrated local prompt | One lifecycle entry | Existing mode and permissions |
| Type a password into sudo/SSH or a program | No input entry | Existing screen permissions; visible output is still visible |
| Open Vim, a REPL, nested shell, or tmux | Outer command only | Existing mode and permissions |
| Connect with `ssh host` | The local SSH command, ending when it returns | Continues inside SSH; no remote helper is installed |
| Use an external private session | No history and no hooks | Unavailable while private. The owner sharing action (v0.7.0 and later) enables collaboration in the same session; human takeover alone does not |

## Startup compatibility

- Bash: an initial `PROMPT_COMMAND` bootstraps hooks after normal startup. Existing
  prompt commands and their exit status are preserved. An existing DEBUG trap,
  function tracing or extended debugging prevents installation. Startup files
  that overwrite `PROMPT_COMMAND` can prevent the bootstrap and leave command
  recording unavailable. Bash uses fresh history entries, so disabled history,
  history exclusions and suppressed duplicates can leave commands unrecorded.
- Zsh: a temporary `.zshenv` restores the original `ZDOTDIR`, sources the original
  `.zshenv`, and arranges first-prompt installation of `preexec`/`precmd` hooks.
  Other startup files and existing hook arrays remain in use. Startup scripts
  that replace these hooks can prevent recording.
- Initial support is for interactive local profiles with default/login/interactive
  flags. Custom startup commands, remote backend profiles, other shells and Windows
  report unavailable. There is no fallback to keystroke capture.

Only the initial shell is integrated. Installing remote hooks, broadening shell
support, and detecting/redacting secrets embedded in command arguments are separate
work. An integration does not certify command arguments as safe to share.

## Delivery and verification

Lifecycle events use a private, bounded temporary mailbox; no helper executable or
network service is needed. There are at most 64 queued records, each bounded to
64 KiB. Overflow or invalid framing stops recording for that tab; open a new tab
to reconnect. Normal shutdown removes the mailbox. A process crash may leave
temporary command data, subject to the same-user boundary in [security.md](security.md).

`crates/core/tests/shell_integration.rs` exercises real Bash/Zsh PTYs, authentication
input, nested shells, private sessions, existing prompt/debug hooks, suppressed
history, agent correlation and unconfirmed completion. Frontend tests cover live
and saved lifecycle records. Zsh can be selected with `CONN_TEST_ZSH`.
The separate authentication fixture project covers real SSH password/retry/key
authentication, hidden/masked/visible prompts and Vim. These Linux checks do not
establish native macOS shell-hook behavior; platform-specific hands-on verification
remains necessary. See the [v0.6.0 validation limits](releases/v0.6.0-validation.md#coverage-limits).
