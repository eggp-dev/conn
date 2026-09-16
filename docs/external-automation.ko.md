# 외부 앱의 터미널 자동화

[English / 전체 API 계약](external-automation.md)

**v0.5.0의 실험적 기능이며 기본값은 꺼짐입니다.** 첫 네이티브 어댑터는 macOS
AppleScript입니다. Windows/Linux는 공통 내부 계약을 공유하지만 외부 실행 어댑터는
아직 없습니다. 실제 PAM 호환성과 macOS 권한 동작은 Mac에서 검증해야 합니다.

## PAM의 새 창 명령 (v0.5.1)

사진과 같은 iTerm 형태의 **한 줄 새 창 생성 명령**을 지원하는 구현을 추가했습니다.
v0.5.1부터 사용할 수 있습니다.

```applescript
tell application "Conn"
    create window with default profile command "/bin/sh -c '__RUN_COMMAND__ ; echo \"Press [Enter] key to exit.\"; read ANSWER;'"
end tell
```

[복사할 전체 템플릿](../examples/applescript/pam-window.applescript)

- `__RUN_COMMAND__`는 PAM이 실제 접속 명령으로 치환해야 합니다. Conn이 치환하지 않습니다.
  스크립트 편집기에서 테스트할 때는 `pwd`로 바꿉니다.
- 설정 → 자동화에서 **기본 프로필**을 허용해야 합니다. 새 네이티브 창에 그 프로필의
  셸을 열고, 지정 문자열을 기존 제어 승인·명령 정책·실행 유예 경로로 전달합니다.
- 명령을 생략하면 기본 프로필의 새 창만 만듭니다. 기존 창과 탭은 유지합니다.
- 창의 UI가 준비되기 전에는 입력하지 않습니다. 명령이 전달되면 자동화 제어권을
  반환하므로 사람이 접속과 상호작용할 수 있습니다. 반환은 명령 완료나 접속 성공을 뜻하지 않습니다.
- 안내 문구와 Enter 대기는 명령 안의 `echo`·`read`가 수행합니다. Enter 이후에는
  기본 프로필의 셸로 돌아가며, Conn 창을 자동으로 닫지는 않습니다.
- 각 창은 자기 탭·출력·설정 대상 세션을 갖습니다. 한 창을 닫아도 다른 창의 셸은 유지됩니다.

호환 범위는 이 생성 명령의 문법입니다. 결과는 불투명한 세션 ID이며 iTerm의
`window` 객체가 아닙니다. `tell current session`, 분할 패널, iTerm의 전체 객체 모델은
지원하지 않습니다. 기존 `create session`·`write text`·상태 조회 API도 유지됩니다.
[원래 iTerm 명령 안내](https://iterm2.com/documentation-scripting.html)

## 사용 흐름

PAM이나 외부 런처가 AppleScript로 Conn을 실행하고, 저장된 프로필로 탭을 만든 뒤
그 세션에 접속 명령을 전달합니다. 이미 실행 중인 앱에서도 같은 방식으로 동작합니다.

1. **설정 → 자동화**에서 사용할 프로필을 선택하고 AppleScript를 켭니다.
2. 외부 앱의 스크립트 템플릿에 [예제](../examples/applescript/pam-connect.applescript)를
   적용합니다. `osascript pam-connect.applescript`는 `pwd`만 요청합니다.
   인자로 SSH 접속 명령 하나를 전달할 수 있습니다. 기본 프로필도 허용 목록에 있어야 합니다.
3. macOS의 앱 제어 권한 요청과 Conn의 세션 제어 요청을 승인합니다.
   제어권 승인 뒤에도 명령 정책과 Co-pilot 제안 수락은 적용됩니다.
4. 예제는 입력 전달 상태를 확인하고 제어권을 반환합니다. 탭은 사람이 사용할 수 있도록 유지됩니다.

**생성·입력·상태 조회·반환은 하나의 발신 프로세스에서 실행해야 합니다.**
`osascript`를 명령마다 따로 실행하면 소유자가 달라집니다. PAM이 osascript를 실행하면
표시되는 발신자는 중간 실행기이며, PAM 업체를 인증했다고 간주하지 않습니다.
설정은 로컬 호출자의 프로필 접근을 허용하며 새 세션마다 제어권 승인을 받습니다.

```applescript
tell application "Conn"
    activate
    set sessionID to create session
    set requestID to write text "pwd" to session sessionID intent "현재 폴더 확인"
    -- 전체 예제처럼 request state로 완료·거절·시간 초과를 확인합니다.
end tell
```

Conn 전용 사전입니다. 위에 명시한 새 창 명령 외에 iTerm/Terminal의 전체 스크립트가
호환되는 것은 아닙니다. 실제 PAM의 템플릿에서 사용하는 명령을 확인해야 합니다.

## 제공 명령

| 명령 | 의미 |
| --- | --- |
| `create session [profile "프로필-ID"]` | 새 탭 생성·선택 후 세션 ID 반환 |
| `write text "명령" to session id` | 입력을 큐에 넣고 요청 ID 반환. 기본값은 정책을 거쳐 Return 전달 |
| `request state requestID` | 요청 상태를 문자열로 조회 |
| `request status requestID` | 상태와 세부 결과를 JSON으로 조회 |
| `session status sessionID` | 모드·실행 여부·현재 보고 있는 탭인지 조회 |
| `cancel request requestID` | 대기 요청과 해당 자동화 세션의 후속 입력 취소 |
| `release session sessionID` | 자동화 접근 해제. 터미널은 유지 |

`write text`에는 `newline false`, `intent "사유"`, `request id "고유-ID"`를
추가할 수 있습니다. 동일 앱 실행에서 같은 요청 ID·내용을 다시 보내면 기존 요청을
반환합니다. 다른 내용에 같은 ID를 사용하면 거부합니다.

`delivered`는 입력 처리 결과이며 셸 명령 성공이나 SSH 인증 완료가 아닙니다.
Co-pilot에서 `newline false`로 보낸 입력은 제안으로 남으므로 상세 결과의
`proposed`와 `inputDelivered`를 확인합니다. SSH 명령 뒤에 후속 명령을 무작정
연속 입력하면 안 됩니다. 첫 어댑터는 터미널 출력 읽기를 제공하지 않습니다.

한 요청은 제어 문자가 없는 한 줄·최대 16 KiB입니다. Return은 newline 옵션으로
보냅니다. 세션별 대기 16건, 동시 자동화 세션 32개, 앱 실행당 보존 요청 256건,
큐 대기를 포함한 요청 기한 120초로 제한합니다. 이미 전달된 입력은 취소로 되돌릴 수
없으며, 앱 충돌이나 결과가 불분명한 시간 초과 뒤에 자동 재실행하지 않습니다.

## 권한과 기록

- 기존 엔진·정책·UI·타임라인을 공유합니다. 외부 입력은 전용 연결로 승인 경로를
  거치며 사람의 키보드 입력으로 처리하지 않습니다.
- 생성할 때 받은 세션에만 입력합니다. 다른 호출자의 세션이나 기존 사람 탭은 접근할 수 없습니다.
- 사람이 제어권을 가져오거나 정책이 거절하면 남은 작업을 중단합니다. 뒤에 대기 중인
  명령이 자동으로 제어권을 다시 얻지 않습니다. 설정 변경·권한 해제도 연결을 중단합니다.
- 타임라인에서 AppleScript·외부 자동화 출처를 표시합니다. 원문 명령과 사유는
  기존 감사 기록에 저장될 수 있습니다.
- **비밀번호·토큰 주입과 자동 비밀정보 가림은 지원하지 않습니다.** SSH 키·에이전트
  또는 인라인 비밀정보가 없는 PAM 인증 방식을 사용합니다. 공통 API의 민감 입력 요청은
  큐에 넣기 전에 거부합니다. 셸 기록과 화면 출력은 별도 저장 경로입니다.

## 시작 처리와 검증

화면과 외부 요청이 같은 시작 처리를 사용합니다. 외부 요청이 먼저 만든 세션은 화면이
초기 상태를 읽어 연결하며, 화면 연결 전 출력은 제한된 버퍼에 보존합니다. 이미 기본 탭이
생성된 뒤 외부 생성 요청이 오면 새 탭을 추가합니다. Cocoa 메인 루프는 셸 준비 작업을
기다리며 멈추지 않습니다. 창별 준비 상태와 출력 버퍼를 분리하며, 새 창 생성 실패나
창 닫기는 해당 창의 대기 요청과 셸만 정리합니다.

공통 테스트는 일회용 실제 PTY에서 프로필 제한·초기 실행·승인·입력 전달·중복 요청·
다른 발신자 거부·취소·실행 유예 중 사람 개입을 확인합니다. macOS CI는 앱 번들의
사전과 Info.plist, 예제의 컴파일을 확인하도록 구성합니다.

**실제 Mac 검증은 별도입니다.** 서명·공증된 Apple Silicon 앱에서 첫 권한 승인/거절,
앱 종료/실행 중 양쪽의 호출, 한글·인용부호, 모드·정책·취소·동시 호출과 해당 PAM의
스크립트를 확인해야 합니다. Linux 테스트나 사전 컴파일만으로 이 검증을 대신하지 않습니다.
