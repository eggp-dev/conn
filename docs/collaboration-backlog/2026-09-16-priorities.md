# Conn improvement queue / 개선 작업 목록

Updated: 2026-09-16. Goal: 100+ GitHub stars through a clear first-use experience, easy installation, and reliable human–agent collaboration. This is a prioritization document, not permission to publish or a statement that all items are release blockers.

목표는 첫 방문자가 가치를 이해하고 설치 → 연결 → 첫 협업까지 쉽게 도달하게 하는 것이다. 아래 우선순위는 작업 순서이며 보안 취약점 심각도와 다르다. 기존 관찰 기록은 덮어쓰지 않는다.

## 1. v0.6.0 implementation / 구현 현황

| Order | ID | Work / 작업 | Acceptance / 완료 기준 |
|---|---|---|---|
| 1 | DIST-C001 | In-app automatic updates / 앱 내 자동업데이트 | 백그라운드 확인·다운로드 진행·서명 검증·사용자 선택에 따른 설치/재시작. 활성 셸을 강제 종료하지 않음. 실패 후 현재 앱 정상 실행 |
| 2 | DIST-C002 | Frictionless macOS first install / macOS 설치 동의 화면 제거 | DMG 열기 → Applications 복사. 불필요한 라이선스 Agree 없음. 라이선스 배포물 보존. 코드 서명·공증·stapling 유지 |
| 3 | UX-C003 | Timeline strip density / 하단 타임라인 최소 크기 | 최소 표시 너비·간격·겹치지 않는 선택 영역. 넘침 탐색. 10/75/200개 기록 및 좁은 창 검증. 과거 탐색 위치 유지 |
| 4 | UX-C004 | Settings follow-through / 설정 정리 후속 | 저장·즉시 적용·새 탭 기본값 의미 통일. 프로필 기본값 표시 중복 축소. 사용자 지정 프리셋 표시, 숫자 직접 입력 검토. 진단은 상태 요약 후 상세 정보 |

### DIST-C001 · Automatic updates

The v0.6.0 candidate adds native checking, verified download and explicit installation/restart. Release signing credentials and the platform release matrix must be ready before publication.

- Tauri updater integration; a shared update state for automatic checks and manual “Check for updates”.
- Show available version, progress, failure/retry, and install/restart choice. Preserve manual download fallback where in-app updates are unsupported.
- Publish updater metadata, platform artifacts and signatures in CI. Add dedicated updater signing secrets and embed only the public key. Apple signing/notarization and updater signatures are different responsibilities.
- macOS Apple Silicon and Windows: test native installation behavior, permissions, relaunch, and active-session handling. Intel Mac remains out of scope.
- Linux: distinguish AppImage self-update from system-managed package updates; do not promise one installation path for every package format.
- Define stable/preview channels explicitly; do not silently migrate stable users to preview builds.
- Verify old → new version, interrupted download, invalid signature, missing artifact, offline checks, and concurrent requests.
- Existing installations without updater support need one manual installation of the first updater-enabled version.
- A passing build is not proof of successful installed-app upgrade; require native candidate verification.

### DIST-C002 · macOS installation

Previously `bundle.licenseFile` injected a DMG agreement. The candidate instead bundles LICENSE as a resource and verifies mounting with closed stdin.

- Remove the macOS DMG agreement at bundle configuration time; preserve required license notices as bundled resources/distribution files. Check effects on Windows/Linux before changing shared configuration.
- Update verification script and related tests to verify a DMG without an agreement prompt. Do not rewrite a signed DMG after signing.
- Verify Applications destination/link, arm64 artifact, Gatekeeper launch, notarization and stapled ticket on the final delivered artifact.
- Keep normal first-launch OS security prompts distinct from the removable DMG license agreement.

## 2. Included collaboration and UI changes / 함께 포함되는 변경

These changes exist in the working tree. They are not claimed merged or released.

- Timeline isolation by shell session and preservation of saved activity under its own session.
- Bash prompt-array integration repair for shell-authored human command recording; no raw authentication/editor keystroke capture.
- Handback hint reserves space, Escape dismisses it, and timeline uses the same dock motion convention.
- Shared menu/control-centre surfaces; settings and command navigation live in the left menu.
- Central settings window with stable sidebar and content transition.
- Compact agent setup rows, advanced permissions/profile/pacing disclosures, simpler policy rules, contextual scope selection, bilingual labels.
- Settings focus containment, background inertness, Escape focus restoration, labels and selected-state semantics.

Validation observed this session: frontend build and 36 existing frontend tests passed; browser navigation, collapsed options, labeled pacing control, reverse-Tab containment, and Escape restoration checked. Existing tests do not substitute for native UI acceptance.

Remaining checks:

- macOS/Windows installed app, Korean layout, narrow window and 200% zoom.
- Screen-reader semantics, measured contrast, reduced motion, and real frame-time/PTY-resize profiling.
- Settings actual save/apply/cancel outcomes and profile backend variants; no permission weakening for testing.
- Native collaboration after UI and shell integration changes. Separate development-browser evidence from signed-release evidence.

## 3. Existing collaboration investigations / 기존 협업 백로그

| ID | Work / 작업 | Current status / 상태 |
|---|---|---|
| BUG-C001 | Interactive remote Codex `input_pending` / 원격 대화형 입력 실패 | 최소 재현 대기, 수정 보류 |
| BUG-C002 | Snapshot rows after resize / 리사이즈 후 과거 행 잘림 | 화면·스냅샷 동시 비교 필요 |
| UX-C001 | Control grant versus command approval / 제어 승인과 명령 승인 구분 | 이해도 검증; 승인 정책 유지 |
| UX-C002 | Connected, controlling, idle / 연결·제어·유휴 구분 | 사용자 이해도 검증 |
| TEST-C001 | Full Xcode scripting dictionary check | 전체 Xcode 환경 재검증 |
| TEST-C002 | Signed automation candidate | cold launch·sender 소유권·takeover·종료 수용 테스트 |
| DOC-C001 | Private target versus caller recording boundary | 호출자와 대상의 기록 범위 안내 정리 |
| PROD-C001 | Human corrects agent's path in the shared shell | 제품 가설 검증 중 |
| DEMO-C001 | Correction → handback → agent resumes | 가상 데이터 재촬영, 보류 |

Evidence: [remote collaboration](2026-09-16-remote-codex.md), [external automation](2026-09-16-applescript.md), [timeline density](2026-09-16-timeline-density.md).

## Scope guard / 범위

- Do not reclassify earlier fixes as new defects without rechecking the current code/release.
- The earlier review's P3 suggestions remain separately deferred unless explicitly accepted; this list does not reinstate all of them.
- Do not commit, push, merge, or release based solely on this backlog update.

## 2026-09-17 candidate verification

DIST-C001/C002 and UX-C003/C004 are implemented in the v0.6.0 candidate. Local Rust workspace, frontend, release-tooling and native updater failure-path checks pass. Browser checks cover 10/75/200-record density, history position, new-shell isolation, numeric/custom pacing, unsaved-profile discard and Korean narrow layout. See [candidate validation](../releases/v0.6.0-validation.md). Native installed-version upgrades and previously deferred investigations remain explicit follow-ups.
