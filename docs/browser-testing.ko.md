# Conn 웹 호스트와 브라우저 검증

[English](browser-testing.md) · [아키텍처](architecture.ko.md) · [경계 설계](web-harness-design.ko.md)

웹 프런트엔드는 모노레포의 `@conn/web` 패키지입니다. 네이티브 앱과 같은 `@conn/ui`의 ConnApp, 컴포넌트, 룬 상태, 터미널 큐와 협업 액션을 사용합니다. `conn-web`과 Tauri는 같은 Rust `AppRuntime`을 호출합니다. 호스트 전송, 네이티브 알림, 링크 열기와 업데이트 기능만 어댑터가 담당합니다.

## 로컬 실행

저장소 루트에서 고정된 Rust 도구 체인과 Node 24 이상으로 실행합니다.

```sh
npm ci
npm run dev:web
```

`http://127.0.0.1:1429`를 열고 서버가 출력한 비공개 연결 파일의 `bootstrapToken`을 입력합니다. 브라우저는 HttpOnly·SameSite 쿠키로 연결을 기억합니다. 토큰을 WebSocket 쿼리나 localStorage에 저장하지 않습니다. 개발 실행기는 `conn-web`과 같은 소스의 MCP 실행 파일 `conn`을 함께 빌드합니다.

기본 상태 폴더는 `~/.config/conn-web-dev`이며 `CONN_WEB_STATE`로 변경합니다. 프런트엔드·백엔드 포트는 1429·1430이며 `CONN_WEB_PORT`, `CONN_WEB_BACKEND_PORT`로 바꿉니다. `CONN_WEB_SETUP_HOME`은 실험용 에이전트 설정 파일의 저장 위치를 분리합니다.

MCP 클라이언트에는 같은 버전의 실행 파일과 연결 파일에 기록된 `agentSocket`을 사용합니다.

```sh
target/debug/conn --socket /path/to/state/conn.sock mcp --agent-id your-agent
```

여러 단계의 작업 동안 MCP 프로세스를 유지해야 합니다. 새 프로세스는 새 연결이며 다시 연결 승인을 받습니다. 소유자 연결 코드는 에이전트 접속 정보와 다릅니다.

## 빌드된 패키지 운영

[릴리스 다운로드](https://github.com/eggp-dev/conn/releases)에 Linux x64 독립 웹 압축 파일을 제공합니다. 압축을 풀고 아래 실행 명령을 사용하면 됩니다. 다른 대상이나 개발용 빌드에만 개발 도구가 필요합니다. 서버·MCP CLI·UI·소스 매니페스트·라이선스·체크섬을 함께 제공합니다.

```sh
npm run package:web
```

출력된 폴더에는 `conn-web`, 같은 빌드의 `conn`, `ui/`, 매니페스트, 체크섬과 시작 안내가 들어갑니다. Node와 Vite는 빌드할 때만 필요합니다.

```sh
./conn-web serve --state-dir /path/to/conn-state --port 1423
```

셸은 서버를 실행한 사용자 권한으로 동작합니다. 첫 구현은 루프백 연결만 지원합니다. 원격 운영을 확장할 어댑터 경계는 있지만 원격 인증·TLS·배포·다중 사용자 운영까지 구현된 것은 아닙니다.

## 수명과 제어

- PTY는 서버 프로세스가 소유합니다. 브라우저를 새로고침하거나 연결을 끊어도 실행 중인 셸을 유지합니다. 서버를 종료하면 셸도 끝나며 서버 재시작은 셸 복구가 아닙니다.
- 같은 셸 묶음에서 입력·크기를 결정하는 사람의 화면은 하나입니다. 두 번째 화면에서 명시적으로 이어가면 이전 화면의 입력이 중지됩니다. 에이전트 권한은 부여하지 않습니다.
- 화면은 체크포인트를 먼저 파싱하고 순번이 붙은 출력을 이어받습니다. 복원이 끝나기 전에는 입력할 수 없습니다. 출력이 누락되면 다시 복원하며 사용자 입력을 재전송하지 않습니다.
- 입력·터미널 응답·크기 변경은 두 클라이언트와 Rust 소유자에서 같은 세션별 순서를 지킵니다. 느린 다른 작업은 터미널 전달을 직렬화하지 않습니다.
- 연결 승인·선택한 세션 공유·제어권·명령 승인은 구분합니다. 같은 이름의 새 에이전트 연결이 이전 권한을 상속하지 않습니다.

## 검증 재현

```sh
cargo test --locked
cargo build --locked -p conn-web -p conn
npm run build:web
npm run test:ui
npm run test:screen -w @conn/ui
npm run test:checkpoint -w @conn/ui
npm run test:app-lifetime -w @conn/ui
npm run test:collaboration
node scripts/generate-owner-contract.mjs --check
node scripts/check-ui-boundaries.mjs
```

Chromium이 없으면 먼저 `npm exec -w @conn/collaboration-tests -- playwright install chromium`을 실행합니다. 협업 검사는 임시 포트에서 프로덕션 웹 번들과 실제 Rust 백엔드를 실행하고, 지속 연결한 실제 MCP 프로세스와 소유자 화면의 클릭·키 입력으로 진행합니다. 앱 내부 상태를 가져오거나 승인 화면을 합성하지 않습니다. 증거는 출력된 임시 폴더에 저장합니다.

앱 수명 검사는 실제 ConnApp을 통제된 호스트 포트와 함께 여러 개 마운트해 격리·실패·정리를 확인하는 별도 검사입니다. 실제 셸이나 네이티브 IPC의 증거는 아닙니다. 파서·체크포인트 검사는 실제 Rust 모델과 설치된 xterm을 비교하지만 모든 터미널 확장의 호환성을 의미하지는 않습니다.

## SSH 입력·커서·승인 회귀 검사

```sh
npm run test:input -w @conn/collaboration-tests
```

기본값은 로컬 Bash PTY입니다. 폐기 가능한 SSH 픽스처를 시험하려면 `CONN_AUTH_FIXTURE_RUNTIME`을 `.runtime` 폴더로 지정합니다. 합성 암호와 고정한 호스트 키를 사용하는 픽스처가 `127.0.0.1:22222`에서 실행 중이어야 합니다. 인증도 화면에 보이는 PTY로 입력하며 실제 MCP·프로덕션 UI·Rust 호스트를 사용합니다. 화면 행·커서 비교와 스크린샷은 저장소 밖에 남깁니다.

macOS 등에서 계정을 새로 만들지 않고 시험하려면 폐기 가능한 키 인증 OpenSSH 픽스처의 클라이언트 설정을 지정할 수 있습니다.

```sh
CONN_SSH_FIXTURE_CONFIG=/path/to/disposable/client.conf \
  npm run test:input -w @conn/collaboration-tests
```

이 설정에는 `validation-host` 별칭, 루프백 주소, 시험 포트, 폐기 가능한 키와 고정한 호스트 키가 있어야 합니다. 원격 셸은 `remote$` 프롬프트를 제공해야 합니다. 스크립트가 `ssh -F <config> validation-host`를 화면으로 실행하며, 종료 후 로컬 셸 복귀까지 검사합니다. 실제 계정의 키·SSH 설정을 사용하거나 바꾸지 마세요. 설정 경로에는 영문·숫자·`_./-`만 허용합니다. 두 픽스처 변수를 함께 지정하면 키 인증 설정을 우선합니다.

브라우저 결과는 실행한 운영체제의 Chromium 증거입니다. macOS WebKit·네이티브 운영체제 알림·설치·배포 완료를 입증하지 않습니다. 이번 [공통 런타임 macOS 실기 결과](shared-runtime-macos-validation-results.ko.md)와 이전 [SSH 입력 검증 기록](ssh-input-review-results.ko.md)은 별도로 구분합니다.


같은 SSH 픽스처에 복합 명령으로 진입하려면 입력 검사 명령에 `CONN_SSH_COMPOUND_ENTRY=1`을 추가한다. 한글 검사는 원격 UTF-8 로케일(예: `LC_ALL=C.UTF-8`)이 필요하다. 사용자 서버가 아닌 폐기 가능한 픽스처에만 설정한다.

빌드된 배포 파일 자체를 검사하려면 `CONN_TEST_WEB_BUNDLE=/path/to/conn-web-0.8.7-linux-x64 npm run test:collaboration`을 실행한다. 구성 요소를 다시 빌드하지 않으며, 별도 상태 디렉터리에서 묶음의 실제 UI·서버·지속 MCP CLI를 사용한다.

## 실제 Vim 편집 회귀 검사

위 Rust 실행 파일과 웹 UI를 빌드한 뒤 실행합니다.

```sh
npm run test:editor -w @conn/collaboration-tests
CONN_EDITOR_BROWSER=webkit npm run test:editor -w @conn/collaboration-tests
```

필요하면 고정된 Playwright CLI로 WebKit과 호스트 의존성을 설치합니다. 실제 `vi` 실행 파일(`vi -Nu NONE -n -i NONE`), 격리된 임시 파일, 프로덕션 UI와 Rust PTY를 사용합니다. 비공유·공유 상태의 편집, 사람 개입, 커서 편집, 정확한 저장 내용, 편집 중 화면 재연결, 100줄 파일의 창 크기 변경 후 편집을 검사합니다. Vim을 실행한 뒤 타이핑 전에 터미널 포커스를 강제로 복구하지 않아 포커스 상실도 검출합니다. `CONN_SSH_FIXTURE_CONFIG`와 `CONN_SSH_COMPOUND_ENTRY`는 위의 폐기 가능한 SSH 픽스처를 선택하며, 그 안에 Vim 호환 `vi`가 설치되어 있어야 합니다. 운영 호스트를 대상으로 실행하지 마세요.

`CONN_EDITOR_OUTPUT`으로 증거 폴더를 지정합니다. 예상하지 못한 브라우저 오류는 실패 처리합니다. 이전에도 관찰된 `ResizeObserver loop completed with undelivered notifications.` 경고는 별도로 집계해 `runtime.json`에 남깁니다. 편집 성공을 그 경고가 무해하다는 근거로 해석하지 않습니다. Linux WebKit을 포함한 브라우저 결과는 macOS 네이티브 키보드·IME·IPC 동작을 입증하지 않습니다. 미해결 네이티브 제보는 [0.8.7 Vim 조사 기록](vim-input-investigation.ko.md)을 참고하세요.
