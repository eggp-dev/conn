# Policy

`~/.conn/policy.yaml` (or `--policy PATH`). An example file is written on first run.

## Format (v0.3)

There are three kinds of rule.

| Kind | What it matches | Example |
|---|---|---|
| `pattern` | a regex over the text of one segment (a line is split on `&&` `;` `\|`) | `pattern: 'kubectl delete'` |
| `command` (+`args`) | **the command that actually runs**. `\rm`, `/bin/rm`, `command rm`, `r''m`, `sudo rm` and `env X=1 rm` all count as `rm`. `args` is a regex over the joined arguments | `command: rm` / `args: '(^\|\s)-[a-zA-Z]*[rR]'` |
| `protected` | globs over the **target paths** of delete / move / chmod commands, resolved to absolute paths against the shell's cwd | `'~/.ssh/**'` |

And three switches.

| Key | Default | Meaning |
|---|---|---|
| `opaque` | `confirm` | the verdict for constructs whose effect cannot be inspected (`sh -c`, `eval`, `source`, `xargs`, `find -delete/-exec`, `python -c`, `ssh`, `$( )`) |
| `isolate_dangerous` | `true` | when a confirm/deny-level segment shares a line with other segments, deny instead of asking. One approval = one dangerous action |
| `require_intent` | `true` | an agent's ENTER without a one-line intent does not run |

Plus `unattended` (default `copilot`): the highest mode an entrusted agent may use while the human is away from the tab.

### The basic form

```yaml
deny:
  - pattern: 'rm -rf /(\s|$)'
    label: 'delete root'
  - pattern: ':\(\)\{.*\};:'
    label: 'fork bomb'

confirm:
  - command: rm
    args: '(^|\s)-[a-zA-Z]*[rR]'
    label: 'recursive delete'
  - pattern: 'kubectl delete'
    label: 'delete resource'
  - command: git
    args: '^(clean|reset --hard|push .*--force|push .*-f\b)'
    label: 'destructive git'
  - command: sudo
    label: 'privilege escalation'
  - pattern: '(^|[^>&])>[^>&]'
    label: 'overwrite file'
  - pattern: 'DROP TABLE'
    label: 'drop table'

protected:
  - '~/.ssh/**'
  - '~/.conn/**'

default: allow      # allow | confirm | deny
```

- `pattern`: Rust `regex` syntax, `is_match` (partial match) over the segment. Add `^` / `$` yourself when you need anchors.
- `label`: the phrase shown **at the top** of the approval card. Keep it short and concrete so the human can decide in three seconds. `[A] allow for this session` works per label.
- `default`: the verdict for a command no rule matched.

## Evaluation

The line is split into segments, and each segment takes the first match in the order `deny → confirm → (opaque, if opaque) → default`. The line's verdict is the worst of its segments. A target path that matches `protected` is a deny. With `isolate_dangerous` on, a dangerous segment that shares the line with any other segment is denied (label "run dangerous commands alone").

A `deny` cannot be lifted by an approval or by an `[A]` session allow. Session allows work per segment label.

The approval card shows, in this order: the **intent** the agent wrote, the **target paths** the engine computed (existence, git repository, entry count, protected), the per-segment verdicts, and the raw line. What the human has to compare is the intent against the targets.

Policy applies only to **an agent's ENTER**. Human commands are recorded, not checked.

## Keep it narrow

Approvals are read only when they are rare. A system that asks about everything effectively asks about nothing.

- Put only hard-to-undo things under `confirm`: deletes, force pushes, privilege escalation, production changes.
- Do not list read-only commands (`get`, `ls`, `status`, `log`).
- When the same label keeps appearing in repetitive work, allow it for the session with `[A]`. It resets when the session ends.

## Startup

An invalid or unreadable policy prevents a new shell from starting. Conn records `policy_load_failed`; fix the file and retry. A missing file creates the example policy. Existing sessions retain their last good rules if a later reload fails.

## Reload

The proxy checks the file's mtime every second and re-reads it when it changes. On a parse failure (bad YAML, bad regex) it **keeps the previous policy**, prints one warning line to the terminal and writes `policy_reload_failed` to the audit log. It never falls back to having no policy.

## Limits

The structural analysis is deterministic and conservative. It catches or questions the common evasions (`\rm`, `/bin/rm`, `sudo`, `cd .. && rm`, `sh -c`), but it cannot see aliases, commands inside script files, or commands assembled through variable substitution. It guards against mistakes and casual workarounds, not against a malicious agent ([security.md](security.md)).

## Command extraction

The string that is checked is the input line at ENTER. conn tracks the line from the bytes written to the PTY, and after tab completion or history navigation it uses the VT model's cursor row with the prompt removed. The `cmd` in the audit log is this value.
