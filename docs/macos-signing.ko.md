# macOS 릴리스 서명

[English](macos-signing.md) · 한국어 · [플랫폼 방침](platform-support.ko.md) · [릴리스](releasing.ko.md)

Mac 공개 패키지는 Developer ID 서명과 Apple 공증을 통과해야 합니다. 이 문서는 파이프라인과 필요한 근거를 설명하며 특정 실행이 이미 성공했다는 뜻은 아닙니다. 해당 실행의 `-signing.json` 파일 두 개와 릴리스 안내를 확인하세요.

GitHub의 `macos-15`, `macos-15-intel` 러너에서 두 아키텍처를 빌드하고 서명합니다. 관리자는 Linux에서 CI를 운영할 수 있으며 릴리스 작업마다 개인 Mac을 사용할 필요는 없습니다. 네이티브 첫 실행·GUI 확인에는 Mac 환경이 필요합니다. [GitHub 러너 안내](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)를 참고하세요.

## 릴리스 Secret 6개

기존 Apple ID 공증 방식을 사용합니다. macOS 릴리스 작업이 사용하는 GitHub Actions Secret은 다음 6개입니다.

| Secret | 값 |
|---|---|
| `APPLE_CERTIFICATE` | Developer ID 인증서와 개인 키가 포함된 `.p12`의 Base64 값 |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12` 내보내기 암호 |
| `APPLE_SIGNING_IDENTITY` | 전체 Developer ID Application 서명 신원 |
| `APPLE_ID` | Apple 계정 이메일 |
| `APPLE_PASSWORD` | 계정 로그인 암호가 아닌 앱 전용 암호 |
| `APPLE_TEAM_ID` | 개발자 Team ID |

작업이 임시 키체인용 무작위 암호를 만듭니다. **`KEYCHAIN_PASSWORD` Secret을 추가하지 않습니다.** 자격 증명은 Actions secrets에 보관하고 저장소·이슈·대화·로그에 넣지 마세요. PR 빌드에는 이 값들을 제공하지 않습니다. 릴리스 워크플로와 ref를 보호하고, 필요하면 보호된 Actions 환경으로 검토 단계를 추가할 수 있습니다.

유효한 Developer ID Application 인증서는 개인 키를 포함하고 설정한 Team ID와 일치해야 합니다. 멤버십만으로 인증서가 준비되거나 공증이 통과한 것은 아닙니다. 값이 없거나 잘못되면 워크플로가 실패하며 ad-hoc 서명으로 전환하지 않습니다. 인증서·공증 입력은 [Tauri 서명 안내](https://v2.tauri.app/distribute/sign/macos/)에 설명되어 있습니다.

## 파이프라인 단계

1. **준비.** 임시 러너 키체인에 인증서를 가져오고 Developer ID 신원과 Team 일치를 확인합니다.
2. **빌드.** Tauri가 앱과 내장 CLI를 빌드·서명하고 앱 공증과 티켓 첨부를 수행합니다.
3. **최종 파일 검증·공증.** 엄격한 서명 검사, hardened runtime, 안전한 타임스탬프를 확인합니다. 독립 CLI를 별도로 서명해 ZIP으로 Apple에 제출하고 `Accepted`를 요구합니다. 최종 DMG도 공증·티켓 첨부한 뒤 마운트해 내장 앱과 티켓을 확인합니다. 독립 실행 파일 자체에는 티켓을 첨부할 수 없으므로 오프라인 티켓이 있다고 설명하지 않습니다.
4. **패키징과 근거 기록.** 최종 DMG와 CLI 압축 파일을 모은 뒤 Mac 대상별로 `conn-v0.3.0-<target>-signing.json`을 만듭니다. 소스 커밋, 제출 ID, 승인 여부, 검사 결과, 최종 파일 해시를 기록하며 인증서 암호·자격 증명 값·상세 공증 로그를 넣지 않습니다.
5. **업로드 검사.** 두 보고서가 최종 DMG와 CLI 압축 파일에 일치해야 체크섬 생성과 초안 업로드로 진행합니다. 보고서도 공개 자산이며 `SHA256SUMS`에 포함됩니다.
6. **정리.** 실패한 경우까지 항상 임시 키체인·자격 증명 파일을 지우고 이전 키체인 검색 목록을 복원합니다.

`.app` 서명만으로 별도 배포 CLI까지 서명되지는 않습니다. 패키징·체크섬 생성 전에 모든 서명·티켓 첨부를 마쳐야 합니다. 파일을 다시 빌드하거나 바꾸면 기존 해시로 연결한 검증 근거는 더 이상 맞지 않습니다.

JSON 보고서는 네이티브 CI 검사 요약이며 별도로 서명된 증명서는 아닙니다. Apple 서명과 공증 티켓은 배포 소프트웨어에 적용됩니다.

## 공개 전 확인

Apple Silicon·Intel Mac에서 브라우저로 초안을 내려받으세요. 서명과 티켓을 확인하고 [네이티브 조작 검증표](platform-support.ko.md#릴리스-검증표)를 진행합니다. 앱 확인 명령 예시는 다음과 같습니다.

```sh
codesign --verify --deep --strict --verbose=2 /Applications/Conn.app
codesign -dv --verbose=4 /Applications/Conn.app
spctl --assess --type execute --verbose=4 /Applications/Conn.app
xcrun stapler validate /Applications/Conn.app
```

종료 코드뿐 아니라 서명 신원과 공증 근거를 확인하세요. 검증한 OS 버전, 파일 해시, 첫 실행 결과, 미검증 항목을 릴리스 안내에 기록합니다. ad-hoc 파일을 공증된 공개 패키지로 배포하지 않고, 공개한 파일을 뒤늦게 바꾸지 않습니다. 바이너리를 바꾸면 새 버전으로 배포하세요.
