# 확장 기능

[English](extensions.md) · [신뢰 모델](security.md#native-extensions)

현재 확장은 터미널 테마를 제공합니다. 그 뒤의 레지스트리는 앞으로 검토된 외부 연동을
붙일 자리이며, 지금 제공하는 실행형 확장은 없습니다. 마켓플레이스나 임의의 제삼자
실행 코드를 설치하는 기능은 제공하지 않습니다.

## 터미널 테마 바꾸기

**설정 → 확장**에서 Midnight·Paper·Solar·Nord·Mono를 고르거나 **테마 설치…**로
Conn 테마 JSON을 가져옵니다. 테마는 지원되는 터미널 색상만 바꿉니다. 승인 컨트롤을
숨기거나 공유를 바꾸거나 스크립트·명령을 실행하거나 앱 내비게이션을 대체할 수 없습니다.
기존 외형 설정은 별도로 유지합니다.

테마 파일의 전체 예제는 [영문 문서](extensions.md#change-a-terminal-theme)에 있습니다.

- `apiVersion: 1`, `kind: "theme"`, `capabilities: ["theme"]`를 사용합니다.
- `id`는 고유해야 하며 테마 내부의 `id`와 일치해야 합니다.
- 배경·글자·커서·선택 영역은 여섯 자리 16진수 색상입니다.
- `ansi`는 기본 여덟 색과 밝은 여덟 색을 순서대로 포함합니다.
- 알 수 없는 필드·API 버전·중복 ID·추가 권한·실행 진입점은 거절합니다.
- 사용자 테마는 최대 32개이며 `extensions.json`에 저장합니다.

첫 확장 호스트는 셸 시작 파일을 편집하거나 프롬프트 프로그램을 설치하지 않습니다.
실제 셸 코드를 실행하는 프롬프트 확장은 별도의 실행 계약이 필요합니다.

## 확장 구조

| 기여 기능 | 권한 | 실행 방식 |
| --- | --- | --- |
| 터미널 테마 | `theme` | 검증된 선언형 색상 |

등록·설정·공통 UI는 Conn이 소유합니다. 확장은 PTY나 소유자 브리지를 받지 않습니다.
manifest 형식에는 `provider`·`completion` 종류와 `visible_frame`·`model_request`·
`proposal` 권한도 정의되어 있지만, 이런 manifest는 검토된 내장 ID와 정확히 일치할 때만
허용하며 현재 그런 내장 확장은 없으므로 거절합니다. v0.7.0 프리뷰의 내장 OpenAI 명령
제안과 API 키 저장은 제거했습니다. manifest 자체가 실행 코드를 격리하는 것은 아닙니다.
임의의 네이티브·JavaScript 실행, 자유로운 네트워크·파일 접근, 웹뷰, 플러그인 간
의존성은 이 호스트의 범위에서 제외합니다.

그 프리뷰에서 API 키를 저장했다면 Conn은 더 이상 그 키를 읽거나 지우지 않습니다.
macOS Keychain, Windows Credential Manager 또는 Linux Secret Service 키링에서
`dev.eggp.conn.model-provider` 항목을 직접 삭제하세요.

구현 계약은 [`crates/frontend/src/extensions`](../crates/frontend/src/extensions/README.md)에
있습니다. 기존 `plugin/` 디렉터리는 **외부 클라이언트용 MCP·스킬 연결 패키지**이며
새로운 앱 내부 확장 런타임과 구분합니다.

## 검증 범위

단위 테스트는 스키마 거절, 실행형 종류·추가 권한 거절, 테마 저장, 제거된 제공자가 쓴
`extensions.json` 읽기를 검사합니다. macOS·Windows의 실제 화면 동작을 증명하지는
않습니다. 공유 데이터 범위는 [신뢰 모델](security.md)을 참고하세요.
