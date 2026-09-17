# I changed the directory. Continue from here.

English · [한국어](collaboration-story.ko.md) · [Watch the recreation](demo.md) · [Try it yourself](first-collaboration.md)

On September 16, we were using a local agent to prepare a project on a remote Mac through a shared SSH terminal. The agent had typed a clone command with a destination the user did not want. Before it ran, the user took the keyboard and corrected the location.

That direct input returned terminal control to the user. The agent's attempted Enter was rejected. The user completed the clone in the preferred location and moved into it. Then the agent read the changed terminal and resumed from there.

The user had changed the working state directly, and the agent had to observe that state before continuing. There was no need to reconstruct the whole shell session in a chat message.

## The conversation and the work stayed connected

The same session also involved finding the remote machine, connecting over SSH, and letting the user handle authentication. Later, a remote Codex command returned a natural-language description of the project. Those were useful steps, but the path correction made the collaboration especially clear: the user could participate in the task itself, not only approve or comment on it.

Conn focuses on making that handoff understandable with your existing agent. This single session does not establish an advantage over other terminals.

The remote experiment also had limits. A later attempt to send input into the interactive remote Codex interface was blocked by `input_pending`. That retry did not succeed. The local recreation below does not establish that the remote issue is fixed.

## A smaller recreation you can try

For the new demonstration, we removed SSH setup and private machine details. The prepared workspace contains only `draft` and `workspace` directories.

An actual Codex CLI session connects to Conn, reads the terminal and runs `pwd` in `draft`. It keeps control at an empty prompt. The user role types `cd ../workspace` and `pwd`, taking control and changing the location.

Asked to continue, Codex takes a fresh snapshot, checks the directory again and creates `collaboration.txt` in `workspace`. The resulting file contains exactly:

```text
We continued from your correction.
```

No corresponding file exists in `draft`. The outcome depends on reading the changed state, rather than continuing from the earlier destination.

The checkpoint was prepared; it does not show a spontaneous model mistake. User-role actions were automated, with no independent participant. The footage uses actual Codex CLI and released Conn source, with the native backend through the browser test adapter. Waits are edited. [Recording notes](demo.md) describe the environment; the clip is not a native installer test or verification that remote interactive input is fixed.

## The next useful correction

A directory is a small example. The same interaction can help when choosing a project, adding a missing test or completing a step yourself. Take control, make the change, then ask the agent to read the terminal before it continues.

[Try the local first collaboration](first-collaboration.md). Afterward, tell us where you wanted to step in and whether the agent understood your change.

## The recorded screens

These frames come from the same actual capture. The human role is automated; the side-by-side layout is recording tooling.

![The human role has changed the working directory directly](assets/conn-collaboration-correction.webp)

![The file is verified in the new directory and absent from the original](assets/conn-collaboration-result.webp)

[Timeline from the same session](assets/conn-collaboration-timeline.webp)
