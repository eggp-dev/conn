# 0.8.7 macOS Vim input investigation

[한국어](vim-input-investigation.ko.md) · [Run the test](browser-testing.md#actual-vim-editing-regression)

2026-09-23. **Unresolved: the reported native macOS failure has not been reproduced.** This report is not evidence of a fix or a reason to publish another release.

## Report and baseline

The user could edit files in vi before the update, but reports no response to Escape → i → English text in the latest Conn. It is not yet established whether ordinary shell input also stops, output alone freezes, or keyboard focus is lost. No user files, authentication data or live remote sessions were inspected or modified.

Code review compared `v0.8.6` and `v0.8.7`. Local tests used checkout `05b6214`, included in v0.8.7. No speculative product-code change was made.

## Source findings

- `Session::human_input` forwards human bytes to the PTY. The new `HumanInput.pending` flag is not a direct gate on human typing.
- The common Term component drops input when `renderReady` or `backendOnline` is false. Readiness, native output subscription and checkpoint delivery need further investigation; the existence of this gate does not establish causation.
- Input, terminal replies and resize operations share a session FIFO. On the affected Mac, distinguish key event generation, queue submission, native call completion and PTY output progress.
- Previous input/rendering tests did not execute, edit and save with real Vim. Long shell input and parser tests did not establish editor compatibility.

## Executed tests

Tests used the production UI, real Rust web host, actual PTYs and real vi. Files/settings were disposable; SSH used disposable loopback OpenSSH. Linux WebKit is not native macOS WebKit evidence.

| Host / browser | Shell | Editing scenarios |
| --- | --- | --- |
| Linux / Chromium | Local Bash | 5 passed |
| Linux / WebKit | Local Bash | 5 passed; 1 ResizeObserver warning |
| Linux / Chromium | Real loopback SSH | 5 passed |
| Linux / WebKit | Real loopback SSH entered through a compound command | 5 passed; 1 ResizeObserver warning |

Each run covers private editing, reattachment during editing, shared human takeover after control approval, shared reattachment, and last-row editing in a 100-line buffer after resizing. Assertions include normal/insert mode transitions, cursor editing, saving and saved content. The test does not forcibly restore focus after launching Vim.

`ResizeObserver loop completed with undelivered notifications.` initially failed strict runs, although their editing/saving checks passed. The final test counts and reports this previously observed warning separately in `runtime.json`; other browser errors still fail. Neither harmlessness nor a causal link to the incident is established.

## Remaining verification

1. Establish whether ordinary shell input or only Vim stops in the affected native app.
2. Inspect content-free focus/readiness, input completion and output progress. Do not collect raw keys, editor buffers or `.env` contents.
3. Compare the previous release on the same Mac/SSH path and fix the reproduction conditions without closing live work or discarding unsaved files.
4. Turn the reproduction into a failing regression test, fix it and verify natively. The Linux results do not substitute for native Mac acceptance or a release decision.
