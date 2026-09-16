# Conn 시작하기

[English](getting-started.md) · 한국어 · [Conn 소개로 돌아가기](../README.ko.md)

Conn을 설치하고 에이전트를 연결해 같은 터미널에서 번갈아 작업하세요. 데스크톱 다운로드에는 에이전트 연결용 CLI가 포함되어 있으므로 Rust·Node.js·개발 서버가 필요하지 않습니다.

## 1. 설치하고 세션 열기

OS와 CPU에 맞는 [v0.6.0 프리뷰](https://github.com/eggplantiny/conn/releases/tag/v0.6.0)를 받으세요. 실제 서명 상태와 실행 검증 결과는 릴리스 안내에서 확인할 수 있습니다.

| 플랫폼 | 다운로드 | 설치 |
|---|---|---|
| Ubuntu x64 | [`.deb`](https://github.com/eggplantiny/conn/releases/download/v0.6.0/conn-v0.6.0-x86_64-unknown-linux-gnu-desktop.deb) | 아래 명령으로 설치한 뒤 앱 목록에서 Conn을 엽니다. |
| Ubuntu x64 | [AppImage](https://github.com/eggplantiny/conn/releases/download/v0.6.0/conn-v0.6.0-x86_64-unknown-linux-gnu-desktop.AppImage) | 실행 권한을 준 뒤 파일을 엽니다. |
| macOS Apple Silicon | [`.dmg`](https://github.com/eggplantiny/conn/releases/download/v0.6.0/conn-v0.6.0-aarch64-apple-darwin-desktop.dmg) | DMG를 열고 Conn을 Applications로 옮긴 뒤 그곳에서 실행합니다. |
| Windows x64 | [설치 `.exe`](https://github.com/eggplantiny/conn/releases/download/v0.6.0/conn-v0.6.0-x86_64-pc-windows-msvc-setup.exe) | 설치 파일을 실행하고 시작 메뉴에서 Conn을 엽니다. |

새 릴리스의 Intel Mac 배포는 잠시 중단합니다. 기존 Intel 파일은 [이전 릴리스](https://github.com/eggplantiny/conn/releases)에 유지하며 Intel Mac에 Apple Silicon 파일을 설치하지 마세요.

Ubuntu에서는 다운로드한 폴더에서 실행합니다.

```sh
sudo apt install ./conn-v0.6.0-x86_64-unknown-linux-gnu-desktop.deb
```

AppImage를 선택했다면 다음과 같이 실행합니다.

```sh
chmod +x ./conn-v0.6.0-x86_64-unknown-linux-gnu-desktop.AppImage
./conn-v0.6.0-x86_64-unknown-linux-gnu-desktop.AppImage
```

Linux 빌드 대상은 Ubuntu 24.04·26.04 x64이며 AppImage에도 시스템 라이브러리가 필요합니다. Windows 프리뷰는 의도적으로 무서명 배포하므로 SmartScreen·알 수 없는 배포자 경고가 나올 수 있고 관리되는 PC에서는 설치가 차단될 수 있습니다. Mac 공개 파일은 릴리스의 Developer ID 서명·공증 검사를 통과해야 합니다. 각 대상의 범위는 [플랫폼 지원](platform-support.ko.md)을 참고하세요.

원하면 파일과 함께 [SHA256SUMS](https://github.com/eggplantiny/conn/releases/download/v0.6.0/SHA256SUMS)를 받으세요. Linux는 `sha256sum --ignore-missing -c SHA256SUMS`, macOS는 `shasum -a 256 <파일>` 결과와 해당 줄 비교, Windows는 `Get-FileHash <파일> -Algorithm SHA256`을 사용합니다. 체크섬은 파일 손상을 확인하며 코드 서명과는 별개입니다.

## 2. 터미널 설정

**왼쪽 위 C 아이콘 → 설정**을 엽니다.

1. **터미널 프로필**에서 로컬 셸, 시작 폴더, 기본 프로필을 고릅니다. **+**로 해당 프로필의 새 탭을 열 수 있으며 기존 탭의 프로필은 유지됩니다.
2. **테마 → 언어**에서 English 또는 한국어를 고릅니다. 핸드오프 효과를 끌 수 있으며 아이콘은 OS의 모션 줄이기 설정도 따릅니다.
3. 설정을 닫고 **오른쪽 위 제어 상태 표시**를 누릅니다. 첫 요청에서는 **Autopilot**, **제어권 부여 전 확인 켜기**, **실행 유예 2초**로 설정하세요.

Autopilot은 정책 안에서 에이전트 실행을 허용합니다. **Co-pilot**에서는 제안된 명령마다 사람이 Enter를 누르며, **Observe**는 에이전트가 쓰지 않고 읽기만 하게 합니다.

## 에이전트 연결

**설정 → 에이전트 → 에이전트 연결**에서 진행합니다.

1. Codex / ChatGPT 로컬 작업, Claude Code, Cursor, GitHub Copilot(VS Code 또는 CLI)을 고릅니다.
2. **설정하기**를 눌러 MCP 서버와 협업 스킬을 등록합니다.
3. 카드의 안내대로 클라이언트를 재시작하거나 다시 연결합니다. Conn을 켜 두고 에이전트에게 현재 터미널을 읽어 달라고 요청하세요.

**설정 완료**는 파일 등록을 뜻하며 실제 접속하면 **현재 접속 중**으로 표시됩니다. 클라이언트의 신뢰 확인과 명령 승인은 별도로 유지됩니다. [클라이언트별 경로·갱신·해제·문제 해결](agent-integrations.ko.md).

앱에 포함된 CLI의 절대 경로와 현재 연결 주소를 사용합니다. Rust·Cargo·Node.js 설치나 PATH 변경은 필요 없으며 AppImage의 CLI는 유지되는 경로에 복사합니다. 앱을 옮기거나 재설치했다면 **설정 갱신**을 사용하세요. 기존에 수동으로 등록한 Conn 항목은 덮어쓰지 않고 검토하도록 안내합니다.

다른 클라이언트는 **다른 MCP 클라이언트 · 수동 설정**을 펼쳐 MCP JSON 또는 Codex TOML을 복사합니다. JSON은 설정 데이터이므로 셸 명령으로 실행하지 않습니다. 이 로컬 연결은 ChatGPT 웹·클라우드 작업용 원격 서버를 제공하지 않습니다.

### 직접 설정과 연결 주소

CLI를 별도로 설치해 `PATH`에 등록했다면 Codex CLI에서 다음 명령으로 연결할 수 있습니다.

```sh
codex mcp add conn -- conn mcp
```

기본 주소는 Unix의 `~/.conn/conn.sock`, Windows의 로컬 사용자별 Named Pipe입니다. **설정 → 진단**에서 현재 주소를 확인하세요. 기본값이 아닌 세션에는 `conn --socket <연결주소> mcp` 또는 `CONN_SOCKET`을 사용합니다. 데스크톱에서 복사하는 설정에는 이 값이 자동으로 포함됩니다. 브라우저 테스트 어댑터는 별도 주소를 사용합니다.

## 3. 제어권 주고받기

에이전트에게 요청하세요.

> 모든 셸 작업은 Conn으로 해줘. 현재 화면을 읽고 현재 디렉터리를 확인할 제어권을 요청해. 정확한 명령도 포함해 줘. 파일은 변경하지 마. 내가 거절하거나 제어권을 가져오면 멈추고 무슨 일이 있었는지 알려줘.

1. 제어 요청을 검토합니다. **요청 원문 보기**에는 인자와 에이전트가 전달한 예정 명령이 표시됩니다.
2. **허용**을 선택합니다. 별도의 명령 승인이 뜨면 그것도 검토하세요. 실행 유예 중 **Enter**는 즉시 실행하고 **Esc**는 취소합니다.
3. 결과를 함께 읽습니다. Enter가 전달됐다고 성공으로 판단하지 말고 에이전트가 스냅샷을 다시 확인하도록 요청하세요.
4. 직접 명령을 입력해 제어권을 가져와 보세요. 준비가 되면 에이전트에게 바뀐 화면을 읽고 계속하도록 요청합니다.
5. **타임라인**에서 명령과 제어권 변경을 확인합니다. 요청을 거절해 실행되지 않는지도 확인할 수 있습니다. 다시 시도할 때는 명시적으로 요청하세요.

**제어권을 가져와도 이미 실행 중인 명령을 되돌리거나 멈추지는 않습니다.** 에이전트의 추가 입력을 막는 동작입니다. 실행 중인 프로세스를 멈추려면 Ctrl-C 같은 일반적인 터미널 인터럽트를 사용하세요.

열린 C 아이콘은 사람 제어, 대기 중인 요청, 에이전트 제어, 실행 유예를 나타내는 상태 표시이며 승인 버튼은 아닙니다.

## 4. 타임라인 읽기

**전체**는 명령과 협업을 합쳐 보여주고 **명령**, **협업**으로 걸러 볼 수 있습니다. **상세**에서 이벤트 순서를, **요청 원문 보기**에서 기록한 메서드와 인자를 확인하세요. 오래된 기록이나 일부 클라이언트에는 예정 명령이 없을 수 있습니다.

거절, 정책 차단, 취소, 만료, 실행은 서로 다른 결과입니다. **실행됨**은 입력이 셸에 도착했다는 뜻이며 명령 성공을 뜻하지 않습니다. 타임라인은 터미널 출력이나 스크롤백을 저장하지 않습니다.

## 기존 터미널과 CLI 사용

[Linux x64](https://github.com/eggplantiny/conn/releases/download/v0.6.0/conn-v0.6.0-x86_64-unknown-linux-gnu-cli.tar.gz), [Mac Apple Silicon](https://github.com/eggplantiny/conn/releases/download/v0.6.0/conn-v0.6.0-aarch64-apple-darwin-cli.tar.gz), [Windows x64](https://github.com/eggplantiny/conn/releases/download/v0.6.0/conn-v0.6.0-x86_64-pc-windows-msvc-cli.zip)용 CLI 압축 파일을 받으세요. `conn`(Windows는 `conn.exe`)을 사용자 `PATH`에 포함된 폴더에 풀고 터미널을 다시 엽니다.

```sh
conn --version
conn
```

세션을 열어 두세요. 다른 터미널에서 예제 설정과 읽기 전용 요청을 보냅니다.

```sh
conn mode autopilot
conn gate --ask
conn pacing --enter-grace-ms 2000
conn agent --agent-id demo run "pwd" --reason "Show the current directory without changing files"
```

cmd.exe에는 `pwd` 대신 `cd`를 사용하세요. `conn take`는 제어권을 회수하고 `conn log -n 30`은 최근 기록을 읽습니다. `agent run`은 입력 후 스냅샷을 출력하며 셸 명령의 종료 상태를 추적하지 않습니다. 비-POSIX·원격 프로필에는 명령 검토가 필요합니다.

## 자주 묻는 문제

| 증상 | 확인할 것 |
|---|---|
| 에이전트가 CLI를 찾지 못함 | 설정에서 연결 구성을 다시 복사하세요. 데스크톱 연결은 절대 경로를 사용하므로 `PATH` 변경이 필요하지 않습니다. |
| 에이전트가 연결되지 않음 | Conn을 열어 두고 현재 주소를 사용하세요. 앱을 옮겼다면 설정을 다시 복사하세요. |
| 읽지만 입력하지 못함 | Observe 모드, 도구 권한, 제어권 소유자, 해당 탭이 보이는지 확인하세요. |
| 요청이 계속 대기함 | 제어 요청, 명령 승인, Co-pilot 제안이 있는지 확인하세요. |
| `unattended` 또는 `suspended` | 해당 탭으로 돌아가거나 명시적으로 맡기세요. 에이전트는 보이는 탭을 바꿀 수 없습니다. |
| 복합 명령이 차단됨 | `isolate_dangerous: true`이면 이동, 위험 작업, 검증을 각각 요청하세요. |
| 명령이 아직 실행 중임 | 기다린 뒤 스냅샷을 다시 읽으세요. 입력 결과는 종료 상태가 아닙니다. |

정책은 **설정 → 정책** 또는 `~/.conn/policy.yaml`에 있습니다. 승인으로 정책 차단을 해제할 수는 없습니다. 설정과 기록의 기본 위치는 `~/.conn`(Windows는 `%USERPROFILE%\.conn`)입니다. 요청과 프로필 환경 값에 민감한 내용이 있을 수 있으므로 공유 전에 확인하세요.

## 소스에서 실행

소스 빌드는 개발이나 직접 수정할 때 사용합니다. rustup으로 Rust와 네이티브 컴파일러·링커를 설치하세요. 데스크톱 빌드에는 **Node.js 24**와 [Tauri 필수 구성 요소](https://v2.tauri.app/start/prerequisites/)도 필요합니다. 저장소에서 Rust 도구 버전을 고정합니다.

```sh
git clone https://github.com/eggplantiny/conn.git
cd conn
cargo install --path crates/cli --locked
cd frontends/tauri
npm ci
npm run tauri dev
```

기존 터미널 프런트엔드는 Cargo 설치 후 `conn`을 실행하세요. 이 경로에는 Node.js와 Tauri가 필요하지 않습니다.

다음 안내: [백엔드](backends.md), [정책](policy.md), [신뢰 범위](security.md#한국어-요약), [기여](../CONTRIBUTING.ko.md).

## 업데이트 (v0.6.0 이상)

백그라운드에서 새 버전을 확인하고 서명된 업데이트를 다운로드합니다. 준비되면 Conn 메뉴에 **설치하고 다시 시작**이 표시됩니다. 설치 시점은 사용자가 결정하며, 실행 중인 명령을 포함한 모든 셸 세션이 종료된다는 안내를 확인합니다. 저장된 설정과 타임라인 기록은 유지됩니다.

**업데이트 확인 → 업데이트 설정**에서 자동 확인·다운로드를 끄거나 프리뷰 포함 여부를 바꿀 수 있습니다. 프리뷰 빌드는 기본적으로 프리뷰를 포함하고, 정식 빌드는 포함하지 않습니다. 채널을 바꿔도 이전 버전으로 내리지 않습니다. GitHub 계정이나 토큰은 필요 없습니다.

Apple Silicon macOS, Windows x64, Linux AppImage에서 앱 내 설치를 지원합니다. `.deb`는 패키지 관리자 또는 새 설치 파일을 사용하세요. 기존 0.5.x는 0.6.0을 한 번 직접 설치해야 합니다. 확인·다운로드 실패는 실행 중인 셸에 영향을 주지 않으며 다시 시도하거나 릴리스 페이지에서 받을 수 있습니다.
