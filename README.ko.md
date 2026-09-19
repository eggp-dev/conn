<p align="center">
  <img src="frontends/tauri/public/conn-icon.svg" width="64" height="64" alt="Conn 아이콘: 열린 C와 제어 커서">
</p>

<h1 align="center">Conn</h1>
<p align="center"><strong>에이전트는 내 터미널에서 일하고, 키보드는 내가 쥡니다.</strong></p>
<p align="center"><a href="README.md">English</a> · 한국어</p>

Conn은 지금 쓰는 AI 에이전트와 함께 쓰는 터미널입니다. 에이전트는 **내가** 들어가 있는 셸에서 이어서 일합니다. 내가 접속해 둔 서버, 내가 활성화한 환경, 내가 고른 폴더 그대로입니다. 실행되는 명령은 전부 눈앞에서 보이고, 위험한 명령은 내 승인을 기다리며, 키보드를 치면 제어권이 돌아옵니다.

- **내가 있는 곳에서 시작합니다.** SSH 접속, 컨테이너 진입, 가상환경 활성화는 내가 직접 하고, 그 살아 있는 셸을 에이전트가 이어받습니다. 숨김 프롬프트에 입력한 암호는 에이전트에게 전달되지 않습니다.
- **몰래 실행되는 것은 없습니다.** 에이전트가 제출하는 모든 명령에는 한 줄짜리 이유가 붙습니다. 기본 설정에서 `rm`, `sudo`, `git push --force`, `kubectl delete`, 파일 덮어쓰기는 내 승인을 기다리고, 승인 한 번은 동작 하나에만 적용됩니다. SSH 세션 안이나 Conn이 들여다볼 수 없는 프로그램 안에서는 에이전트의 **모든** 명령이 내 확인을 기다립니다.
- **입력하면 바로 넘겨받습니다.** 키를 하나만 눌러도 에이전트의 다음 입력이 셸에 닿기 전에 제어권이 돌아옵니다. 잘못된 곳을 고친 뒤 "화면을 읽고 이어서 해"라고 말하면 됩니다.
- **쓰던 에이전트, 쓰던 모델 그대로.** **Codex, Claude Code, Cursor, GitHub Copilot**과 MCP로 연결됩니다. Conn은 모델을 실행하지 않고, 추가 구독도 필요 없습니다.

[![15초 미리보기. 사람이 SSH로 서버에 로그인하고, 에이전트가 캐시 삭제를 제안하자 거부한 뒤 직접 입력해 고치는 모습](docs/assets/conn-remote-preview-ko.webp)](docs/assets/conn-remote-ko.mp4)

*내 서버에는 내가 로그인합니다. 에이전트가 캐시를 통째로 지우겠다고 합니다. 안 된다고 하고 내가 직접 고치면, 에이전트가 나머지를 끝냅니다.* [52초 전체 영상](docs/assets/conn-remote-ko.mp4) · [English video](docs/assets/conn-remote-en.mp4) · [한 번에 이어서 찍었습니다: 촬영 기록](media/demo/RECORDING-REMOTE.md)

## 설치

```sh
# macOS (Apple Silicon)
brew install --cask eggplantiny/tap/conn

# Linux (x86_64): 내 계정에만 AppImage 설치, 체크섬 검증, sudo 불필요
curl -fsSL https://raw.githubusercontent.com/eggplantiny/conn/main/scripts/install.sh | sh
```

Rust나 Node.js는 필요 없습니다. 에이전트 연결 도구가 함께 들어 있습니다. 두 방법 모두 아래 다운로드와 같은 서명된 릴리스 파일을 설치하며, 이후에는 앱 안에서 업데이트됩니다. Windows는 설치 파일을 사용하세요.

| 운영체제 | v0.8.1 프리뷰 |
|---|---|
| macOS · Apple Silicon | [Mac용 다운로드](https://github.com/eggplantiny/conn/releases/download/v0.8.1/conn-v0.8.1-aarch64-apple-darwin-desktop.dmg) |
| Windows · x64 | [설치 파일 다운로드](https://github.com/eggplantiny/conn/releases/download/v0.8.1/conn-v0.8.1-x86_64-pc-windows-msvc-setup.exe) |
| Ubuntu · x64 | [.deb 다운로드](https://github.com/eggplantiny/conn/releases/download/v0.8.1/conn-v0.8.1-x86_64-unknown-linux-gnu-desktop.deb) · [AppImage](https://github.com/eggplantiny/conn/releases/download/v0.8.1/conn-v0.8.1-x86_64-unknown-linux-gnu-desktop.AppImage) |

Mac 배포 파일은 서명·공증되어 있습니다. Windows 프리뷰는 무서명이며, Linux는 Ubuntu 24.04에서 빌드하며, AppImage는 앱 안에서 업데이트되고 `.deb`는 새 패키지를 설치해 업데이트합니다. Intel Mac 배포는 잠시 중단했습니다. [설치·업데이트 안내](docs/getting-started.ko.md) · [전체 파일·체크섬](https://github.com/eggplantiny/conn/releases/tag/v0.8.1)

## 처음 5분

1. **Conn을 열고** **설정 → 프로필**에서 사용할 셸을 고릅니다.
2. **에이전트를 연결합니다.** **설정 → 에이전트**에서 클라이언트를 골라 **설정하기**를 누르고 클라이언트를 재시작합니다. 처음 연결되면 Conn에 **참여를 요청합니다** 카드가 뜹니다. **허용**을 누르세요. [연결 안내](docs/agent-integrations.ko.md)
3. **평소처럼 터미널에서 작업하다가** 에이전트에게 말합니다. *"Conn 터미널을 보고 거기서 이어서 해 줘."*
4. **한 번 끼어들어 보세요.** 에이전트가 제어권을 쥐고 있을 때 키를 누르면 키보드가 바로 돌아옵니다. [첫 협업 예제](docs/first-collaboration.ko.md)가 임시 폴더 하나로 이 과정을 안내합니다. 서버는 필요 없습니다.

Codex는 데스크톱 앱·CLI·IDE 확장에서 로컬 작업으로 사용하세요. 대화는 쓰던 에이전트 클라이언트에서 하고, 터미널은 함께 쓰는 작업 공간입니다.

## 결정은 내가 합니다

| | |
|---|---|
| **세 가지 모드** | **Observe**: 에이전트는 화면만 봅니다. **Co-pilot**: 에이전트가 한 줄을 제안하고 내가 Enter를 누릅니다. **Autopilot**: 에이전트가 직접 실행하고, 중요한 순간에는 정책이 나에게 묻습니다. |
| **승인** | 거부·확인 규칙을 YAML 정책 파일 하나로 관리합니다. 위험한 명령을 다른 명령과 이어 붙이면 거절되고, 단독으로 다시 보내야 합니다. 원격 서버나 다른 프로그램 안에서는 명령을 하나씩 확인합니다. |
| **참여 전에 묻기** | 새 에이전트 연결은 내가 허용하기 전까지 세션에 대해 아무것도 알 수 없습니다. |
| **한 번에 한 탭** | 에이전트는 한 탭에서만 제어권을 갖고, 받은 세션에 머물며, 내가 다른 탭으로 가도 따라오지 않습니다. |
| **기록** | 타임라인에 명령, 제어권 전환, 승인과 거절이 에이전트가 밝힌 이유와 함께 남습니다. |
| **비공개 세션** | 외부 실행 도구가 시작한 세션은 내가 공유하기 전까지 에이전트에게 보이지 않습니다. |

직접 입력해도 **이미 실행 중인 프로세스가 멈추지는 않습니다.** Ctrl-C 같은 터미널 조작은 그대로 쓰면 됩니다. Conn은 **운영체제 샌드박스가 아닙니다.** 명령은 내 계정 권한으로 실행됩니다. 에이전트는 로컬 소켓으로만 연결되고, Conn은 네트워크 포트를 열지 않습니다. [신뢰 모델](docs/security.md) · [정책](docs/policy.md)

## 다른 협업: 내가 테스트를 추가하면 에이전트가 고칩니다

https://github.com/user-attachments/assets/4f06180c-6a2c-4c10-99e1-7941db86470b

에이전트가 실패하는 테스트를 고칩니다. 내가 키보드를 잡고 실패하는 경계 조건을 하나 추가한 뒤, 터미널을 읽고 이어가라고 말합니다. 62초 영상입니다. [촬영 설명](docs/demo-test-repair.ko.md)

## v0.8.0에서 달라진 점

- 창을 최소화했거나 다른 앱 뒤에 있거나 다른 탭을 보고 있어도, 에이전트는 공유된 세션을 계속 읽고 작업합니다.
- 새 에이전트 연결은 **허용**을 누를 때까지 대기합니다. **설정 → 에이전트**에서 이 확인을 끌 수 있습니다.
- 제어권은 한 번에 한 탭에만 주어지고, 내가 입력하면 즉시 돌아옵니다.

앱을 업데이트한 뒤 MCP 클라이언트도 함께 재시작하세요. [변경 이력](CHANGELOG.md) · [제품 계약](docs/PRD.ko.md) · [확장 안내](docs/extensions.ko.md)

## 더 알아보기

- [이전 영상: 에이전트가 엉뚱한 폴더에서 시작하면 내가 고칩니다](docs/demo.ko.md)
- [협업 이야기: 같은 터미널에서의 작은 보정](docs/collaboration-story.ko.md)
- [FAQ: 제어권·SSH·암호·기록](docs/faq.ko.md)
- [프로필과 백엔드](docs/backends.md) · [CLI 설치](docs/getting-started.ko.md) · [외부 자동화](docs/external-automation.ko.md)
- [기여하기](CONTRIBUTING.ko.md) · [문제 제보](https://github.com/eggplantiny/conn/issues/new/choose) · [보안 제보](SECURITY.md)

**프리뷰 · MIT 라이선스.**

에이전트에게서 키보드를 되찾고 싶었던 순간이 있었다면 **Star로 Conn을 알려 주세요.** 그 순간이 어땠는지 이슈로 들려주시면 더 좋습니다.
