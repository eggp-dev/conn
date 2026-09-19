# Trust model and data handling

[Reporting a vulnerability](../SECURITY.md) · [한국어 요약](#한국어-요약)

**Unreleased hardcut.** This page describes the working tree's new shared-surface
contract. The released v0.6.0 app keeps externally created sessions private for their
whole lifetime and does not provide the sharing transition or native model extensions.

<a id="one-terminal-one-presented-surface"></a>

## One shared terminal session

Observation is the current terminal grid parsed from PTY output in Conn's core.
Window focus, minimization, overlays and tab selection are not access controls.
No renderer heartbeat is required. Agents do not follow the human across tabs;
explicit session participation still applies to every read and write.

The grid contains no raw keyboard input, process memory, environment or scrollback.
No-echo passwords are absent; masked passwords appear as masks. ANSI conceal and
explicit equal foreground/background colors are suppressed in text projection.
An agent cannot submit a cursor line that contains such withheld text, because the
resolved command is echoed back to it. Colors that merely look alike under a theme
(for example black text on a default dark background) are not detected.
A child printing plaintext secrets exposes them to all authorized observers.

This is a terminal-text contract, not pixel equality with the owner's viewport.
Scroll position, themes, graphical terminal protocols and desktop overlays are not
exported. Theme-dependent color coincidences and graphical rendering are not a
security boundary. Do not hide credentials using colors; use no-echo input.
Clearing the screen cannot erase prior snapshots, child history or model copies.
The small pinned terminal-parser patch and its tests are documented in
[the parser patch notes](../vendor/vt100/CONN-PATCH.md).

## Participation, control and the local owner

The local socket is restricted to the same OS user. Within that boundary the desktop app
asks before a new agent connection participates: until the owner presses **Allow**, the
connection learns nothing about any session and is not a sharing candidate. The answer
binds to the live connection, never to its display name, and a denied connection stays
out. This is a consent boundary, not protection against malware already running as the
same user. The prompt can be disabled in settings for fully trusted local setups.

Sharing and input control are separate. Explicit sharing selections use live connection
IDs, not display names: reconnecting or choosing another agent's name does not inherit
that selection. Removing a participant or stopping sharing invalidates queued disclosure
and pending agent work. Ordinary newly opened shared tabs retain their initial
collaboration policy until the owner makes an explicit participant selection.

Only the native owner window may change sharing, decide approvals,
change settings or supply human input. Public IPC accepts agent connections only.
Declaring `kind: human` or `kind: frontend` does not confer owner authority. There is no
public raw-output subscription or frontend identity fallback. Authorized session
observation remains available without an active renderer. See [protocol v2](protocol.md).

Control requests and command approvals remain distinct. Human input revokes an agent's
lease before reaching the PTY. Grace can delay an allowed Enter, and the owner may
cancel or co-sign it. Taking control does not terminate an already running child process.
Stopping sharing blocks further agent access; it cannot recall information already sent.
Owner approval operations now live in the app, not public CLI/socket commands.

## A cooperative guard, not OS isolation

Conn's policy combines pattern matching and structural analysis of submitted shell lines.
Remote, non-POSIX and shared external-origin sessions require command review rather than
resolving their targets against the host filesystem. In an integrated local shell,
an unconfirmed foreground program (including SSH or an editor) also requires review
until the trusted outer shell reports completion. Session allowances do not remove
that review requirement. Explicit deny rules remain in force. See [backend boundaries](backends.md#execution-and-policy-boundaries).

Shell aliases/functions, variables, scripts, nested shells and interactive line editors
can change what a submitted line does. Planned commands and intent are agent-supplied
information, not proof of execution. Conn does not inspect arbitrary scripts or stop an
agent's separate shell/file tool. A logged command is not proof of exactly what a shell
parsed; the macOS 15 / Bash 3.2 Unicode PTY report illustrates that distinction.

Same-user OS processes, administrators and a compromised harness are not isolated by
Conn. An agent controlling an authenticated remote shell can use that account's existing
permissions. Hiding a password does not remove those permissions or prevent the child
from reading files available to the authenticated account.

Invalid/unreadable collaboration policy files block new ordinary shells. Failed reloads
retain the last valid policy. External launch permission remains a separate direct-input
grant; it is not a model approval. Neither grant should be treated as an OS sandbox.

<a id="ordinary-session-command-reconstruction-limits"></a>

## Command recording

Conn does not reconstruct human commands from raw typing, paste, Enter presses or screen
text. Supported local Bash/Zsh integration reports command start/completion separately.
Authentication prompts and editor input do not become human command history. After
a sharing boundary, recording waits for a trusted shared prompt and new shared input;
delayed private execution events cannot become shared history. A
content-free unfinished-input flag prevents an agent from appending to a human's line.

Explicit agent requests, intents and submitted commands can be kept in review UI, audit
and timeline. Clean submitted text is tracked independently of visual wrapping. Dirty
agent input may still need the internal policy tracker's current line; wrapped editing
can make this incomplete. These records are not forensic proof or continuous output.
Do not embed credentials in commands or original request parameters.

<a id="private-external-sessions-unreleased"></a>

## Private external sessions

An authorized external launcher creates a private session before starting the child.
Its startup program replaces the profile program on that PTY. Later external writes use
the owner-bound automation service without pretending to be an AI agent. Status is
metadata-only; input, output, intent, cwd and raw errors are excluded. The optional legacy
`intent` field is ignored. Payload buffers and request metadata are bounded and volatile.

While private, the session is absent from public discovery and inaccessible by explicit
ID. Only its owner window receives output. Conn creates no private activity audit,
saved timeline, payload-derived title, recent automation activity or raw human-input
history. Programmatic clipboard writes from private output are disabled; explicit
human copying remains possible.

The new owner action can share **the same PTY and authenticated connection**. It first
revokes the external writer and queued input, cancels pending agent work, resets the
observation boundary, and starts from human control. The next frame is the existing
human-visible viewport: there is no automatic sanitization or alternate agent screen.
It does not infer authentication success. An unfinished tracked input line refuses the
transition; complete or cancel that input first.

Collaboration activity can be recorded from sharing onward. Private inputs, startup
arguments and old output are never reconstructed into that history. External sessions
never install new shell hooks during sharing, so later human commands in such a session
are not fabricated from typing. Stopping sharing disables further activity recording
apart from the stop transition itself. Existing saved shared records remain.

Human takeover, cancel, release, permission/profile removal and closing also revoke the
external writer. Its old handles cannot regain authority or silently follow the user
into another tab. Already delivered input cannot be undone. See [external automation](external-automation.md).

## Native extensions and model data

The [extension host](extensions.md) accepts declarative terminal themes and reviewed
built-in native execution. It is not a general third-party code sandbox or marketplace.
Extensions cannot access a PTY, grant control, publish their own terminal frame or bypass
sharing. Conn renders their settings and proposals with its common UI.

The optional OpenAI suggestion feature is off by default. The user saves their own key,
selects a model and enables suggestions. An automatic request after an 800 ms typing
pause requires a confirmed idle local shell prompt under human control; a content-free
signal triggers it. Manual invocation can also be used in an unconfirmed environment.
Only the current authorized visible text is sent; no files, history, hidden input or
previous model conversation is added.
The suggestion is a single-line suffix; accepting it inserts text through the common
human-input path and never sends Enter. A changed frame invalidates the suggestion.

Keys live in macOS Keychain, Windows Credential Manager or Linux Secret Service. A
locked/unavailable store fails without plaintext fallback. Keys are not written to profile
environment, extension JSON, activity logs or proposal payloads. The reviewed native
provider sends requests to a fixed HTTPS OpenAI endpoint with redirect/proxy overrides
disabled, bounded input/output, cancellation, timeouts and request limits. Provider error
bodies are not shown. `store: false` does not promise zero provider retention or free
processing after a local cancellation. External agent clients have their own model and
data policies. Conn still has no telemetry client.

## Stored data and local endpoints

Default native state is under `~/.conn` or the selected config directory. Policy, shell
profiles, extension settings and audit records live there; the webview saves UI preferences
and recent timeline records locally. Profile environment values are plaintext, so do not
use profiles as a credential store. Stored command arguments and original requests may
contain sensitive file contents. There is no automatic historical secret redaction.

| Surface | Boundary |
| --- | --- |
| Unix IPC | Local socket with owner-only permissions |
| Windows IPC | Local owner-only named pipe; remote clients rejected |
| Native owner | Window-bound Tauri command/event bridge |
| Browser test adapter | Loopback HTTP/WebSocket, exact Origin and token checks; runs real shells |
| MCP | Local `conn mcp` agent transport; owner operations unavailable |
| Native OpenAI provider | Opt-in HTTPS model request, current visible context only |

Do not expose the browser adapter through a public reverse proxy or upload `.conn` as a
diagnostic bundle. Local `conn log` reads a file with the caller's OS permissions; it is
not an extra MCP history permission. Existing logs/backups are not erased by sharing changes.

<a id="secret-exposure-scenarios-unreleased-hardening"></a>

## Secret exposure scenarios

| Scenario | Expected result and boundary |
| --- | --- |
| Hidden or masked authentication | No hidden input is reconstructed; only rendered masks may be observed when shared. |
| Visible child output | Shared snapshots and enabled native suggestions may transmit it. |
| Sharing stops during a response | Session/participation/surface checks discard obsolete queued disclosure; delivered copies remain. |
| Old external input handle | No input after takeover/sharing/revocation; no silent reacquisition. |
| Agent command or original request | Review/history may contain it; no automatic redaction. |
| Startup argv, environment, child logs, tmux | Outside Conn's input-recording protection; can retain or reprint values. |
| Keychain missing or locked | Key save/read fails; no config/environment fallback. |
| Clipboard, screenshot, crash dump, old backup | Separate human/OS exposure routes; not automatically cleared. |

The hardcut needs synthetic hidden/masked/visible input tests, frame/scroll/conceal
checks, revocation races, owner-spoofing tests and same-SSH-session handoff tests. Unit
checks and browser adapters do not prove native macOS/Windows rendering, system unlock
prompts or every external launcher. Live OpenAI calls require a user-configured smoke test.

<a id="shell-command-integration-unreleased"></a>

### Shell command integration

Local Bash/Zsh use a bounded per-session mailbox with owner-only Unix permissions.
The initial shell PID and sequence are checked, but same-user processes are still trusted.
Terminal escape sequences cannot create these events. Missing/conflicting hooks, history
suppression or mailbox overflow never enable raw-input reconstruction. A crash can leave
temporary command data. See [English](shell-integration.md) and [Korean](shell-integration.ko.md).

### Linux external automation

The D-Bus adapter is off by default and needs executable and profile permission. Bus
UID/PID, executable and process-start checks identify the caller; one unique connection
owns its handles. Interpreter permission covers scripts that interpreter runs. This is
not code signing or protection against privileged bus monitoring. Previous acceptance
used an isolated session bus/X11 display; Wayland-specific behavior remains separate.

## Lifetime

Sharing changes preserve the process. Closing the desktop session or reloading/disconnecting
the browser test client ends its shells. There is no detach/reattach or process restoration
from saved timeline data. Back up work as files/version control.

## 한국어 요약

**개발 중인 공유 셸 화면 계약입니다. 배포판의 동작과 구분합니다.**

- 사람과 에이전트가 같은 표시 화면을 사용합니다. 에이전트에게 원시 PTY 출력이나
  원시 입력·스크롤백을 제공하지 않습니다. 창 가림·최소화·다른 탭 선택은 공유 권한을 바꾸지 않습니다.
- 숨김 입력은 복원하지 않고 별표는 별표로 보냅니다. 이미 보이는 민감정보나 나중에
  자식이 다시 출력한 정보는 공유될 수 있습니다. 픽셀 스크린샷은 제공하지 않으며 색상이나 테마는 비밀을 숨기는 수단이 아닙니다.
- 공유 대상은 표시 이름이 아니라 실제 연결 ID입니다. 공개 소켓에서 인간·프런트엔드
  역할을 자칭해도 승인·공유·화면 게시·직접 입력 권한을 얻지 못합니다.
- 비공유 인증 후 같은 PTY·SSH 연결에서 공유할 수 있습니다. 외부 입력 권한과 큐를
  먼저 해제하고 현재 화면부터 공유하며, 과거 비공유 입력은 기록으로 복구하지 않습니다.
- 사람의 원시 키 입력은 기록하지 않습니다. 지원하는 로컬 셸의 실행 훅만 사람 명령을
  기록합니다. 외부 세션에는 공유 전환 시 훅을 새로 설치하지 않습니다.
- 내장 명령 제안은 기본 꺼짐입니다. 사용자 키는 OS 보안 저장소에만 저장합니다. 켜면
  확인된 로컬 프롬프트의 입력 멈춤 또는 수동 요청 때 현재 공유 화면 텍스트를 OpenAI에
  보냅니다. 인증·편집기 입력은 자동 호출하지 않으며 수락은 입력만 하고 실행하지 않습니다.
- 정책·공유 제한은 OS 격리가 아닙니다. 같은 계정의 다른 도구, 로그인된 원격 계정의
  권한, 자식 기록·argv·환경·과거 스냅샷까지 제거하지 않습니다.
- 네이티브 화면·키체인·실제 모델 호출과 외부 런처는 각 플랫폼에서 별도 검증해야 합니다.

<a id="비공유-외부-세션-변경--미배포"></a>

[외부 자동화 사용법](external-automation.ko.md) · [확장 기능](extensions.ko.md) · [보안 제보](../SECURITY.md)
