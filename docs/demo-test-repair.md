# About the Conn demo

English · [한국어](demo-test-repair.ko.md) · [Back to Conn](../README.md)

https://github.com/user-attachments/assets/c20ec712-1915-44c2-a6a2-08b83e6951c4

[Watch in Korean](https://github.com/user-attachments/assets/4f06180c-6a2c-4c10-99e1-7941db86470b)

## One task, two turns

The left side shows an actual Codex CLI session. The right side shows Conn and the shell shared by the agent and the user role. The conversation stays in Codex while both participants work in the same terminal.

1. **The agent takes a turn.** Codex reads a failing discount test from Conn, requests control, fixes the calculation, and verifies that two tests pass.
2. **The user role steps in.** Direct terminal input takes control back and adds a prepared third test: a 125% discount should produce a zero price. Running the tests exposes a result of `-30` instead of `0`.
3. **The agent continues.** The follow-up is: “I added an edge case. Read the terminal and continue.” Codex reads the changed screen and the new test through Conn, updates the calculation to keep prices at zero or above, verifies all three tests pass, and releases control.

The agent reads the new state when asked to continue. It receives the screen through Conn's snapshot tool; the demo does not depend on manually copying terminal output into the conversation.

## How it was made

- The session uses **Codex CLI 0.153.0**, **gpt-6-astra**, and medium reasoning effort, connected to Conn over MCP stdio. The model chooses its commands in response to the actual terminal state; agent commands are not prequeued by a demo driver.
- The scenario uses a disposable demo project, a prepared edge-case test, authored prompts, and separate session data. Browser automation performs the user-role approvals and terminal input. No independent human participant is portrayed.
- A capture-only PTY bridge displays the real Codex CLI beside the unchanged Conn Svelte frontend. Conn uses its native Rust backend on Ubuntu through the local browser test adapter. This split view is a recording arrangement; users run Codex and Conn in their own windows.
- The picture is an actual screen recording. A stable split view keeps the conversation and shared terminal visible. Captions and cuts shorten waiting time; the finished film is an edited account of the session, not a full transcript.
- English and Korean editions use the same session footage with localized editorial text.
- The repository uses a linked poster and MP4 files. These links do not promise an inline GitHub video player; a hosted attachment can replace the link later.

See the [recording setup](../media/demo/RECORDING.md) and [browser test adapter](browser-testing.md). This footage validates the captured collaboration flow on Ubuntu; it is not a native installer test for Linux, macOS, or Windows.

## Reading the interaction

Human terminal input reclaims the lease and stops subsequent agent writes. It does not undo a command or terminate a process already running; normal terminal interrupt controls still apply.

A planned command in a request describes the agent's intent. An `executed` timeline entry records delivery to the shell, not an exit status. In the demo, the actual test output provides the result. The timeline helps review commands and control changes; it does not store terminal output or scrollback.

For the same interaction with your own agent, follow [Getting started](getting-started.md). See the [trust model](security.md) for the scope of policy and audit records.
