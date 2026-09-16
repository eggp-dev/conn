# Trust model and data handling

[Reporting a vulnerability](../SECURITY.md) · [한국어 요약](#한국어-요약)

## A cooperative guard

Conn mediates a terminal shared by a human and a trusted agent harness. Its policy engine combines pattern matching and structural analysis of POSIX shell lines. It normalizes some wrappers, resolves visible target paths and asks for review of opaque execution. Dangerous commands mixed with other segments can be denied so that the agent must submit them separately for review.

**This is not isolation.** A process running as your OS user, a malicious agent, shell aliases/functions, variable expansion, nested shells and scripts can bypass assumptions in that analysis. A declared intent and planned command are agent-supplied descriptions, not proof of what a shell will do. Conn does not inspect arbitrary script contents or stop an agent from using another tool outside Conn.

Remote and non-POSIX profiles require per-command review. They do not resolve target paths against the host filesystem, and session-wide allowances do not remove this review requirement. Explicit deny rules remain in force. See [backend boundaries](backends.md#execution-and-policy-boundaries).

Invalid or unreadable policy files prevent new shells from starting and produce a `policy_load_failed` audit event. Correct the file and retry. A failed reload keeps the existing session's last successfully loaded rules; new tabs remain blocked until the file is fixed. A missing policy file creates the example policy.

## Command reconstruction limits

Clean input is tracked in full, including Unicode and lines that wrap visually. Policy checks and the command audit use that complete tracked text. After completion, history navigation or cursor editing marks input dirty, Conn falls back to the VT cursor row with the prompt prefix removed. **Wrapped or complex edited input can therefore be incomplete in policy evaluation and command history.** Use explicit full commands through the agent API and inspect the terminal before approving sensitive work. Original request details preserve what was submitted to Conn, but they are not a full terminal recording.

A complete submitted command is not proof of the bytes a shell eventually parses. Shell line editors, key bindings and locale settings can transform terminal input before parsing; the macOS 15 / Bash 3.2 Unicode PTY report is an example of this distinction. Conn records submitted text and input delivery, not a shell parser acknowledgement. For sensitive operations, inspect the resulting screen and filesystem state as well as the approval record.

Alternate-screen input (for example inside an editor) is not treated as shell command history. Shell syntax, prompts and interactive applications differ; reconstructed command records should not be treated as forensic proof.

## Approval and takeover

Control requests and command approvals are separate. Granting control does not waive policy. Observe, Co-pilot and Autopilot change how an approved agent participates; they do not create OS isolation. Human input revokes the agent lease before reaching the terminal. Grace lets the user cancel or run an allowed command sooner.

`conn approve` requires a specific approval ID. Automation that approves arbitrary pending requests defeats human review and must not be configured for real user sessions. Isolated tests may explicitly approve their own known requests to verify engine transitions; they should use temporary state and disposable shells.

## Local endpoints and network

| Surface | Boundary |
|---|---|
| Unix agent IPC | Local Unix domain socket, mode 0600 |
| Windows agent IPC | Local named pipe, owner-only DACL, remote clients rejected |
| Tauri desktop | Native command/event bridge to the shared harness |
| Browser development harness | HTTP frontend on 127.0.0.1:1421 and WebSocket backend on 127.0.0.1:1423; token and exact Origin validation; one browser client |
| MCP | The local `conn mcp` process returns responses to the agent client |

Same-user processes are within the trust boundary. The browser adapter is a development tool that starts real native shells; do not expose it through a public reverse proxy. SSH and Docker profiles launch the configured clients, which may contact remote systems. Conn contains no model provider integration or telemetry client. An agent client can send terminal snapshots and tool responses to its own model provider under its own data policy.

## Stored data

Default native state is under `~/.conn` (or the selected test/config directory). It includes policy, audit records, shell profiles and frontend defaults. The webview/browser also stores UI preferences and recent timeline data locally. Profile environment values are plaintext; avoid putting long-lived credentials there.

Audit and saved timeline records can include agent identities, intents, full commands, paths, approval outcomes and **original request parameters**. Commands that write a file can include that file's content. Treat these records as potentially sensitive. There is no automatic secret redaction. Do not upload your `.conn` directory or paste raw request JSON into public issues without reviewing it.

The audit is not continuous terminal output or scrollback recording. A terminal snapshot can still expose secrets currently visible on screen. Only actions passing through Conn are covered; an agent's separate shell tool is outside this history. This preview does not offer tamper-proof logs or complete execution provenance.

## Lifetime

There is no detach/reattach. Closing the desktop session or disconnecting/reloading the browser harness ends its native shell sessions and control leases. Saved timeline records do not restore running processes. Back up work through normal files/version control, not the timeline.

## 한국어 요약

Conn은 신뢰하는 사람과 에이전트가 터미널을 공유하도록 돕습니다. 정책 검사는 실수 방지 장치이며, 악의적인 에이전트나 같은 OS 사용자 권한의 프로세스를 격리하는 보안 경계가 아닙니다.

- 직접 입력한 전체 명령은 화면 줄바꿈과 무관하게 검사·기록합니다. 자동 완성, 히스토리, 커서 편집 후에는 화면의 현재 행을 복원에 사용하므로 긴 편집 명령이 불완전하게 검사·기록될 수 있습니다.
- 전체 명령을 기록했더라도 셸이 같은 바이트를 해석했다는 증거는 아닙니다. 셸의 줄 편집기·키 바인딩·로케일이 입력을 변환할 수 있습니다. macOS 15 / Bash 3.2의 Unicode PTY 제보도 이 차이를 보여 줍니다. 민감한 작업은 승인 기록과 함께 실제 화면·파일 결과를 확인하세요.
- 잘못되었거나 읽을 수 없는 정책 파일이 있으면 새 셸을 시작하지 않습니다. 파일을 고치고 다시 시도하세요. 실행 중인 세션의 정책 재로딩이 실패하면 마지막으로 정상 로드한 규칙을 유지합니다.
- 원격 및 비 POSIX 프로필은 명령별 검토를 요구합니다. 제어권 승인과 명령 실행 승인은 별개입니다.
- 원문 요청, 명령, 경로, 의도와 승인 결과가 감사 로그·타임라인에 저장될 수 있습니다. 파일을 쓰는 명령에는 파일 내용도 포함될 수 있습니다. 자동 비밀정보 삭제 기능은 없습니다.
- 프로필 환경 변수는 평문으로 저장합니다. 브라우저 테스트 어댑터는 실제 셸을 실행하므로 외부에 공개하지 마세요.
- Conn 자체에는 모델 제공자 연결이나 텔레메트리가 없습니다. 연결한 에이전트 클라이언트는 화면과 도구 응답을 자신의 모델 제공자에 전송할 수 있습니다.
- 새로고침·연결 종료 시 테스트 셸이 종료됩니다. 저장된 타임라인은 프로세스를 복구하지 않습니다.

보안 제보 방법은 [보안 정책](../SECURITY.md)을 참고하세요.
