# Conn 유지보수와 릴리스

[English](releasing.md) · 한국어 · [사용자 설치](getting-started.ko.md) · [플랫폼 방침](platform-support.ko.md)

## 저장소와 CI

`eggplantiny/conn`의 통합 브랜치는 `main`입니다. Actions 토큰은 기본 읽기 전용으로 유지하고 비공개 보안 제보를 켜며 `main` 변경에 **Required checks**와 대화 해결을 요구합니다. [버전 관리되는 규칙](../.github/main-ruleset.json)은 PR·CI 요건과 소유자의 명시적인 복구 우회 권한을 제공합니다. 기존 규칙이 있으면 중복 생성하지 말고 수정하세요.

CI는 버전 일치, 공개 파일 검사, 문서 상대 링크, 릴리스 도구, 프런트엔드 테스트와 Svelte 빌드를 확인합니다. Linux·macOS ARM·Windows에서 Rust 테스트와 네이티브 데스크톱 디버그 빌드를 실행하고 릴리스 패키징에는 Intel macOS를 추가합니다. 이 검사는 설치 프로그램이나 데스크톱 실제 조작 검증을 대신하지 않습니다.

Actions는 커밋 SHA로 고정합니다. PR에는 서명 secrets를 제공하지 않고 초안 업로드 작업만 `contents: write` 권한을 가집니다. Mac 릴리스 작업은 [macOS 서명 안내](macos-signing.ko.md)의 자격 증명 6개를 사용하며 임시 키체인 암호는 작업 중 생성합니다.

## 버전 준비

1. 워크스페이스, `conn-core` 의존성, 데스크톱 Cargo, Tauri 설정, 프런트엔드 package/lock, 플러그인 manifest, marketplace 버전을 맞추고 두 Cargo lockfile을 갱신합니다.
2. 사용자 관점의 [CHANGELOG](../CHANGELOG.md) 항목을 적고 정확한 릴리스 버전으로 확인합니다.

   ```sh
   python3 scripts/release.py check --tag v0.4.0
   python3 scripts/check_repo.py
   python3 -m unittest discover -s tests/release -v
   cargo test --workspace --locked
   cd frontends/tauri
   npm ci
   npm test
   npm run build
   ```

3. 검토한 릴리스 변경을 합치고 해당 커밋의 **Required checks**를 기다립니다. 태그의 소스에 에이전트 연결 UI와 바이너리 설치 문서도 포함하세요.
4. 의도적으로 버전 태그를 생성하고 푸시합니다.

   ```sh
   git tag -a v0.4.0 -m "Conn v0.4.0 preview"
   git push origin v0.4.0
   ```

태그 생성과 공개는 관리자의 릴리스 작업입니다. 공개된 태그를 옮기지 마세요.

## 초안 파이프라인

`v*` 태그 또는 기존 태그를 지정한 수동 실행으로 `release.yml`을 시작합니다. 버전 일치, 태그가 `main`에 포함된 커밋을 가리키는지 확인하고 그 커밋의 CI를 다시 실행합니다.

| 네이티브 대상 | 빌드 환경 | 산출물 |
|---|---|---|
| Ubuntu x64 | Ubuntu 24.04 | CLI `.tar.gz`, 데스크톱 `.deb`, `.AppImage` |
| macOS Apple Silicon | `macos-15` | CLI `.tar.gz`, 데스크톱 `.dmg`, 서명 보고서 |
| macOS Intel | `macos-15-intel` | CLI `.tar.gz`, 데스크톱 `.dmg`, 서명 보고서 |
| Windows x64 | Windows Server 2022 | CLI `.zip`, NSIS 설치 `.exe` |

실행 대상은 Ubuntu 24.04·26.04, 의도적인 Windows 무서명 프리뷰, Developer ID 서명·공증을 적용한 Mac 파일입니다. GitHub Mac 러너가 서명을 수행하므로 CI 작업마다 개인 Mac이 필요하지 않습니다. ad-hoc 서명으로 조용히 전환하지 않습니다.

각 네이티브 러너는 저장소 밖에 독립 CLI 압축 파일을 풀고 Rust·Node.js를 실행 경로에서 제외한 상태로 `--version`을 확인합니다. 이는 패키지의 CLI 검사이며 데스크톱 GUI 조작 검증은 아닙니다.

릴리스 단계는 다음과 같습니다.

1. 각 네이티브 러너에서 같은 버전의 CLI와 데스크톱 패키지를 빌드합니다.
2. Mac에서는 [서명 안내](macos-signing.ko.md)에 따라 앱, 내장 CLI, 독립 CLI, DMG의 서명·공증을 수행합니다. 아키텍처별로 승인 결과와 최종 파일 해시가 담긴 보고서를 만듭니다.
3. `release.py package`가 파일명을 정리합니다. `finalize`는 바이너리·설치 파일 9개와 유효한 Mac 보고서 두 개를 모두 확인한 뒤 `SHA256SUMS`와 영·한 안내를 만듭니다.
4. 해당 커밋의 CI와 모든 빌드·서명 작업이 통과하면 `draft`가 **12개 자산**을 업로드합니다. 바이너리·설치 파일 9개, 서명 보고서 2개, `SHA256SUMS`입니다. 미공개 프리릴리스를 생성하거나 갱신하며 이미 공개한 릴리스는 수정하지 않습니다.

재실행으로 미완성 초안을 보완할 수 있습니다. 자동 공개는 하지 않습니다. Actions 임시 산출물은 7일간 보관하며 업로드한 릴리스 파일은 유지됩니다. CLI 압축 파일에는 라이선스와 설치 안내가 들어갑니다. 이번 프리뷰에는 자동 업데이트가 없습니다.

## 검토와 공개

다른 로컬 빌드 대신 내려받은 초안 파일을 검토합니다.

- 두 Mac 보고서, `Accepted` 공증 결과, 최종 파일 해시를 확인합니다. 서명 보고서는 정확한 릴리스 파일과 일치해야 합니다.
- 체크섬을 비교합니다. 모든 파일이 있으면 Linux는 `sha256sum -c SHA256SUMS`, macOS는 `shasum -a 256 -c SHA256SUMS`를 사용합니다. Windows는 `Get-FileHash <파일> -Algorithm SHA256` 결과를 해당 줄과 비교합니다. 체크섬은 배포자 서명과 별개입니다.
- [플랫폼 검증표](platform-support.ko.md#릴리스-검증표)를 진행합니다. 빌드·서명 결과와 새 설치·네이티브 GUI·외부 에이전트 연결 결과를 구분하고 확인하지 못한 OS 버전이나 동작은 미검증으로 표시합니다.
- Rust·Node.js·개발 서버 없이 앱을 실행하고 `PATH` 설정 없이 복사한 에이전트 연결 구성이 동작하는지 확인합니다. Windows PowerShell·cmd와 Mac 두 아키텍처의 실제 조작을 검증 완료로 소개하려면 해당 환경에서 먼저 시험합니다.
- Windows 무서명 안내를 유지합니다. Mac 파일에는 성공한 Developer ID·공증 근거가 필요합니다. 시스템 보호 기능을 끄도록 안내하지 않습니다.
- 영·한 릴리스 안내와 다운로드 링크를 검토하고 변경 이력의 미정 날짜를 공개일로 바꾼 뒤 검토한 프리릴리스를 명시적으로 공개합니다. 공개된 버전 페이지와 연결한 각 파일을 열어 접근 가능한지 확인합니다.

CI 빌드 통과만으로 모든 설치·조작이 검증되지는 않습니다. 실제로 수행한 플랫폼 검사와 미검증 항목을 릴리스 안내에 적으세요. 비밀 정보는 Actions에 보관하며 인증서·자격 증명·상세 비공개 로그를 첨부하지 않습니다.

구현 참고: [Tauri GitHub 파이프라인](https://v2.tauri.app/distribute/pipelines/github/), 저장소의 [서명 기준](macos-signing.ko.md).

## 실패와 복구

빌드·서명 실패 시 릴리스는 미공개 상태로 남습니다. 원인을 고치고 초안 워크플로를 다시 실행하세요. 미공개 태그를 바꾸는 결정은 명시적으로 기록해야 합니다. 공개한 릴리스라면 태그와 바이너리를 보존하고 문제를 기록한 뒤 패치 버전을 배포합니다. 공개 파일을 조용히 교체하지 않습니다.
