# Changelog

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
