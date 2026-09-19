# 플랫폼 지원과 배포

[English](platform-support.md) · 한국어 · [설치](getting-started.ko.md) · [릴리스](releasing.ko.md)

Conn은 MIT 라이선스 소스와 네이티브 바이너리를 [GitHub Releases](https://github.com/eggp-dev/conn/releases/tag/v0.8.1)로 제공합니다. 모든 데스크톱 패키지는 같은 Rust 엔진과 Svelte UI를 사용하며 브라우저 하네스는 개발용 어댑터입니다.

## 다음 프리뷰 릴리스 대상

v0.4.1 이후 Intel Mac 배포는 잠시 중단합니다. 기존 공개 Intel 파일은 삭제하거나 교체하지 않고 유지합니다.

아래 표는 빌드·배포 기준이며 모든 대상의 실제 조작 검증이 끝났다는 뜻은 아닙니다. 각 릴리스에 실제 빌드, 서명, 설치, 실행 결과를 구분해 기록합니다. 날짜별 근거는 [백엔드 검증 기록](backend-verification.md)에서도 확인할 수 있습니다.

| 대상 | 빌드 러너 | 파일 | 배포 방침 |
|---|---|---|---|
| Ubuntu 24.04·26.04 x64 | `ubuntu-24.04` | `.deb`, `.AppImage`, CLI `.tar.gz` | 24.04에서 빌드하고 두 Ubuntu 버전의 실행 결과를 기록합니다. |
| Windows x64 | `windows-2022` | NSIS `.exe`, CLI `.zip` | 의도적인 무서명 프리뷰입니다. 설치 경고와 Windows 데스크톱 검증 결과를 안내합니다. |
| macOS Apple Silicon | `macos-15` | `.dmg`, CLI `.tar.gz` | 공개 파일에는 Developer ID 서명과 공증 승인이 필수입니다. |

[시작하기](getting-started.ko.md)에서 설치 파일을 선택하세요. 데스크톱에는 같은 버전의 CLI가 포함됩니다. **설정 → 에이전트 → 에이전트 연결**에서 CLI 절대 경로와 현재 연결 주소가 포함된 설정을 복사할 수 있습니다. Rust·Node.js 설치나 `PATH` 변경은 필요하지 않습니다.

### Linux

CLI를 포함한 릴리스 바이너리는 Ubuntu 24.04에서 빌드합니다. 더 새로운 Ubuntu에서 만든 로컬 빌드는 새 glibc·시스템 라이브러리에 의존할 수 있으므로 릴리스 기준을 대신하지 않습니다. 같은 `.deb`·AppImage를 내려받아 24.04와 26.04에서 그래픽 세션까지 확인합니다. AppImage에도 시스템 라이브러리가 필요합니다. [Tauri AppImage 안내](https://v2.tauri.app/distribute/appimage/)를 참고하세요.

### Windows

이번 프리뷰에 서명 인증서는 필수가 아닙니다. SmartScreen·알 수 없는 배포자 경고가 나올 수 있고 관리되는 PC에서는 설치가 차단될 수 있습니다. 릴리스 안내에 설치 파일·CLI가 무서명임을 표시합니다. 시스템 보호 기능을 끄도록 안내하지 않습니다. Windows 서명은 추후 별도로 추가할 수 있습니다. [Tauri Windows 서명 안내](https://v2.tauri.app/distribute/sign/windows/)를 참고하세요.

Windows Server CI의 빌드 통과는 Windows 데스크톱·WebView2·ConPTY 실제 조작 검증을 뜻하지 않습니다. 실제 확인한 데스크톱 OS, 셸, 설치 동작을 기록합니다.

### macOS

[서명 파이프라인](macos-signing.ko.md)은 GitHub의 Mac 러너, Developer ID Application 자격 증명, Apple 공증을 사용합니다. 앱·내장 CLI·독립 CLI의 서명을 요구하며 앱과 DMG에는 공증 티켓을 첨부합니다. 독립 CLI는 ZIP으로 제출해 공증하지만 실행 파일 자체에 티켓을 첨부할 수는 없습니다.

Apple Silicon 대상은 최종 파일 해시와 연결된 공개 `-signing.json` 보고서를 만듭니다. 성공한 서명 보고서는 해당 검사 통과를 뜻하며 첫 실행 GUI 검증까지 완료됐다는 뜻은 아닙니다. Apple Silicon에서 브라우저로 새로 내려받아 실행한 뒤 실제 조작 검증을 주장하세요.

Tauri 설정의 macOS 최소 버전은 12.0이지만 검증된 최소 버전을 뜻하지는 않습니다. Linux ARM, Windows ARM, Ubuntu 22.04와 다른 Linux 배포판은 이번 프리뷰의 바이너리 호환 보장 범위에 포함되지 않습니다.

## 릴리스 검증표

내려받은 릴리스 후보로 확인합니다. 태그·커밋, 파일명·SHA-256, OS 버전, CPU, 셸, Linux 화면 세션을 기록하세요. 각 항목은 **통과**, **실패**, **미검증**으로 구분하며 CI 통과를 네이티브 조작 검증으로 바꾸어 적지 않습니다.

- [ ] 체크섬을 비교하고 깨끗한 사용자 계정 또는 VM에 설치합니다.
- [ ] Rust·Node.js·개발 서버 없이 실행하고 번들 CLI 경로와 설정의 에이전트 연결 복사를 확인합니다.
- [ ] 복사한 설정으로 외부 에이전트를 연결해 화면과 임시 파일을 읽습니다.
- [ ] 입력·붙여넣기·크기 변경·탭 생성과 종료를 확인하고 탭과 함께 셸도 종료되는지 봅니다.
- [ ] Ubuntu는 bash, Windows는 PowerShell·cmd.exe, Mac은 Apple Silicon의 zsh를 확인합니다.
- [ ] 요청 승인, 별도 요청 거절, 실행 유예 취소, 직접 입력으로 제어권 회수를 확인합니다. 셸의 실제 결과와 타임라인 요청 원문도 봅니다.
- [ ] 네이티브 앱에서 영·한 전환, 키보드 포커스, 줄바꿈, 모션 줄이기를 확인합니다.
- [ ] 앱을 다시 열어 설정·타임라인을 확인하고 제거한 뒤 남는 사용자 데이터를 기록합니다.
- [ ] Ubuntu: 24.04·26.04에서 두 패키지 형식을 실제 그래픽 세션으로 확인합니다.
- [ ] Windows: 신뢰 경고, WebView2 준비 상태, named pipe 연결·정리를 기록합니다.
- [ ] macOS: 서명 보고서를 확인하고 Apple Silicon에서 새로 받은 앱·CLI 실행을 시험합니다.

SSH·Docker·WSL·Git Bash는 프로필 옵션입니다. 실제 시험한 외부 환경만 검증 완료로 표시하세요. 빌드, 패키지 생성, 서명 검사, 실제 조작은 각각 별도 근거입니다.

## 공개와 업데이트

워크플로는 바이너리·설치 파일 7개, Mac 업데이트 아카이브, 업데이트 서명 3개, `latest.json`, Mac 서명 보고서, `SHA256SUMS`를 담은 **Draft prerelease**를 준비하며 자동 공개하지 않습니다. Mac 서명 근거가 없거나 검사에 실패하면 초안 업로드를 막습니다. Windows 무서명은 명시적인 프리뷰 방침입니다.

관리자는 공개 전에 [릴리스 검사](releasing.ko.md)를 검토하고 실제 실행 결과와 한계를 기록하며 다운로드 링크가 파일과 일치하는지 확인합니다. v0.6.0부터 macOS·Windows·AppImage는 서명된 앱 내 업데이트를 지원합니다. deb는 수동 또는 패키지 관리자 업데이트를 사용합니다. 기존 버전은 한 번 직접 업그레이드해야 합니다. [업데이트 안내](getting-started.ko.md#업데이트-v060-이상)를 참고하세요.
