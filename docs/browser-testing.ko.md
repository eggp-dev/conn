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

브라우저 결과는 Linux Chromium 증거입니다. macOS WebKit·네이티브 운영체제 알림·설치·배포 완료를 입증하지 않습니다. 이전 네이티브 결과는 [SSH 입력 검증 기록](ssh-input-review-results.ko.md)에 따로 남겨 두었습니다.
