# Changelog

## 0.5.1 — Preview · 2026-09-16

- Support `create window with default profile command` for PAM-style AppleScript launch templates, with real native windows, per-window sessions/output, the existing approval/policy path, and automatic control return after input delivery.
- Compile both session-based and window-launch PAM examples in macOS CI. Native interactive verification remains pending.

한국어: PAM의 새 창 생성 문법을 추가했습니다. 창별 탭·출력을 분리하고 시작 명령은 기존 승인·정책을 거쳐 전달한 뒤 제어권을 반환합니다. 실제 Mac 대화형 검증은 남아 있습니다.

## 0.5.0 — Preview · 2026-09-16

- Prevent mode changes from stranding shell input; restore real Co-pilot interrupts and withdraw entrusted control when the human returns.
- Enforce reviewed-shell approval limits in the core, including CLI requests.
- Expose configured/effective modes to MCP clients and distinguish command approval, proposal acceptance and grace co-signing in the timeline.
- Show connection counts and idle activity separately from local integration configuration.
- Explicitly release integration setup locks even when a concurrently launched shell inherits a file descriptor.
- Add a shared external automation service, macOS AppleScript dictionary, session-scoped input requests, Automation settings, and an example PAM launcher. Native Mac/PAM acceptance testing remains unverified in this preview.
- Correct locale/readline-sensitive PTY fixtures and symlinked temporary-path release tests.
- Limit future macOS builds to Apple Silicon; keep existing Intel release assets available.

한국어: 입력·제어권 관련 버그와 타임라인·연결 진단을 수정했습니다. 외부 앱용 AppleScript 연동과 자동화 설정을 추가했으며 실제 Mac/PAM 연동은 이 프리뷰에서 아직 검증하지 못했습니다. 향후 Mac 배포는 Apple Silicon을 대상으로 합니다.

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
