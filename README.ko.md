<p align="center">
  <img src="frontends/tauri/public/conn-icon.svg" width="88" height="88" alt="Conn 아이콘: 열린 C와 제어 커서">
</p>

<h1 align="center">Conn</h1>
<p align="center"><strong>쓰던 에이전트와, 하나의 터미널에서.</strong></p>
<p align="center"><a href="README.md">English</a> · 한국어</p>

Conn은 나와 AI 에이전트가 함께 쓰는 터미널입니다. 쓰던 에이전트에서 대화하고, 같은 셸에서 작업을 지켜보다가 직접 입력해 이어받으세요. 내 작업이 끝나면 에이전트에게 현재 화면을 다시 읽고 계속하라고 요청하면 됩니다.

## 다운로드

**앱을 내려받아 설치하세요. Rust나 Node.js는 필요하지 않습니다.** 에이전트 연결에 쓰는 Conn CLI도 앱에 포함됩니다.

| 운영체제 | v0.5.1 프리뷰 다운로드 |
|---|---|
| macOS · Apple Silicon | [Apple Silicon용 다운로드](https://github.com/eggplantiny/conn/releases/download/v0.5.1/conn-v0.5.1-aarch64-apple-darwin-desktop.dmg) |
| Windows · x64 | [Windows 설치 파일](https://github.com/eggplantiny/conn/releases/download/v0.5.1/conn-v0.5.1-x86_64-pc-windows-msvc-setup.exe) |
| Ubuntu · x64 | [.deb 다운로드](https://github.com/eggplantiny/conn/releases/download/v0.5.1/conn-v0.5.1-x86_64-unknown-linux-gnu-desktop.deb) · [AppImage](https://github.com/eggplantiny/conn/releases/download/v0.5.1/conn-v0.5.1-x86_64-unknown-linux-gnu-desktop.AppImage) |

[전체 다운로드와 체크섬](https://github.com/eggplantiny/conn/releases/tag/v0.5.1) · [설치 도움말](docs/getting-started.ko.md) · [플랫폼 지원](docs/platform-support.ko.md)

새 릴리스는 Apple Silicon, Windows x64, Linux x64를 대상으로 합니다. Intel Mac 배포는 잠시 중단하며 기존 파일은 [이전 릴리스](https://github.com/eggplantiny/conn/releases)에 유지합니다.

Windows 프리뷰는 무서명으로 배포하므로 알 수 없는 게시자 안내가 나타날 수 있습니다. Linux 패키지는 Ubuntu 24.04에서 빌드합니다. 실제 서명·검증 범위는 릴리스 노트에서 확인할 수 있습니다.

## 함께 작업하는 모습

https://github.com/user-attachments/assets/4f06180c-6a2c-4c10-99e1-7941db86470b

에이전트가 실패한 테스트를 고칩니다. 내가 빠진 조건을 추가합니다. 에이전트가 달라진 터미널을 읽고 수정을 이어갑니다.

[English video](https://github.com/user-attachments/assets/c20ec712-1915-44c2-a6a2-08b83e6951c4) · [촬영 설명](docs/demo.ko.md)

*실제 Codex CLI와 Conn을 연결했습니다. 준비된 예제 프로젝트에서 사람 역할의 입력을 자동화했고, 대기 시간을 줄여 편집했습니다.*

## 첫 협업 시작하기

1. **Conn을 여세요.** 위 앱을 설치하고 **설정 → 프로필**에서 사용할 셸을 선택합니다.
2. **에이전트를 연결하세요.** **설정 → 에이전트**에서 Codex·Claude Code·Cursor·GitHub Copilot을 고르고 **설정하기**를 누릅니다. 앱에 포함된 CLI로 MCP와 협업 스킬을 함께 등록합니다. 클라이언트를 재시작하고 Conn은 켜 두세요. [연결 안내](docs/agent-integrations.ko.md)
3. **요청을 하나 보내세요.** 오른쪽 위 제어 메뉴에서 **Autopilot**을 선택하고 **conn 을 넘길 때마다 묻는다**를 켜세요. **실행 유예 2초**로 설정한 뒤 에이전트에게 요청합니다.

   > Conn으로 현재 화면을 읽고, 현재 폴더를 확인할 제어권을 요청해. 정확한 명령을 포함하고 파일은 변경하지 마. 내가 거절하거나 제어권을 되찾으면 멈춰.

요청을 확인해 승인하거나 거절하세요. Conn에 직접 입력하면 내가 제어권을 이어받습니다. 작업을 마친 뒤에는 에이전트에게 현재 화면을 다시 읽고 계속하라고 요청하세요. **타임라인**에서 명령과 제어권 전환, 요청 원문을 함께 확인할 수 있습니다.

제어권을 되찾으면 에이전트의 추가 입력이 중단됩니다. 이미 실행 중인 명령은 별도로 중지해야 합니다.

## 쓰던 도구와 함께

Conn은 데스크톱 앱으로도, 기존 터미널 안의 세션으로도 사용할 수 있습니다. 에이전트는 **MCP 또는 CLI**로 연결하며, Conn 자체가 모델을 실행하지는 않습니다. 영상에서는 Codex CLI를 사용했습니다. MCP stdio를 지원하는 다른 로컬 클라이언트에도 복사한 서버 설정을 등록할 수 있습니다.

CLI만 사용하려면 릴리스의 `-cli` 압축 파일을 받아 `conn`을 `PATH`에 등록한 뒤 터미널에서 실행하세요. [CLI 설치와 소스 빌드](docs/getting-started.ko.md)

## 더 알아보기

| 필요한 내용 | 문서 |
|---|---|
| 설치·연결·문제 해결 | [시작하기](docs/getting-started.ko.md) |
| 로컬 셸·WSL·SSH·Docker | [백엔드와 프로필](docs/backends.md) |
| 승인·실행 규칙 | [정책](docs/policy.md) |
| 소스 빌드·기여 | [기여 안내](CONTRIBUTING.ko.md) |
| 에이전트·프런트엔드 통합 | [프로토콜](docs/protocol.md) · [구조](docs/architecture.md) |

**프리뷰 소프트웨어 · MIT 라이선스.** 명령은 사용자 계정 권한으로 실행됩니다. Conn의 승인 기능은 운영체제 샌드박스가 아닙니다. [신뢰 모델](docs/security.md)과 [보안 제보 안내](SECURITY.md)를 참고하세요.
