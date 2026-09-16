# Changelog

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
