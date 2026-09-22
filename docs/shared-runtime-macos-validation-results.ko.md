# 공통 UI·런타임 macOS 실기 검증

2026-09-22~23 · 버전 `0.8.6` · 개발 빌드 검증, 미배포

**기준 소스에서 macOS PTY 교착을 재현했고 수정 후 같은 네이티브 입력 경로와 실제 SSH 경로를 재검증했다. 확인한 기능 경로는 PASS다.** 배포 서명·공증·업데이트 산출물, 사용자 고유 SSH 게이트웨이와 물리 한글 IME 조합은 이 결과에 포함하지 않는다.

## 소스와 격리

- 저장소: `eggp-dev/conn`, 브랜치: `codex/shared-runtime-macos-validation`.
- 기준 커밋: `7fa572385f92740ab2107f23e7fcfc081b56a79f`.
- 런타임 수정 커밋: `1d30c87d69b6231a9b3c37c171bebc7038f6b1a4`. 최종 앱과 Rust 호스트는 이 커밋과 같은 런타임 소스로 빌드했다. 후속 변경은 검증 스크립트·문서·증거다.
- 별도 `conn-runtime-validation` worktree를 만들었다. 기존 두 checkout, 설치된 `/Applications/Conn.app`, 기본 Conn 설정·소켓·기존 MCP 프로세스는 보존했다. 프로젝트/상위 경로에 적용되는 `AGENTS.md`는 없었다.
- macOS 27.0, Rust 1.95.0, Node 24.13.0. 장치 이름·계정·개인 경로는 공개 기록에서 제외했다.
- 별도 제품명 **Conn Runtime Validation**, 번들 식별자 `dev.eggp.conn.runtimevalidation.7fa5723`, 독립 `CONN_CONFIG_DIR`·`CONN_SOCKET`으로 실행했다. 시험용 웹 호스트도 별도 설정·소켓을 사용했다.
- 실제 실행 파일은 worktree 기준 `frontends/tauri/src-tauri/target/debug/bundle/macos/Conn Runtime Validation.app/Contents/MacOS/conn-desktop`이다. MCP는 같은 번들의 `Contents/MacOS/conn`을 사용했다. 절대 경로와 바이너리 SHA-256은 저장소 밖 실행 기록에 보관했다.
- `npm ci` 후 Rust 데스크톱과 앱 번들을 빌드했다. 외부 Tauri 설정으로 제품명·식별자를 바꾸고 `--debug --bundles app`, `APPLE_SIGNING_IDENTITY=-`를 사용했다. 설치용 배포 빌드가 아니다.

실기 조작은 Computer Use의 네이티브 앱·브라우저 UI로 수행했다. MCP는 실제 CLI의 stdio 프로세스를 여러 단계 동안 유지했다. 소유자 승인에 내부 IPC를 대신 사용하지 않았다. 네이티브와 웹은 같은 Bash 프로필, 기본 정책, autopilot, 승인/lease 시간과 합성 명령을 사용했다. 두 프로세스가 PTY를 자동 공유한다고 가정하지 않았다.

## 재현한 결함과 수정

**FAIL → 수정 → PASS: 긴 입력 중 PTY 교착.** 기준 앱에서 약 3KB의 `printf` 명령을 `terminal_type`으로 보내자 응답과 다음 입력이 멈췄다. 실제 Rust 웹 호스트의 지속 MCP 검사도 같은 길이의 입력에서 멈췄다. 프로세스 스택에서는 쓰기가 세션 잠금을 잡은 채 PTY 버퍼를 기다리고, PTY 출력 리더는 같은 잠금을 기다렸다. macOS PTY에서 readline echo가 양방향 버퍼를 채우며 생기는 대기였다.

`crates/core/src/engine.rs`에서 PTY 읽기와 세션 반영을 분리했다. 리더는 계속 읽고 FIFO 소비자 하나가 바이트 순서대로 세션에 반영한다. 이미 도착한 작은 조각만 묶어 처리하며 추가 지연을 넣지 않는다. 출력 완료 표시는 마지막 read가 아니라 소비자의 반영 완료 뒤에 세운다. 권한·정책·입력 순서나 불확실한 입력의 재전송 규칙은 바꾸지 않았다.

교착을 다시 만들지 않도록 채널의 send는 용량 때문에 대기하지 않는다. 따라서 소비자가 장시간 밀리면 큐 메모리가 늘 수 있다. 이번 입력·출력 시나리오의 통과를 무제한 출력 부하 검증으로 해석하면 안 된다.

회귀 검사 `long_agent_input_drains_pty_echo_without_deadlock`, `long_human_paste_drains_pty_echo_without_deadlock`를 추가했다. 각각 3KB 입력 쓰기가 제한 시간 안에 끝나고 전체 출력과 다음 명령까지 이어지는지 검사한다. 실패 시 독립 경로로 시험 셸을 종료하여 테스트 자체가 교착에 갇히지 않게 했다.

실제 앱 재검증은 다음을 포함했다.

- 로컬 MCP: 긴 명령 완료 후 `AFTER_LONG_INPUT_OK`. [수정 직후 화면](validation/shared-runtime-macos/native-08-long-input-fixed.jpg). 이 이미지는 출력 FIFO 수정 직후이며, 뒤에 작은 출력 조각 합치기를 추가했다.
- 최종 네이티브 + 실제 SSH: 3,029자 명령 뒤 `FINAL_AFTER_LONG_OK`, `한글 FINAL_LINE_TWO`, `remote$`가 각각 올바른 줄에 표시됐다. [최종 SSH 화면](validation/shared-runtime-macos/native-14-final-long-ssh.jpg).
- 최종 네이티브 로컬 사람 입력: Computer Use로 3KB를 붙여넣고 Enter, 이어서 한글을 포함한 여러 줄을 붙여넣었다. `LOCAL_AFTER_LONG_OK`, `한글 LOCAL_TWO`, `LOCAL_THREE`와 프롬프트를 확인했다. [창 크기 복원 후 화면](validation/shared-runtime-macos/native-17-local-human-long-restored.jpg).

[기준 앱 정지 당시 화면](validation/shared-runtime-macos/native-07-long-input-stall.jpg)만으로 교착을 판단하지 않았다. 응답 정지, 스택과 실제 호스트 재현을 함께 사용했다. 원본 스택·인증 로그는 공개하지 않는다.

검증 스크립트도 두 곳을 수정했다. 설정 단축키를 macOS에서도 동작하는 `ControlOrMeta`로 바꾸고, 새 OS 계정이나 실제 개인 키 없이 OpenSSH를 시험하는 `CONN_SSH_FIXTURE_CONFIG` 경로를 추가했다. 기존 합성 암호 픽스처 경로는 유지한다.

## 실제 화면·협업 결과

| 항목 | 판정과 관찰 |
| --- | --- |
| 네이티브 빌드·실행 | **PASS.** 실제 Tauri Rust 데스크톱과 별도 `.app` 실행. 설치 앱과 다른 경로·제품명으로 확인했다. |
| 고정 창의 승인 패널 | **PASS.** 1120×720 창에서 위험 명령 검토를 반복하고 거절했다. 터미널은 137열을 유지하고, 열린 패널에서 30행, 닫힌 뒤 41행으로 복원됐다. [열린 화면](validation/shared-runtime-macos/native-11-panel-review.jpg), [복원·후속 명령](validation/shared-runtime-macos/native-13-restored-after-review.jpg). 캡처 진단 중 별도 요청 하나는 만료됐으며 거절 성공으로 세지 않았다. |
| 긴 입력·줄끝·한글·붙여넣기 | **PASS, 확인한 범위.** 로컬·SSH에서 긴 영문과 한글 혼합을 입력하고 줄끝 이동·삭제·대체 후 `END_Y`를 확인했다. [로컬 편집 결과](validation/shared-runtime-macos/native-06-same-name-new-admission.jpg), [SSH 편집 결과](validation/shared-runtime-macos/native-09-ssh-unicode-edited.jpg). 확정 문자열과 붙여넣기 검사이며 물리 IME 조합 검사와 구분한다. |
| 줄바꿈·커서·다음 출력 | **PASS, 확보한 현재 화면.** 긴 행이 오른쪽까지 사용되고 여러 줄 출력 뒤 새 프롬프트가 분리된다. MCP 그리드·커서와 화면을 대조했다. 실제 Chromium에서는 전체 행/커서 자동 비교도 수행했다. 네이티브의 모든 프레임을 픽셀 단위로 비교한 것은 아니다. |
| 연결 승인과 공유 | **PASS.** 연결을 허용해도 비공유 셸 목록은 0개였다. 사람의 선택 공유 후 세션이 보였으나 제어권 없는 쓰기는 `not_controller`였다. |
| 제어권과 명령 승인 | **PASS.** 별도 제어 요청을 UI에서 승인했다. 공유 직후 첫 일반 명령은 환경 검토가 필요했고, 승인 후 다음 일반 명령은 같은 MCP 프로세스에서 추가 연결 승인 없이 실행됐다. [첫 명령 검토](validation/shared-runtime-macos/native-03-initial-review.jpg). |
| 위험 명령과 사람 개입 | **PASS.** 시험 경로의 `rm -rf`는 검토 카드로 보냈고 거절했다. 거절 후 입력을 취소하고 다음 일반 명령이 실행됐다. 사람이 커서를 편집하면 에이전트 쓰기와 Enter 모두 `not_controller`; 명시적 제어 반환도 성공했다. |
| 같은 이름의 새 프로세스 | **PASS.** 별도 MCP 프로세스를 같은 표시 이름으로 연결하자 새 연결 승인 카드가 나타났다. 기존 권한을 이어받지 않았으며 새 요청은 거절했다. [화면](validation/shared-runtime-macos/native-06-same-name-new-admission.jpg). |
| 에이전트 새 탭·알림 | **PASS.** 새 탭에 에이전트 이름·요청 배지와 `Needs you`가 표시되고 사람의 기존 SSH 탭은 유지됐다. 앱 내부 알림도 확인했다. [네이티브 화면](validation/shared-runtime-macos/native-15-background-agent-tab.jpg). 웹에서도 같은 흐름을 확인했다. |
| 웹 새로고침·화면 인계 | **PASS.** 실제 화면에서 설정한 `CONN_RETAINED=web_retained_42`가 새로고침과 `Continue here` 인계 뒤 유지됐다. 이전 화면은 `Open in another view`로 입력이 차단됐다. [인계 뒤 출력](validation/shared-runtime-macos/web-04-handoff-retained.jpg). |
| 입력 실패·화면 복구 | **PASS, 검사별 범위.** 실제 웹/Rust/MCP 검사는 첫 입력 응답의 `owner_busy` 주입 후 명시적 재연결, 기존 PTY 유지, 불확실한 입력 재전송 없음, 새 입력 순서 회복을 확인했다. 체크포인트 실패·소켓 종료 경합은 공통 UI와 앱 수명 검사로 확인했다. 실제 네이티브 IPC에 고장을 주입한 결과는 아니다. |
| 웹 서버 수명 | **PASS.** 시험 서버 종료 시 기존 Bash PTY 두 개도 종료됐음을 확인했다. 같은 설정·포트로 재시작하여 새 연결 코드로 접속하자 `RESTART_unset`인 새 셸이 생겼다. [재시작 화면](validation/shared-runtime-macos/web-06-host-restarted.jpg). 서버 재시작 후 셸 복구 기능은 아니다. |

### SSH와 플랫폼 차이

실제 `/usr/sbin/sshd`를 루프백 시험 포트에서 실행했다. 폐기 가능한 호스트 키·클라이언트 키, 고정한 known-hosts, 별도 `ssh -F` 설정과 원격 Bash를 사용했다. 개인 SSH 키, 에이전트 소켓과 기존 SSH 설정은 사용하지 않았다. 암호 인증·PAM·포트 포워딩도 사용하지 않았다. 호스트의 기존 실행 계정 권한에서 돌린 시험이며 별도 원격 사용자나 사용자 게이트웨이 환경은 아니다.

네이티브에서도 SSH `exit` 뒤 로컬 `validation$`로 복귀하고 `NATIVE_LOCAL_RETURN_OK`를 실행했다. 같은 로컬 PTY가 살아 있고 사람 제어 상태임을 MCP 그리드로 확인했다. 시험을 마친 뒤 이번에 시작한 MCP·웹 호스트·sshd·화면 유지 프로세스를 종료했다. 검증 앱은 사람 제어의 로컬 셸로 남겼다.

처음에는 `printf ...; ssh ...`처럼 복합 명령으로 진입하여 추가 환경 검토가 나타났다. 같은 설정에서 단독 `ssh ...`로 다시 진입하자 안전 명령 자동 실행과 위험 명령 검토를 확인했다. 코어는 단일 SSH 명령의 셸 수명만 SSH 전송으로 추적한다(`Session::shell_integration_event`, `remote_policy`). 이 차이는 셸 상태/명령 분석 경계이며 네이티브·웹 UI 분기나 사적 셸이라는 추정으로 설명하지 않았다.

네이티브는 Tauri IPC·WebKit, 웹은 WebSocket·브라우저 인증과 명시적 화면 인계를 사용한다. 공유 화면·승인 카드는 공통 UI이고 권한·정책은 공통 런타임/코어에 있다. 이번 동일 조건 비교에서 이를 벗어나는 정책 차이는 찾지 못했다. 네이티브 운영체제 알림 센터의 전달·클릭, 업데이트·외부 링크 기능은 별도 플랫폼 어댑터 실기 범위다.

### 화면 캡처 제한

화면 잠금/백그라운드 구간에 macOS 캡처 오류와 과거 프레임이 발생했다. WebKit 콘솔에서 `document.visibilityState === "hidden"`과 `ResizeObserver loop completed with undelivered notifications`도 관찰했다. AX 상태와 픽셀이 다른 캡처는 PASS 근거에서 제외했다. 창 메뉴로 창을 다시 올린 뒤 패널 복원 화면을 확보했고, 마지막 로컬 붙여넣기는 창을 확대했다가 원래 1120×720으로 복원한 후 현재 출력을 확보했다.

이 관찰로 앱의 전경 렌더링 결함이 확정됐다고 판단하지 않았다. 반대로 백그라운드에서 매 프레임 즉시 그려진다고 보증하지도 않는다. `native-10`, `native-12`, `native-16` 원본은 진단 자료로만 보관하며 공개 PASS 증거에 넣지 않았다. 별도 크기 변경으로 확보한 마지막 로컬 화면은 고정 창 패널 반복 검사의 증거로 사용하지 않았다.

## macOS에서 다시 실행한 자동 검사

| 검사 | 결과 |
| --- | --- |
| `cargo test --locked` | **274 PASS, 1 ignored**. 기준 272개에서 교착 회귀 2개 추가 |
| Tauri manifest의 `cargo test --locked` | **4 PASS, 1 ignored**. ignored는 최종 릴리스 서명 산출물 필요 |
| `npm run test:ui` | **65 PASS** |
| 실제 Rust·프로덕션 웹·지속 MCP | **8 PASS** |
| 로컬 입력·렌더링 | **23 PASS** |
| 실제 루프백 OpenSSH 입력·렌더링 | **24 PASS**, 로컬 셸 복귀 포함 |
| 코어/xterm 그리드·커서 | **85 PASS** |
| 실제 xterm 체크포인트 복원 | **984 PASS** |
| 앱 수명·격리·실패 처리 | **PASS** |
| 네이티브 앱 / 웹 UI 빌드 | **PASS / PASS** |
| 생성된 owner 계약·공통 UI import 경계 | **PASS** |

웹 자동 검사는 이 macOS에서 실행한 Chromium 결과다. 네이티브 Computer Use 증거와 합산하여 같은 종류의 테스트 수로 표현하지 않았다. 기존 프런트엔드 경고와 npm 의존성의 low 등급 항목 2개는 이번 수정 대상이 아니다.

## 판정과 남은 범위

- **FAIL 해결:** 기준 커밋의 3KB 입력 교착. 공통 코어 수정과 사람/에이전트 회귀 검사, 네이티브 로컬·SSH 재실행으로 확인했다.
- **PASS:** 위 표에 명시한 개발 빌드·실제 화면·지속 MCP·루프백 SSH·웹 수명 경로.
- **BLOCKED / 범위 밖:** 사용자 고유 원격 게이트웨이·인증 환경은 시험 접속 조건이 없어 검증하지 않았다. 물리 한글 IME 조합, 네이티브 알림 센터, 실제 네이티브 IPC 장애 주입도 수행하지 않았다. 자동 검사나 문자열 붙여넣기로 해당 검증을 대신했다고 표시하지 않는다.
- **배포 판정 없음:** 배포 서명·공증·업데이트 최종 산출물은 검증하지 않았다. 원격 웹 인증·TLS·다중 사용자 운영, 별도 데스크톱과 웹의 자동 PTY 공유, 서버 재시작 복구도 이번 구현의 기능이 아니다. 메인 병합·태그·배포를 수행하지 않았다.

공개 [요약 증거와 이미지 SHA-256](validation/shared-runtime-macos/evidence.json)은 계정·장치 식별 정보, 실제 키·연결 토큰, 원본 인증 로그를 포함하지 않는다. 이미지는 Computer Use가 반환한 원본 JPEG 바이트이며 내용을 수정하지 않았다. 전체 명령 응답·빌드 로그·인증 픽스처·절대 실행 경로는 저장소 밖 격리된 기록에 보관했다.
