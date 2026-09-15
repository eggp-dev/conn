# macOS 서명 준비표

[English](macos-signing.md) · 한국어 · [플랫폼 방침](platform-support.ko.md)

**현재 상태: 준비 단계.** Apple 개발자 가입과 서명 자격 증명은 대기 중입니다.
현재 워크플로는 ad-hoc 테스트 패키지를 만듭니다. 아래는 Developer ID 배포를 활성화하기
위한 작업이며 이미 서명 연동이 끝났다는 뜻이 아닙니다.

## 관리자가 준비할 것

1. Apple Developer Program 가입을 완료하고 Team ID를 확인합니다.
2. Mac에서 인증서 서명 요청(CSR)을 만들고 Mac App Store 밖에서 배포하는
   **Developer ID Application** 인증서를 발급합니다. 개인 키를 보관하고 인증서·키를
   암호가 걸린 `.p12`로 내보낸 뒤 전체 서명 신원을 기록합니다. GitHub 다운로드를 위해
   App Store에 앱을 등록할 필요는 없습니다.
3. 공증 자격 증명을 선택합니다. Apple ID·**앱 전용 암호**·Team ID 조합을 쓰거나
   App Store Connect API 키를 사용할 수 있습니다.
4. GitHub Actions에 릴리스 서명용 보호 환경을 만들고 secrets에 직접 등록합니다.
   이슈·PR·대화·저장소 파일·빌드 로그에 인증 정보를 붙이지 않습니다.

Apple ID 방식을 선택할 때 준비할 항목입니다.

| 이름 | 값 |
|---|---|
| `APPLE_CERTIFICATE` | 개인 키가 포함된 `.p12`의 Base64 인코딩 값 |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12`를 내보낼 때 사용한 암호 |
| `APPLE_SIGNING_IDENTITY` | 전체 Developer ID Application 서명 신원 |
| `KEYCHAIN_PASSWORD` | 임시 CI 키체인 전용 암호 |
| `APPLE_ID` | Apple 계정 이메일 |
| `APPLE_PASSWORD` | 계정 로그인 암호가 아닌 앱 전용 암호 |
| `APPLE_TEAM_ID` | 개발자 Team ID |

## 자격 증명 준비 후 저장소에서 할 일

- macOS 릴리스 작업과 보호된 릴리스 ref·환경에만 자격 증명을 제공합니다.
  PR 검사·일반 빌드에는 서명 secrets가 필요하지 않습니다.
- 임시 러너 키체인에 인증서를 가져오고 현재 ad-hoc 신원을 교체한 뒤 Tauri에 공증
  자격 증명을 연결합니다. 필수 값이 없으면 서명 빌드를 실패시키며 조용히 ad-hoc으로
  전환하지 않습니다.
- Mac 두 아키텍처를 빌드하고 앱과 내장 `conn` sidecar를 확인합니다. 별도 CLI 압축
  파일의 원본은 워크스페이스 `target` 아래에 있으므로 앱을 서명했다고 독립 CLI까지
  서명된 것은 아닙니다. 이 바이너리에 hardened runtime을 적용해 서명하고 허용되는
  공증 컨테이너로 제출하여 승인받은 뒤 최종 CLI 압축 파일을 만듭니다.
- 앱·DMG의 공증과 stapling을 확인합니다. 모든 서명·stapling 변경 **후에** 체크섬을
  다시 생성합니다. 현재 ad-hoc이라고 쓰는 릴리스 노트도 각 파일의 실제 서명 상태에
  맞춥니다.
- Apple Silicon·Intel Mac에서 브라우저로 초안을 새로 내려받고 설치한 뒤
  [플랫폼 검증표](platform-support.ko.md#릴리스-검증표)를 진행합니다.

완성된 앱의 확인 명령 예시입니다.

```sh
codesign --verify --deep --strict --verbose=2 /Applications/Conn.app
codesign -dv --verbose=4 /Applications/Conn.app
spctl --assess --type execute --verbose=4 /Applications/Conn.app
xcrun stapler validate /Applications/Conn.app
```

명령 종료 코드뿐 아니라 서명 신원과 공증 결과도 확인합니다. 비밀 정보가 없는 검증
결과를 릴리스 근거에 보관합니다. 이미 공개한 파일은 보존하며 바이너리를 바꾸려면
새 버전으로 배포합니다.

근거: [Tauri macOS 서명·공증 안내](https://v2.tauri.app/distribute/sign/macos/),
[Apple Developer 가입](https://developer.apple.com/programs/enroll/).
