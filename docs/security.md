# Trust model and data handling

[Reporting a vulnerability](../SECURITY.md) · [한국어 요약](#한국어-요약)

## A cooperative guard

Conn mediates a terminal shared by a human and a trusted agent harness. Its policy engine combines pattern matching and structural analysis of POSIX shell lines. It normalizes some wrappers, resolves visible target paths and asks for review of opaque execution. Dangerous commands mixed with other segments can be denied so that the agent must submit them separately for review.

**This is not isolation.** A process running as your OS user, a malicious agent, shell aliases/functions, variable expansion, nested shells and scripts can bypass assumptions in that analysis. A declared intent and planned command are agent-supplied descriptions, not proof of what a shell will do. Conn does not inspect arbitrary script contents or stop an agent from using another tool outside Conn.

Remote and non-POSIX profiles require per-command review. They do not resolve target paths against the host filesystem, and session-wide allowances do not remove this review requirement. Explicit deny rules remain in force. See [backend boundaries](backends.md#execution-and-policy-boundaries).

For ordinary collaboration sessions, invalid or unreadable policy files prevent new shells from starting and produce a `policy_load_failed` audit event. Correct the file and retry. A failed reload keeps the existing session's last successfully loaded rules; new collaboration tabs remain blocked until the file is fixed. A missing policy file creates the example policy. Private external automation has its own unreleased input and recording contract below.

## Ordinary-session command reconstruction limits

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

## Private external sessions (unreleased)

**This replacement is UNRELEASED, not a protection provided by v0.5.1.** The
[external automation guide](external-automation.md) describes private sessions
created by an authorized native launcher. Their private policy is established
before the child starts. A supplied startup program replaces the local profile
program on its PTY; later external writes use a separate input source and do not
pass through agent registration, proposals, policy approvals or Grace.

Conn creates no activity audit, saved timeline, payload-derived title, recent
automation list or raw human-input history for an external-origin private session.
Request status contains metadata and generic error codes only, never input,
intent, output, cwd or raw errors. `intent` remains syntactically accepted but is
ignored and discarded. Payload buffers are released on delivery, cancellation,
timeout and failure; owner/request metadata is bounded and volatile. Only
permissions survive an app restart. Existing recorded-adapter settings require an
explicit re-enable because the new grant permits direct execution.

Private sessions are excluded from MCP/public local IPC discovery and access,
including explicit IDs, output subscriptions and status/title/cwd routes. Only
their owning native window receives terminal output. Programmatic clipboard
writes from that output are disabled; explicit human copying is still available.
AI sharing of these sessions is not implemented. Ordinary sessions keep the
existing agent policy and history behavior.

Human input, takeover, cancellation, release, configuration revocation or close
revokes the external writer and pending queue. It cannot silently reacquire the
same session. Already delivered bytes cannot be withdrawn; a live terminal remains
available to the human after writer release or launcher exit.

The no-activity-history scope covers Conn's private-session recording paths. It
does **not** promise zero retention: native strings, memory scrollback, PTY/OS
buffers, child output/logs, shell history/tracing, argv, environment, clipboard and
crash dumps remain separate surfaces. Same-user processes may still use other
tools to access the OS; these API gates are not an OS sandbox. There is no password
detector or automatic proof that login has finished.

The general raw human-input tracking problem in ordinary sessions is **not fixed
by this change**. Existing audit files, saved timeline records and backups also
remain; no automatic historical cleanup or credential classification is performed.

## Stored data

Default native state is under `~/.conn` (or the selected test/config directory). It includes policy, audit records, shell profiles and frontend defaults. The webview/browser also stores UI preferences and recent timeline data locally. Profile environment values are plaintext; avoid putting long-lived credentials there.

Ordinary-session audit and saved timeline records, and records from earlier automation releases, can include agent identities, intents, full commands, paths, approval outcomes and **original request parameters**. Commands that write a file can include that file's content. Treat these records as potentially sensitive. There is no automatic secret redaction. Do not upload your `.conn` directory or paste raw request JSON into public issues without reviewing it.

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

### 비공유 외부 세션 변경 — 미배포

아래 계약은 **미배포(UNRELEASED)**이며 v0.5.1의 보호 기능이 아닙니다. 외부 런처가
허용된 프로필로 만드는 세션은 자식 실행 전부터 비공유로 설정하고, 시작 프로그램은
프로필 프로그램을 직접 대체합니다. 외부 입력은 에이전트 등록·정책 승인·제안·Grace를
거치지 않습니다. 기존 자동화 허용은 한 번 직접 다시 켜야 합니다.

- 비공유 외부 세션의 감사 이벤트·저장 타임라인·원문 제목·최근 자동화 활동·원시 사람
  입력 기록을 만들지 않습니다. 상태는 메타데이터와 일반 오류 코드만 포함하며 `intent`는
  받더라도 사용하지 않고 버립니다. 원문 버퍼는 전달·취소·실패·시간 초과 때 해제합니다.
- MCP·공개 로컬 IPC의 목록과 직접 접근, 화면·이벤트·제목·작업 폴더 공개를 차단합니다.
  소유한 네이티브 창에만 출력합니다. 출력에 의한 프로그램적 클립보드 쓰기는 막으며,
  사람이 직접 복사할 수는 있습니다. AI와 공유하는 기능은 아직 없습니다.
- 사람 입력·제어 회수·취소·반환·설정 해제·종료 시 외부 입력 권한과 큐를 해제합니다.
  같은 세션의 권한을 자동 재획득할 수 없고, 이미 전달한 바이트를 되돌릴 수는 없습니다.
- 메모리 스크롤백·네이티브 문자열·PTY/OS 버퍼·자식/셸 기록·argv·환경·클립보드·충돌
  덤프는 별도 경로입니다. 메모리 잔존이 없다는 약속이나 같은 사용자 프로세스의 OS
  격리가 아니며 비밀번호·인증 완료 탐지 기능도 아닙니다.
- 일반 세션의 원시 사람 입력 기록은 이번 변경으로 해결되지 않습니다. 이전 감사 로그·
  타임라인·백업도 자동 삭제하지 않습니다.

[외부 자동화 사용법](external-automation.ko.md)에 전체 계약과 검증 조건을 정리했습니다.

보안 제보 방법은 [보안 정책](../SECURITY.md)을 참고하세요.
