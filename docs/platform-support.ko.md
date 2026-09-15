# 플랫폼 지원과 배포

[English](platform-support.md) · 한국어 · [설치](getting-started.ko.md) · [릴리스](releasing.ko.md)

Conn은 MIT 라이선스 소스와 네이티브 바이너리를 GitHub Releases로 배포합니다.
모든 데스크톱 플랫폼이 같은 Rust 엔진과 Svelte UI를 사용합니다. 브라우저 하네스는
이 엔진에 연결하는 개발용 어댑터입니다.

## 첫 프리뷰 배포 방침

아래는 배포 목표입니다. 해당 릴리스 파일로 검증표를 통과해야 실제 검증된 플랫폼으로
표시합니다. 확인한 근거는 날짜가 있는 [검증 기록](backend-verification.md)에 남깁니다.

| 플랫폼 | 릴리스 빌드 환경 | 파일 | 공개 방침 |
|---|---|---|---|
| Ubuntu 24.04·26.04 x64 | GitHub `ubuntu-24.04` | `.deb`, `.AppImage`, CLI `.tar.gz` | 같은 CI 산출물을 두 Ubuntu 버전에서 검증합니다. |
| Windows x64 | GitHub `windows-2022` | NSIS `.exe`, CLI `.zip` | 무서명 프리뷰로 배포하며 배포자·SmartScreen 경고를 안내합니다. 공개 전에 Windows 데스크톱에서 검증합니다. |
| macOS Apple Silicon | GitHub `macos-15` | `.dmg`, CLI `.tar.gz` | Apple 가입 후 Developer ID 서명·공증을 적용해 공개합니다. 현재 ad-hoc 산출물은 테스트용입니다. |
| macOS Intel | GitHub `macos-15-intel` | `.dmg`, CLI `.tar.gz` | 동일한 서명 방침을 적용하고 Intel 환경에서도 별도로 검증합니다. |

Tauri 설정의 macOS 최소 버전은 현재 12.0이지만, 실제 검증된 최소 버전을 뜻하지는
않습니다. Windows Server CI도 Windows 데스크톱·WebView2 동작을 보증하지 않습니다.
각 릴리스에 실제 확인한 OS 버전을 적습니다. Linux ARM, Windows ARM, Ubuntu 22.04와
다른 Linux 배포판의 바이너리 호환성은 이번 프리뷰의 보장 범위가 아닙니다.

### Linux: 대상 중 오래된 Ubuntu에서 빌드

개발 PC는 Ubuntu 26.04입니다. 여기서 만든 파일은 더 새로운 glibc·시스템 라이브러리에
의존할 수 있으므로 Ubuntu 24.04 릴리스 빌드를 대신하지 않습니다. AppImage에도 시스템
라이브러리 기준이 있으며 모든 Linux 호환을 보장하지 않습니다. CLI sidecar를 포함한
릴리스 빌드는 `ubuntu-24.04`로 고정하고, 그 결과를 깨끗한 24.04·26.04 환경에서
검증합니다. [Tauri AppImage 안내](https://v2.tauri.app/distribute/appimage/)를 참고하세요.

### Windows: 먼저 무서명으로 배포

이번 프리뷰에는 Windows 인증서가 필수가 아닙니다. 무서명 설치 파일·CLI에 SHA-256
체크섬과 서명 상태 안내를 제공합니다. SmartScreen이나 알 수 없는 배포자 경고가
나타날 수 있고 관리되는 PC에서는 설치가 차단될 수 있습니다. 시스템 보호 기능을 끄도록
안내하지 않습니다. 인증서·서명 서비스는 추후 도입할 수 있으며 Windows 서명은 Apple
공증과 별개입니다. [Tauri Windows 서명 안내](https://v2.tauri.app/distribute/sign/windows/)를 참고하세요.

### macOS: 가입 후 서명·공증 연결

Apple 개발자 가입은 대기 중입니다. 현재 워크플로의 `APPLE_SIGNING_IDENTITY=-`는
임시 서명(ad-hoc)이며 공증하지 않습니다. 이 파일은 테스트·초안 상태로 유지합니다.
가입만으로 CI가 완성되지는 않습니다. 공개 전에 [macOS 서명 준비표](macos-signing.ko.md)를
진행합니다.

## 릴리스 검증표

개발 소스만 실행하지 말고 내려받은 초안 파일로 확인합니다. 태그·커밋, 파일명·SHA-256,
OS 버전, CPU, 셸, Linux 데스크톱 세션(Wayland/X11), 통과·실패 관찰 결과를 기록합니다.

- [ ] 체크섬을 확인하고 새 사용자 계정 또는 VM에서 데스크톱 설치·CLI 압축 해제를 진행합니다.
- [ ] Node.js·Rust·개발 서버 없이 실행하고 `conn --version`과 진단의 번들 CLI를 확인합니다.
- [ ] 입력·붙여넣기·크기 변경·여러 탭 생성과 종료를 시험하고 탭 종료 시 셸도 끝나는지 확인합니다.
- [ ] 로컬 셸을 검증합니다. Ubuntu는 bash, Windows는 PowerShell·cmd.exe, Mac은 각 아키텍처의 zsh입니다.
- [ ] `conn mcp`로 에이전트를 연결해 임시 파일을 읽고 타임라인 요청 원문을 확인합니다.
- [ ] 삭제를 거절해 파일이 남는지 확인하고, 별도 삭제를 승인해 실제 결과를 확인합니다.
- [ ] Grace 중 취소·키 입력으로 제어권 회수를 시험하고 에이전트 입력이 중단되는지 확인합니다.
- [ ] 네이티브 앱에서 영·한 전환, 키보드 포커스, 줄바꿈, 모션 줄이기를 확인합니다.
- [ ] 재실행 후 설정·타임라인 복원을 확인하고 데스크톱을 제거합니다. 남는 사용자 데이터도 기록합니다.
- [ ] Ubuntu 24.04·26.04 모두에서 `.deb`·AppImage를 실제 그래픽 세션까지 검증합니다.
- [ ] Windows 설치 경고, WebView2 준비 상태, named pipe 연결·정리를 기록합니다. WSL을 검증 완료로 소개하려면 별도로 실행합니다.
- [ ] Mac 양쪽 아키텍처에서 앱·sidecar·독립 CLI의 서명·공증과 새 브라우저 다운로드 후 실행을 검증합니다.

SSH·Docker·WSL·Git Bash는 프로필 옵션입니다. 실제 외부 환경을 사용한 뒤에만 검증
완료로 표시합니다. 자동 빌드, PTY 테스트, 패키지 생성, 네이티브 조작은 각각 별도 근거입니다.

## 공개와 업데이트

워크플로는 바이너리 9개와 `SHA256SUMS`를 담은 **Draft prerelease**를 만듭니다.
관리자가 검증 결과를 보고 명시적으로 공개합니다. Apple 준비 전에는 세 플랫폼 초안을
미공개로 유지할 수 있으며, 예정한 공증 패키지를 ad-hoc 파일로 조용히 대체하지 않습니다.

업데이트는 새 릴리스를 직접 받는 방식입니다. 앱 내 자동 업데이트·업데이터 서명 키,
앱 스토어 등록, 별도 패키지 저장소는 이번 프리뷰에 포함되지 않습니다.
