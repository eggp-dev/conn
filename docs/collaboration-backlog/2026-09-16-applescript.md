# COLLAB-002 · 맥북 개발 빌드의 AppleScript 실험

## 목적과 환경

- 날짜: 2026-09-16, KST.
- 목표: 개발 코드를 dev로 전달하고 실제 Apple Event를 통해 private 창 생성과 입력 전달을 검증한다.
- 소스: `69a4dd9` (`dev`). Linux에서 Rust 196개, UI 테스트·빌드, 릴리스 도구 테스트 48개, 소스 공개 적합성 검사를 통과한 변경이다.
- 조작 경로: Linux Conn의 지속형 MCP → Tailscale SSH → Mac 터미널 → `osascript` → Mac Conn Dev.
- Mac 빌드: `npm ci` 후 Tauri debug app bundle, ad-hoc 서명. 테스트 이름 `Conn Dev`, 식별자 `dev.eggp.conn.devtest`. 배포용 공증 빌드가 아니다.
- 사용자가 지정한 프로젝트 내부 `.local-setup/apple-script`를 별도 설정·소켓 위치로 사용했다. 자동화는 사용자가 앱 설정에서 로컬 프로필을 선택하고 켰다.
- 실명 계정·주소·원본 터미널 기록은 이 문서에 보관하지 않는다. 테스트에는 가짜 표식만 사용했다.

## 실제 진행과 관찰

| ID | 행동·사람의 개입 | 관찰 결과 | 근거 |
|---|---|---|---|
| O1 | 깨끗한 맥북 dev 체크아웃에서 fast-forward pull 후 빌드 | 69a4dd9 수신, Conn Dev.app 빌드·ad-hoc 서명 성공 | Git·빌드 출력 |
| O2 | `check_macos_scripting.py` 실행 | `sdef`가 전체 Xcode를 요구해 중단. 활성 개발 경로는 Command Line Tools | 오류 출력; 앱 동작 실패와 구분 |
| O3 | 별도 설정으로 앱 실행, 사람이 Automation 활성화 | 저장된 permission config가 enabled이며 로컬 프로필 허용 | 사용자 응답·설정 메타데이터 |
| O4 | 앱의 절대 경로를 대상으로 `create window with default profile command` 호출 | session status에 `externalPrivate: true`, `processAlive: true`, `inputAvailable: true` 반환 | 실제 osascript 결과 |
| O5 | `/bin/sh -c`로 `CONN_APPLESCRIPT_SMOKE`와 Enter 안내 출력 | 사용자가 두 문구가 새 창에 보임을 확인 | 사용자 직접 확인 |
| O6 | 별도 새 창에서 같은 osascript 프로세스가 `write text`, 상태 polling, `release session` 수행 | `delivered`, `error: null`과 요청·세션 ID만 반환. 응답에 입력 원문 없음 | 실제 상태 JSON |
| O7 | 자식 셸이 읽은 입력을 가짜 표식과 비교한 경우에만 결과 파일 작성 | `write-result.txt`에 `CONN_WRITE_OK` 확인. 단순 전달 확인보다 강한 자식 수신 근거 | 표식 파일 읽기 |
| O8 | 개발 앱 전용 소켓에 CLI `sessions` 요청 | 두 private 세션이 공개 목록에 나오지 않음 | 공개 IPC 출력 없음 |
| O9 | 테스트 설정 디렉터리 목록 확인 | automation.json, policy.yaml, conn.sock, 의도한 결과 표식만 존재. audit.jsonl 없음 | 파일 목록 |

## 결과와 한계

- 실제 AppleScript 창 생성, 명령 시작, 입력 전달, 상태 조회, writer 해제 흐름을 확인했다.
- 테스트 창은 사람에게 남겼다. 첫 창은 Enter 대기, 두 번째도 후속 Enter 대기이며 writer는 해제했다. 사람의 Enter 이후 종료 화면 유지까지는 확인하지 않았다.
- 새 설정으로 시작했으므로 기존 설정 마이그레이션은 검증하지 않았다.
- 전체 Xcode 부재로 `sdef` 기반 사전 검사 전체는 미완료다. 실제 osascript 구문 해석·호출 성공과 별도로 남긴다.
- cold launch, 다른 sender의 접근 거절, 인간 takeover 후 늦은 쓰기 차단, 서명·공증된 배포 앱의 동의 흐름은 이번 세션에서 미검증이다.
- 공개 세션 목록과 별도 설정 폴더 확인에 한정한다. 명시적 세션 ID를 이용한 snapshot 거절, WebView 저장소, 시스템 로그까지 전수 검사한 결과는 아니다.
- 테스트를 보내는 Linux의 일반 협업 세션에는 가짜 표식이 포함된 명령이 보인다. Mac private 세션의 기록 제외와 호출자·상위 터미널의 기록은 다른 경계다. 실제 비밀정보로 이 테스트 명령을 대체하면 안 된다.

## 후속 후보

- TEST-C001: 전체 Xcode가 있는 환경에서 scripting 검사 재실행.
- TEST-C002: signed release candidate에서 cold launch·sender 소유권·takeover·종료 동작 검증.
- DOC-C001: private 입력 대상과 호출하는 일반 터미널의 기록 경계를 실험 안내에서 명확히 구분한다.

## 재검증

아직 없음.
