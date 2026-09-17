# 에이전트 연동

[English](agent-integrations.md)

Conn v0.4.0부터 클라이언트별 설정 버튼을 제공합니다. v0.3.0 사용자는 먼저 앱을 업데이트하세요.

## 클라이언트를 고르고 설정하기

1. Conn을 설치하고 터미널을 엽니다.
2. **설정 → 에이전트**에서 사용할 클라이언트의 **설정하기**를 누릅니다.
3. 카드의 안내에 따라 클라이언트를 재시작하거나 다시 연결합니다. Conn은 켜 둡니다.
4. “Conn으로 현재 터미널을 읽어 봐. 아직 명령은 실행하지 마.”라고 요청합니다.

Codex의 현재 카드 이름은 **Codex / ChatGPT**입니다. 로컬 Codex 데스크톱·CLI·IDE
작업을 설정하며, 별도 ChatGPT 앱의 연결을 설정하는 기능은 아닙니다.

앱에 포함된 실행 파일을 MCP stdio 서버로 등록하고 협업 스킬도 함께 설치합니다.
Rust·Node.js 설치나 PATH 변경은 필요하지 않습니다. 스킬은 앱 안에 포함하며,
설치 과정에서 인터넷의 설치 스크립트를 내려받거나 실행하지 않습니다.

**설정 완료**는 파일이 등록되었다는 뜻입니다. **현재 접속 중**은 해당 설정의
에이전트 이름으로 이 Conn 인스턴스의 터미널에 실제 접속했다는 뜻입니다.
클라이언트 인증이나 모델 작업 성공, 셸 제어 승인을 뜻하지 않습니다.
클라이언트의 신뢰 확인과 Conn의 승인 정책은 그대로 적용합니다.
스킬을 지원하지 않는 클라이언트도 MCP 서버에 내장된 절차 안내를 사용할 수 있습니다.

| 클라이언트 | 기본 개인 MCP 설정 | 스킬 | 설정 후 |
|---|---|---|---|
| Codex · 로컬 데스크톱 / CLI / IDE | `~/.codex/config.toml`의 `mcp_servers.conn` | `~/.agents/skills/conn/SKILL.md` | 로컬 클라이언트 재시작·새 작업에서 MCP 도구 확인 |
| Claude Code | `~/.claude.json`의 `mcpServers.conn` | `~/.claude/skills/conn/SKILL.md` | 새 세션에서 `/mcp`, `/skills` |
| Cursor | `~/.cursor/mcp.json`의 `mcpServers.conn` | `~/.cursor/skills/conn/SKILL.md` | 재시작 후 MCP 설정에서 Conn 활성화 |
| GitHub Copilot · VS Code | 기본 사용자 프로필 `mcp.json`의 `servers.conn` | `~/.copilot/skills/conn/SKILL.md` | 창 다시 로드 → **MCP: List Servers**에서 Conn 시작 |
| GitHub Copilot · CLI | `~/.copilot/mcp-config.json`의 `mcpServers.conn` | `~/.copilot/skills/conn/SKILL.md` | 새 세션에서 `/mcp` |

VS Code의 기본 사용자 폴더는 macOS에서 `~/Library/Application Support/Code/User`,
Windows에서 `%APPDATA%/Code/User`, Linux에서 `$XDG_CONFIG_HOME/Code/User` 또는
`~/.config/Code/User`입니다. 별도 프로필·포터블 설치·원격 컨테이너는 해당 환경에서
수동 설정합니다. 카드의 **설정 상세 → 수동 설정용 구성 복사**를 이용하면 해당
클라이언트 형식으로 복사할 수 있습니다.

Conn에 전달된 `CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `COPILOT_HOME`,
`XDG_CONFIG_HOME`, `APPDATA`를 반영합니다. Claude 사용자 지정 경로에서는
`$CLAUDE_CONFIG_DIR/.claude.json`에 MCP 설정을 기록합니다.
카드에서 실제 파일 경로를 먼저 확인할 수 있습니다. Dock이나 시작 메뉴에서
연 앱에는 셸에서 지정한 환경 변수가 전달되지 않을 수 있습니다.
프로젝트 설정·조직 정책·다른 프로필·이미 설치된 플러그인이 우선할 수 있으므로
개인 설정 파일을 기록한 것만으로 클라이언트가 로드했다고 판단하지 않습니다.

ChatGPT 웹·클라우드 작업과 다른 컴퓨터에서 실행하는 에이전트는 이 설정으로 로컬
stdio 서버에 접근할 수 없습니다.
이 기능은 셸을 인터넷에 공개하지 않습니다.

## 갱신·충돌·해제

- **설정 갱신**은 실행 파일 경로·연결 주소·스킬을 갱신합니다. 앱을 이동하거나
  재설치한 뒤 사용하세요. AppImage의 CLI는 유지되는 경로에 복사합니다.
- JSONC·TOML의 다른 항목과 주석을 보존합니다. 다른 설치 방식으로 등록된 Conn
  항목은 내용이 같더라도 자동으로 인수하지 않습니다. 설치 후 직접 수정한 항목이나
  스킬도 덮어쓰지 않고 검토하도록 안내합니다.
- **설정 해제**는 Conn이 관리하고 내용이 바뀌지 않은 항목과 스킬만 제거합니다.
  기존에 동일한 스킬이 있었다면 소유권을 가져오지 않습니다. Copilot의 공유 스킬은
  마지막 연결 설정을 해제할 때까지 남습니다. 실행 중인 MCP는 클라이언트를
  재시작해야 내려갈 수 있습니다.
- 심볼릭 링크·리디렉션된 상위 경로·잘못된 형식·중복 키·변경된 설치 위치는
  덮어쓰는 대신 수동 설정을 안내합니다.
- 설치 기록은 `<Conn 설정 폴더>/integrations.json`, 복구 사본은 같은 폴더의
  `integration-backups/`에 있습니다. 사본에 다른 클라이언트 설정이 포함될 수
  있으므로 비공개로 관리하세요. Unix 사본은 `0600`, Windows는 사용자 폴더의 ACL을
  사용합니다. 파일을 원자적으로 교체하고 감지한 실패는 되돌립니다. 전원 종료나
  외부 프로그램의 동시 저장은 수동 복구가 필요할 수 있습니다. 계속 설정 파일을
  덮어쓰는 클라이언트는 닫은 뒤 다시 시도하세요.

브라우저 테스트 하네스는 같은 설치 엔진을 `<테스트 상태>/agent-clients`에 적용합니다.
실제 사용자 클라이언트 설정을 변경하지 않으며 MCP는 네이티브 테스트 백엔드에 연결합니다.

## 확장 구조

`crates/frontend/src/integrations`를 Tauri와 브라우저 하네스가 공유합니다.

- `adapters.rs`: 클라이언트 ID·경로·설정 형식·MCP 기능 차이
- `config.rs`: Conn 항목만 변경하는 JSONC·TOML 편집
- `storage.rs`: 경로 검사·변경 감지·백업·원자적 교체·실패 복구
- `mod.rs`: 목록·등록·갱신·해제·수동 내보내기·소유권 기록
- `AgentConnections.svelte`: 목록 데이터로 구성하는 카드와 영문·한글 안내

새 클라이언트는 어댑터, 검증한 경로와 스키마, 번역 안내, 보존·해제 테스트를 추가합니다.
다른 전송 방식이나 플러그인 패키지는 이 경계 뒤에 추가할 수 있으며 터미널·정책·승인
엔진을 바꾸지 않습니다. 파일 저장을 접속 성공으로 간주하거나, 클라이언트 도구를
자동 승인하거나, 에이전트가 지정한 임의 경로에 설정을 쓰지 않습니다.

## 검증 범위

2026-09-16 기준 각 클라이언트 공식 문서를 확인했습니다.
[공식 자료 목록](agent-integrations.md#compatibility-evidence)을 참고하세요.
5개 어댑터와 Windows 경로·파이프 이스케이프를 자동 테스트하며, 실제 Rust 백엔드와
MCP 프로세스를 이용해 로컬 브라우저 화면을 확인합니다. 모든 OS에서 각 클라이언트의
실제 GUI를 연결하는 검증은 별도의 릴리스 확인 항목입니다.
