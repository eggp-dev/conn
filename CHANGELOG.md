# Changelog

## 0.6.0 — Preview · 2026-09-17

- Add signed in-app updates with background checks/downloads, preview channels, progress and user-controlled installation/restart. Older versions require one manual install; Debian packages remain system-managed/manual.
- Remove the macOS DMG license agreement while retaining bundled MIT notices and Developer ID notarization.
- Keep timeline activity scoped to its shell and preserve readable markers, 24px click targets and horizontal navigation.
- Unify dock motion, menus and settings surfaces. Add compact agent setup, advanced disclosures, numeric pacing, unsaved-change protection and concise diagnostics.

한국어: 앱 내 서명 업데이트, 동의 화면 없는 macOS 설치, 셸별 타임라인과 최소 클릭 영역, 일관된 모션 및 간결한 설정을 추가했습니다. 설치·재시작은 사용자가 선택합니다.


- Restore human command history through local Bash/Zsh execution hooks, without collecting application input or changing user dotfiles. SSH and editor sessions remain usable; only the outer local command is recorded.
- Join shell start/completion to agent review records by submission ID; show working directory, exit code, duration, and unconfirmed completion. Keep private external sessions excluded and show unavailable integration explicitly.

- Scope native frontend event subscriptions to their owning window, preventing other windows from receiving private tab events.
- Linux: add a default-off D-Bus external automation adapter with explicit caller executable permissions, shared private-session handling, and native-window acceptance coverage.

- Stop reconstructing ordinary human commands from raw keystrokes. Password, paste and editor input no longer create command history or payload-bearing traces; unfinished human input blocks agent appends until completed or interrupted.
- Restrict Unix audit files to owner-only permissions and reject symlink destinations. Add English/Korean notices explaining agent screen access and retained request details.


- Replace recorded, agent-based external automation with private, owner-scoped native sessions. A startup command launches the supplied program directly on the PTY and replaces the allowed local profile's executable and arguments.
- Route external writes through a dedicated input source, without agent control approval, policy review, proposals or Grace. Ordinary AI-agent collaboration keeps its existing rules.
- Exclude private external activity, raw human input and payload-derived titles from Conn recording; block MCP/public IPC discovery and access. No private-to-agent sharing is implemented.
- Keep request status metadata-only with generic errors. Accept and discard the legacy `intent` parameter; revoke external writes on human input, cancellation, disable or profile removal.
- Require one explicit re-enable of previous automation permissions; keep the allowed profile selection. Earlier audit/timeline files and backups are not automatically removed.
- Rename launcher examples generically and update the scripting dictionary and English/Korean guides. Native Apple Event and external-launcher acceptance testing remains required before release.

한국어: 외부 자동화를 비공유 세션과 전용 입력 경로로 교체하는 변경입니다. 시작 프로그램은 기본 셸에 입력하지 않고 직접 실행하며, 에이전트 승인·정책·Grace를 거치지 않습니다. 비공유 활동과 원시 사람 입력은 Conn 기록에 남기지 않고 MCP·공개 IPC 접근을 차단합니다. 이전 허용은 한 번 다시 켜야 합니다. 일반 세션의 원시 사람 입력 기록도 제거했습니다. AI 공유는 아직 구현하지 않았으며, 과거 기록도 자동 삭제하지 않습니다. 실제 Mac 검증은 배포 전 별도로 필요합니다.

## 0.5.1 — Preview · 2026-09-16

- Support `create window with default profile command` for external-application AppleScript launch templates, with real native windows, per-window sessions/output, the existing approval/policy path, and automatic control return after input delivery.
- Compile both session-based and window-launch external automation examples in macOS CI. Native interactive verification remains pending.

한국어: 외부 앱의 새 창 생성 문법을 추가했습니다. 창별 탭·출력을 분리하고 시작 명령은 기존 승인·정책을 거쳐 전달한 뒤 제어권을 반환합니다. 실제 Mac 대화형 검증은 남아 있습니다.

## 0.5.0 — Preview · 2026-09-16

- Prevent mode changes from stranding shell input; restore real Co-pilot interrupts and withdraw entrusted control when the human returns.
- Enforce reviewed-shell approval limits in the core, including CLI requests.
- Expose configured/effective modes to MCP clients and distinguish command approval, proposal acceptance and grace co-signing in the timeline.
- Show connection counts and idle activity separately from local integration configuration.
- Explicitly release integration setup locks even when a concurrently launched shell inherits a file descriptor.
- Add a shared external automation service, macOS AppleScript dictionary, session-scoped input requests, Automation settings, and an example external launcher. Native Mac/external-launcher acceptance testing remains unverified in this preview.
- Correct locale/readline-sensitive PTY fixtures and symlinked temporary-path release tests.
- Limit future macOS builds to Apple Silicon; keep existing Intel release assets available.

한국어: 입력·제어권 관련 버그와 타임라인·연결 진단을 수정했습니다. 외부 앱용 AppleScript 연동과 자동화 설정을 추가했으며 실제 Mac 외부 앱 연동은 이 프리뷰에서 아직 검증하지 못했습니다. 향후 Mac 배포는 Apple Silicon을 대상으로 합니다.

## 0.4.1 — Preview · 2026-09-16

- Add Check for Updates to the app menu, with preview selection, release notes, platform downloads, retry and English/Korean UI.
- Query public GitHub releases only on request; preserve the running shell and open downloads in the system browser. Installation remains manual.

한국어: 앱 메뉴에 업데이트 확인을 추가했습니다. 프리뷰 포함 여부와 변경 사항을 확인하고 운영체제에 맞는 파일을 받을 수 있습니다. 설치는 직접 진행합니다.

## 0.4.0 — Preview · 2026-09-16

- Set up MCP and the bundled collaboration skill from Settings for Codex / local ChatGPT tasks, Claude Code, Cursor, GitHub Copilot in VS Code and Copilot CLI.
- Update or remove managed setups while preserving other settings, comments and user-edited entries.
- Separate saved configuration from live agent connections, with English and Korean guidance.
- Add client adapters, private recovery copies, atomic writes, conflict checks and shared-skill ownership.
- Isolate browser-harness client settings from real user configurations.
- Include exact planned commands in the companion skill’s control-request procedure.

한국어: 설정에서 Codex·Claude Code·Cursor·GitHub Copilot의 MCP와 협업 스킬을 등록·갱신·해제할 수 있습니다. 기존 설정을 보존하고 실제 접속 여부를 따로 표시합니다.

## 0.3.0 — Preview · 2026-09-16

First public preview of Conn, a shared terminal for humans and AI agents.

### Collaboration

- One native terminal session shared through local IPC and MCP.
- Observe, Co-pilot and Autopilot modes, explicit control requests, per-command review, execution grace and human takeover.
- A unified timeline for commands and collaboration, including denied requests, policy reasons and collapsible original request details.
- Local shell, PowerShell, Command Prompt, WSL, SSH and Docker profile configuration.

### Interface

- Shared Svelte/Tauri UI with a browser adapter for testing the native harness.
- English and Korean UI, profiles in Settings, a compact app menu, and a state-aware Conn icon that respects reduced motion.
- Improved approval and grace card wrapping.

### Correctness and distribution

- Correct Windows shell termination reporting and tolerate a concurrent shell exit when closing a tab.
- Binary-first English and Korean onboarding with OS-specific download links and embedded collaboration videos.
- Copy MCP JSON or Codex TOML from Settings using the bundled CLI and current endpoint; no Cargo installation or PATH setup required.
- macOS release jobs use Developer ID signing and notarization for the app, sidecar, standalone CLI and disk image; Windows previews remain unsigned.
- Keep the standard browser test launcher independent from manual recording port overrides.
- Refuse to start a new shell when policy loading fails; preserve the last good rules on reload failure.
- Preserve complete clean input for policy checks and audit even when long commands wrap across terminal rows.
- English and Korean onboarding and contributor guides.
- CI for frontend, Rust and desktop builds; version-checked CLI/installer packaging with checksums and draft-only releases.

### Preview limitations

- Policy is a cooperative guard, not a sandbox. Completion/history edits still have reconstruction limits; see [the trust model](docs/security.md).
- No detach/reattach or automatic updater. Release installers require platform smoke testing. Mac publication requires successful Developer ID signing and notarization checks. Windows previews intentionally ship without certificate signing. See [platform policy](docs/platform-support.md).
