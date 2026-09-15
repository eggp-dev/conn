# Conn 유지보수와 릴리스

[English](releasing.md)

## 저장소 설정

저장소는 `eggplantiny/conn`, 통합 브랜치는 `main`입니다. GitHub에서 Actions·Issues·비공개 보안 제보를 켜고, 기본 워크플로 토큰은 읽기 전용으로 유지하세요. 브랜치 규칙에는 **Required checks** 통과와 대화 해결을 요구하고, 복구를 위한 관리자의 명시적 우회 경로를 남깁니다. 1인 프리뷰 운영에서 받을 수 없는 두 번째 리뷰를 필수로 요구하지 않습니다.

About 문구 제안: **A shared terminal for humans and AI agents. Review, hand over, take back.** 토픽: `terminal`, `mcp`, `ai-agents`, `human-in-the-loop`, `tauri`, `rust`, `svelte`. 한국어와 영어 이슈·기여를 모두 받습니다. 향후 웹사이트는 별도 승인 사항이며 이 설정은 도메인을 만들지 않습니다.

버전 관리되는 [main 규칙](../.github/main-ruleset.json)은 PR·CI 통과를 요구하고 강제 푸시·삭제를 막으며, 저장소 소유자에게 명시적인 복구 우회 권한을 둡니다. 같은 규칙이 없을 때만 `gh api --method POST repos/eggplantiny/conn/rulesets --input .github/main-ruleset.json`으로 생성하고, 이후에는 기존 ID를 수정하세요.

## CI 구성

버전 일치, 공개 파일 검사, 문서의 상대 링크, 릴리스 도구 테스트, 타임라인·아이콘 테스트와 Svelte 빌드를 검증합니다. Linux·macOS ARM·Windows에서 Rust 테스트와 데스크톱 디버그 빌드를 실행하며, 릴리스에는 Intel macOS도 포함됩니다. Unix 전용 PTY 테스트 통과만으로 Windows 런타임을 검증했다고 볼 수 없습니다.

Actions는 커밋 SHA로 고정하고 Dependabot이 주간 업데이트를 제안합니다. PR에는 릴리스 자격 증명을 제공하지 않습니다. Draft Release를 만드는 작업만 `contents: write`를 가집니다.

## 버전 올리기

1. 워크스페이스·conn-core 의존성·독립 Tauri Cargo·Tauri 설정·프런트엔드 package/lock·플러그인 manifest·marketplace 버전을 맞춥니다. 두 Cargo lockfile도 갱신합니다.
2. [CHANGELOG](../CHANGELOG.md)에 사용자 관점의 변경을 적고 검증합니다.

   ```sh
   python3 scripts/release.py check --tag v0.3.0
   python3 scripts/check_repo.py
   python3 -m unittest discover -s tests/release -v
   cargo test --workspace --locked
   cd frontends/tauri
   npm ci
   npm test
   npm run build
   ```

3. 검토한 변경을 `main`에 반영하고 **Required checks** 통과를 기다립니다.
4. 해당 커밋에 `git tag -a v0.3.0 -m "Conn v0.3.0 preview"`, `git push origin v0.3.0`을 실행합니다. 태그 생성·푸시는 관리자의 배포 작업이며 로컬 빌드에 포함되지 않습니다. 공개된 태그를 옮기지 마세요.

## Draft Release 파이프라인

`v*` 태그 또는 기존 태그를 지정한 수동 실행으로 시작합니다. 버전 일치와 태그 커밋의 `main` 포함 여부를 확인하고, 정확히 그 커밋의 CI를 다시 실행합니다.

| 대상 | 산출물 |
|---|---|
| Linux x64 | CLI `.tar.gz`, 데스크톱 `.deb`와 `.AppImage` |
| macOS Apple Silicon | CLI `.tar.gz`, 데스크톱 `.dmg` |
| macOS Intel | CLI `.tar.gz`, 데스크톱 `.dmg` |
| Windows x64 | CLI `.zip`, NSIS 설치 `.exe` |

`package`가 파일명을 정리하고 `finalize`가 9개 파일 모두를 확인한 뒤 `SHA256SUMS`와 영·한 릴리스 노트를 만듭니다. 모든 빌드 성공 후에만 **비공개 초안인 Draft prerelease**를 생성합니다. 이미 공개한 릴리스는 수정하지 않습니다. 재실행으로 미완성 초안을 보완할 수 있으며 자동으로 Publish하지 않습니다.

데스크톱에는 같은 버전의 CLI sidecar가 포함됩니다. CLI 압축 파일에는 라이선스와 설치 안내가 들어갑니다. Actions 임시 산출물은 7일 보관하고 업로드한 릴리스 파일은 유지합니다. 자동 업데이트 기능은 없습니다.

## 공개 전 확인

다운로드한 파일의 체크섬을 비교하세요. 전체 파일을 받았다면 Linux는 `sha256sum -c SHA256SUMS`, macOS는 `shasum -a 256 -c SHA256SUMS`, Windows는 `Get-FileHash` 결과를 사용합니다. 체크섬은 전송 오류를 확인하는 수단이며 신뢰할 수 있는 출처 없이 배포자 신원을 보증하지 않습니다.

각 OS·아키텍처에서 설치·실행, CLI 설치/압축 해제, 프로필 열기, `conn mcp` 연결, 임시 파일 읽기, 삭제 거절, 별도 삭제 승인, 제어권 회수·Grace, 타임라인 요청 원문, 영·한 전환, 세션 종료를 확인하세요. 결과와 한계를 릴리스 노트에 남기고 실제 공개 시점에 변경 이력의 날짜를 확정합니다.

### 서명

macOS는 ad-hoc 서명(`APPLE_SIGNING_IDENTITY=-`)을 사용하며 **공증되지 않습니다**. Windows 설치 파일도 **인증서 서명이 없습니다**. OS 경고가 나타날 수 있으므로 검증된 배포자 서명이라고 설명하지 마세요. 넓게 배포하기 전 인증서를 확보하고 보호된 릴리스 환경의 secrets에만 설정해 Tauri 서명·공증을 구성해야 합니다. 인증서·비밀번호·키를 저장소에 넣지 마세요.

공식 [Tauri GitHub 파이프라인](https://v2.tauri.app/distribute/pipelines/github/), [배포 안내](https://v2.tauri.app/distribute/), [Windows 설치 파일 안내](https://v2.tauri.app/distribute/windows-installer/)를 참고하세요. 실제 설치와 서명 검증은 대상 플랫폼에서 수행해야 합니다.

## 실패와 복구

빌드 실패 시 공개 릴리스는 생성되지 않습니다. 소스를 고치고 CI를 통과시킨 뒤 필요하면 새 버전을 만듭니다. 미공개 태그 변경은 협업자에게 명확히 알리고, 이미 공개했다면 기존 태그와 파일을 보존한 채 문제를 기록하고 패치 버전으로 배포하세요. 공개 바이너리를 조용히 교체하지 않습니다.
