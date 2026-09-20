# Conn 유지보수와 릴리스

[English](releasing.md) · 한국어 · [사용자 설치](getting-started.ko.md) · [플랫폼 방침](platform-support.ko.md)

## 저장소와 CI

`eggp-dev/conn`의 통합 브랜치는 `main`입니다. Actions 토큰은 기본 읽기 전용으로 유지하고 비공개 보안 제보를 켜며 `main` 변경에 **Required checks**와 대화 해결을 요구합니다. [버전 관리되는 규칙](../.github/main-ruleset.json)은 PR·CI 요건과 소유자의 명시적인 복구 우회 권한을 제공합니다. 기존 규칙이 있으면 중복 생성하지 말고 수정하세요.

CI는 버전 일치, 공개 파일 검사, 문서 상대 링크, 릴리스 도구, 프런트엔드 테스트와 Svelte 빌드를 확인합니다. Linux·macOS Apple Silicon·Windows에서 Rust 테스트와 네이티브 데스크톱 디버그 빌드를 실행합니다. 릴리스 패키징도 이 세 대상을 사용하며 v0.6.0 이후 Intel Mac 배포는 잠시 중단합니다. macOS 디버그 빌드는 PR CI에서 ad-hoc 서명한 앱으로 묶어 `plutil`·`sdef`로 확인하고, `osacompile`로 해당 앱의 사전을 사용해 외부 자동화 예제를 컴파일합니다. 스크립트는 실행하지 않고 릴리스 자격 증명도 사용하지 않습니다. 이 검사는 설치 프로그램이나 데스크톱 실제 조작 검증을 대신하지 않습니다.

CI는 변경 범위에 맞춰 크기를 정합니다(`scripts/ci_scope.py`, **Scope** 작업이 판정). 문서만 바뀌면 문서·릴리스 도구·프런트엔드 작업만 실행합니다. 웹 UI가 바뀌면 Linux 데스크톱 빌드 하나를 더합니다. Rust를 건드린 PR은 세 운영체제에서 테스트하고 데스크톱은 Linux에서 한 번 빌드합니다. 네이티브 셸·패키징·외부 자동화·CI 자체를 바꾸면 머지 전에 모든 데스크톱을 빌드합니다. `main`에 합쳐진 코드, 수동 실행, 모든 릴리스 검증은 항상 전체 매트릭스를 사용하며, 분류할 수 없는 경로는 코드로 취급합니다. PR에서 전체 매트릭스가 필요하면 푸시 전에 `full-ci` 라벨을 붙이거나 해당 브랜치에서 CI 워크플로를 수동 실행하세요. 범위에 들어간 작업이 성공하지 못하면 **Required checks**가 실패합니다.

Actions는 커밋 SHA로 고정합니다. PR에는 서명 secrets를 제공하지 않고 초안 업로드 작업만 `contents: write` 권한을 가집니다. Mac 릴리스 작업은 [macOS 서명 안내](macos-signing.ko.md)의 자격 증명 6개를 사용하며 임시 키체인 암호는 작업 중 생성합니다.

## 버전 준비

1. 워크스페이스, `conn-core` 의존성, 데스크톱 Cargo, Tauri 설정, 프런트엔드 package/lock, 플러그인 manifest, marketplace 버전을 맞추고 두 Cargo lockfile을 갱신합니다.
2. 사용자 관점의 [CHANGELOG](../CHANGELOG.md)에 `## X.Y.Z — Preview · YYYY-MM-DD` 제목의 섹션을 추가합니다. 릴리스 안내의 "New in this release"는 이 섹션(목록과 `한국어:` 문단)을 그대로 옮기므로 Conn을 설치하는 사람을 위해 적으세요. 섹션이 없으면 `check`가 실패하며, `## Unreleased` 제목은 인정하지 않고 공개하지도 않습니다. 안내의 나머지는 모든 릴리스에 공통인 `scripts/release_notes.md`의 고정 문구이며, 릴리스 때문에 `release.py`를 고칠 일은 없습니다. 그런 다음 정확한 릴리스 버전으로 확인합니다.

   ```sh
   python3 scripts/release.py check --tag v0.8.2
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
   git tag -a v0.8.2 -m "Conn v0.8.2 preview"
   git push origin v0.8.2
   ```

태그 생성과 공개는 관리자의 릴리스 작업입니다. 공개된 태그를 옮기지 마세요.

## 초안 파이프라인

`v*` 태그 또는 기존 태그를 지정한 수동 실행으로 `release.yml`을 시작합니다. 버전 일치, 태그가 `main`에 포함된 커밋을 가리키는지 확인하고 그 커밋의 CI를 다시 실행합니다.

| 네이티브 대상 | 빌드 환경 | 산출물 |
|---|---|---|
| Ubuntu x64 | Ubuntu 24.04 | CLI `.tar.gz`, 데스크톱 `.deb`, `.AppImage` |
| macOS Apple Silicon | `macos-15` | CLI `.tar.gz`, 데스크톱 `.dmg`, 서명 보고서 |
| Windows x64 | Windows Server 2022 | CLI `.zip`, NSIS 설치 `.exe` |

실행 대상은 Ubuntu 24.04·26.04, 의도적인 Windows 무서명 프리뷰, Developer ID 서명·공증을 적용한 Mac 파일입니다. GitHub Mac 러너가 서명을 수행하므로 CI 작업마다 개인 Mac이 필요하지 않습니다. ad-hoc 서명으로 조용히 전환하지 않습니다.

각 네이티브 러너는 저장소 밖에 독립 CLI 압축 파일을 풀고 Rust·Node.js를 실행 경로에서 제외한 상태로 `--version`을 확인합니다. 이는 패키지의 CLI 검사이며 데스크톱 GUI 조작 검증은 아닙니다.

릴리스 단계는 다음과 같습니다.

1. 각 네이티브 러너에서 같은 버전의 CLI와 데스크톱 패키지를 빌드합니다.
2. Mac에서는 [서명 안내](macos-signing.ko.md)에 따라 앱, 내장 CLI, 독립 CLI, DMG의 서명·공증을 수행합니다. Apple Silicon의 승인 결과와 최종 파일 해시가 담긴 보고서를 만듭니다.
3. `release.py package`가 파일명을 정리합니다. `finalize`는 설치 파일·업데이트 파일·서명과 유효한 Apple Silicon 보고서를 모두 확인한 뒤 `SHA256SUMS`와 영·한 안내를 만듭니다. 안내는 `scripts/release_notes.md`에 이 버전의 파일 이름과 변경 이력 섹션을 채운 것입니다.
4. 해당 커밋의 CI와 모든 빌드·서명 작업이 통과하면 `draft`가 **14개 자산**을 업로드합니다. 바이너리·설치 파일 7개, Mac 업데이트 아카이브, 업데이트 서명 3개, `latest.json`, 서명 보고서, `SHA256SUMS`입니다. 미공개 프리릴리스를 생성하거나 갱신하며 이미 공개한 릴리스는 수정하지 않습니다.

재실행으로 미완성 초안을 보완할 수 있습니다. 자동 공개는 하지 않습니다. Actions 임시 산출물은 7일간 보관하며 업로드한 릴리스 파일은 유지됩니다. CLI 압축 파일에는 라이선스와 설치 안내가 들어갑니다. 플랫폼별 서명 업데이트 메타데이터도 같은 릴리스에 게시합니다.

## 검토와 공개

다른 로컬 빌드 대신 내려받은 초안 파일을 검토합니다.

- Apple Silicon 보고서, `Accepted` 공증 결과, 최종 파일 해시를 확인합니다. 서명 보고서는 정확한 릴리스 파일과 일치해야 합니다.
- 체크섬을 비교합니다. 모든 파일이 있으면 Linux는 `sha256sum -c SHA256SUMS`, macOS는 `shasum -a 256 -c SHA256SUMS`를 사용합니다. Windows는 `Get-FileHash <파일> -Algorithm SHA256` 결과를 해당 줄과 비교합니다. 체크섬은 배포자 서명과 별개입니다.
- [플랫폼 검증표](platform-support.ko.md#릴리스-검증표)를 진행합니다. 빌드·서명 결과와 새 설치·네이티브 GUI·외부 에이전트 연결 결과를 구분하고 확인하지 못한 OS 버전이나 동작은 미검증으로 표시합니다.
- Rust·Node.js·개발 서버 없이 앱을 실행하고 `PATH` 설정 없이 복사한 에이전트 연결 구성이 동작하는지 확인합니다. Windows PowerShell·cmd와 Apple Silicon의 실제 조작을 검증 완료로 소개하려면 해당 환경에서 먼저 시험합니다.
- Windows 무서명 안내를 유지합니다. Mac 파일에는 성공한 Developer ID·공증 근거가 필요합니다. 시스템 보호 기능을 끄도록 안내하지 않습니다.
- 영·한 릴리스 안내와 다운로드 링크를 검토하고 변경 이력의 미정 날짜를 공개일로 바꾼 뒤 검토한 프리릴리스를 명시적으로 공개합니다. 공개된 버전 페이지와 연결한 각 파일을 열어 접근 가능한지 확인합니다.

CI 빌드 통과만으로 모든 설치·조작이 검증되지는 않습니다. 실제로 수행한 플랫폼 검사와 미검증 항목을 릴리스 안내에 적으세요. 이 문구는 `scripts/release_notes.md`에 있으므로 검증 범위가 바뀌면 그 파일을 고칩니다. 비밀 정보는 Actions에 보관하며 인증서·자격 증명·상세 비공개 로그를 첨부하지 않습니다.

구현 참고: [Tauri GitHub 파이프라인](https://v2.tauri.app/distribute/pipelines/github/), 저장소의 [서명 기준](macos-signing.ko.md).

## 릴리스 후 한 줄 설치

손으로 할 일은 없습니다. `scripts/install.sh`는 항상 가장 최근에 공개된 릴리스를 찾습니다. Homebrew tap([eggp-dev/homebrew-tap](https://github.com/eggp-dev/homebrew-tap))은 6시간마다 새 릴리스를 확인해 그 릴리스의 `SHA256SUMS`로 cask를 다시 쓰고, macOS 러너에서 실제로 설치해 봅니다(audit, 설치, 서명·공증·버전 확인, 제거). 바로 반영하려면 tap의 **Follow Conn releases** 워크플로를 실행하세요.

웹사이트([conn.eggp.dev](https://conn.eggp.dev), `site/`에서 빌드)는 `scripts/install.sh`를 제공하고, 빌드 시점의 최신 릴리스를 보여 줍니다. 릴리스를 공개하면 **Redeploy the website** 워크플로가 실행되며 `SITE_DEPLOY_HOOK` 시크릿이 필요합니다. 없으면 호스팅 대시보드에서 직접 재배포하세요. `site/README.md`를 참고하세요.

## 저장소의 옛 주소

Conn은 2026-09-20에 `github.com/eggplantiny/conn`에서 `github.com/eggp-dev/conn`으로 옮겼습니다. 이미 설치한 사람들을 위해 두 가지 규칙을 지킵니다.

- **업데이트 매니페스트는 옛 주소를 유지합니다.** Conn 0.6.0~0.8.1은 다운로드 주소가 자신이 빌드될 때의 주소로 시작하는 업데이트만 설치합니다. 그래서 `scripts/release.py`의 `UPDATER_REPOSITORY`는 옛 주소로 두고, 릴리스 테스트가 이를 고정하며, 0.8.2부터는 두 주소를 모두 신뢰합니다. 일괄 치환으로 바꾸지 마세요.
- **`eggplantiny/conn`이라는 이름의 저장소를 다시 만들거나 포크하지 마세요.** GitHub는 그 이름이 비어 있는 동안에만 옛 주소를 넘겨 줍니다. 이름을 다시 쓰면 이전 설치본의 업데이트가 모두 멈춥니다.

저장소를 또 옮기게 되면 `scripts/release.py`의 `REPOSITORY`와 `REPOSITORY_SLUG`, 릴리스·서명 워크플로의 `github.repository` 조건(다른 곳에서는 서명이 거부됩니다), 업데이터 소스의 `RELEASES`(이전 주소를 지우지 말고 신뢰 목록에 추가), `scripts/install.sh`의 `REPO`, tap의 cask와 `CONN_REPO`, `media/demo/src/brand.ts`(이후 영상 재렌더링), README의 설치 명령을 바꾸세요.

## 실패와 복구

빌드·서명 실패 시 릴리스는 미공개 상태로 남습니다. 원인을 고치고 초안 워크플로를 다시 실행하세요. 미공개 태그를 바꾸는 결정은 명시적으로 기록해야 합니다. 공개한 릴리스라면 태그와 바이너리를 보존하고 문제를 기록한 뒤 패치 버전을 배포합니다. 공개 파일을 조용히 교체하지 않습니다.


## 앱 내 업데이트 배포

GitHub repository variable `TAURI_SIGNING_PUBLIC_KEY`와 secrets `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`가 필요합니다. 개인키는 암호화하여 소유자 전용 백업으로 보관하고, 암호는 승인된 비밀 관리 도구로 관리합니다. 키를 분실하면 기존 앱이 이후 업데이트를 신뢰할 수 없습니다. 키·암호를 로그나 소스에 남기지 마세요.

공증과 최종 패키징 뒤 Mac 앱 아카이브·AppImage·Windows 설치 파일에 서명을 붙입니다. CI는 실제 파일을 공개키로 검증하고 플랫폼별 `latest.json`을 생성합니다. 앱은 다운로드 서명이 확인된 경우에만 설치를 제공합니다. 공개된 릴리스를 GitHub API로 검색하므로 프리뷰도 별도 서버 없이 지원하며 정식 채널은 프리뷰로 자동 전환되지 않습니다.

v0.6.0 이전에는 한 번 직접 설치해야 합니다. deb는 패키지 관리자/수동 업데이트를 사용합니다. Mac·Windows·AppImage의 실제 설치 상태에서 버전 간 업그레이드, 권한, 재실행, 활성 셸 종료 확인은 빌드 통과와 별도로 검증해야 합니다. 전체 설정은 [영문 배포 절차](releasing.md#updater-signing)를 참고하세요.
