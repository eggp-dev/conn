# Conn agent integration

English · [한국어](#한국어) · [Getting started](../docs/getting-started.md)

This directory contains two complementary pieces:

| File | Purpose |
|---|---|
| [`.mcp.json`](.mcp.json) | Starts `conn mcp` as an MCP stdio server |
| [`skills/conn/SKILL.md`](skills/conn/SKILL.md) | Teaches the agent how to share a live terminal with a human |

The skill supplies procedure, not a separate execution engine. Tool schemas and permission checks come from Conn's MCP adapter and core.

## Setup

1. Install the Conn CLI and verify `conn --version` in the environment that starts your agent.
2. Open a Conn desktop or terminal session.
3. Register **`conn mcp`** as an MCP stdio server in your client. Use the absolute CLI path if necessary.
4. If your client supports local plugin or skill directories, load this directory or its skill through that client's documented mechanism. Installation commands differ between clients; the MCP server can be registered independently.

For clients using the common JSON shape:

```json
{
  "mcpServers": {
    "conn": {
      "command": "conn",
      "args": ["mcp"]
    }
  }
}
```

The agent identity comes from the MCP client's name. Optional arguments:

```text
conn mcp --agent-id my-agent
conn mcp --tools static
conn --socket <your-session-endpoint> mcp
```

`--tools static` keeps every tool in the list for clients that do not refresh it when permissions change. Unavailable operations still return errors. The default `auto` mode selects static lists for recognized Codex client names and dynamic lists otherwise.

## The workflow

1. Read the current screen with `terminal_snapshot`.
2. Request control with a short reason and **the exact planned command when known**. Conn retains that payload for review even if control is denied; the metadata does not execute or authorize anything.
3. Type the command, then send Enter with an intent. In Co-pilot mode, wait for the human to accept the proposal.
4. Wait for pending decisions. A policy block or denial is a result, not a reason to switch tools and retry.
5. Read another snapshot and report the visible result. `executed` means input reached the shell; it is not a shell exit status.
6. Release control when finished. If the human takes over, stop writing. If the human leaves the tab, wait or request their attention.

Ask your agent to use Conn for all shell work. A client with unrestricted built-in shell tools can still bypass Conn, so this instruction is not an isolation boundary. See the [trust model](../docs/security.md).

## Without MCP

Read the same guide with `conn guide`. For one-shot shell work from another terminal or an agent's CLI tool:

```sh
conn agent --agent-id demo run "pwd" --reason "Show the current directory without changing files"
```

Use `cd` for a cmd.exe target. The `run` action keeps request, typing, Enter, and the result snapshot on one connection. Separate `request`, `type`, and `enter` CLI processes do **not** share a lease. For a long-lived integration, use MCP or the [JSON protocol](../docs/protocol.md).

## 한국어

이 폴더에는 두 가지가 들어 있습니다.

| 파일 | 역할 |
|---|---|
| [`.mcp.json`](.mcp.json) | `conn mcp`를 MCP stdio 서버로 실행 |
| [`skills/conn/SKILL.md`](skills/conn/SKILL.md) | 사람이 있는 터미널을 함께 쓰는 에이전트의 작업 절차 |

스킬은 작업 절차를 제공하며 별도 실행 엔진이 아닙니다. 도구 정의와 권한 검사는 Conn의 MCP 어댑터와 코어가 담당합니다.

### 연결 순서

1. CLI를 설치하고 에이전트를 시작하는 환경에서 `conn --version`을 확인합니다.
2. Conn 데스크톱이나 터미널 세션을 엽니다.
3. 클라이언트에 **`conn mcp`**를 MCP stdio 서버로 등록합니다. 필요하면 CLI 절대 경로를 지정하세요. JSON 형식 예시는 위를 참고하세요.
4. 로컬 플러그인·스킬을 지원하는 클라이언트라면 해당 클라이언트의 안내에 따라 이 폴더나 스킬을 등록합니다. 설치 명령은 클라이언트마다 다르며 MCP 서버만 독립적으로 등록해도 됩니다.

에이전트 이름은 MCP 클라이언트 이름에서 정합니다. `--agent-id my-agent`로 고정하거나, 도구 목록 변경을 반영하지 못하는 클라이언트에는 `--tools static`을 지정할 수 있습니다. 목록에 도구가 보여도 권한이 없으면 실제 호출은 실패합니다. 기본 `auto` 모드는 인식한 Codex 클라이언트 이름에 정적 목록을 사용하고 그 외에는 동적 목록을 사용합니다. 기본값 외의 세션은 `conn --socket <연결-주소> mcp`로 지정하세요.

### 작업 절차

1. `terminal_snapshot`으로 현재 화면부터 읽습니다.
2. 짧은 이유와 **알고 있다면 정확한 예정 명령**을 포함해 제어권을 요청합니다. 거절되더라도 원문이 남습니다. 이 정보 자체가 실행이나 승인을 뜻하지는 않습니다.
3. 명령을 입력한 뒤 의도를 포함해 Enter를 보냅니다. Co-pilot에서는 사람이 제안을 수락할 때까지 기다립니다.
4. 대기 중인 결정이 있으면 기다립니다. 정책 차단이나 거절은 결과이며 다른 도구로 우회할 이유가 아닙니다.
5. 새 스냅샷을 읽고 보이는 결과를 보고합니다. `executed`는 입력 전달을 뜻하며 셸 종료 상태가 아닙니다.
6. 끝나면 제어권을 반환합니다. 사람이 회수하면 입력을 멈추고, 다른 탭으로 이동하면 기다리거나 확인을 요청합니다.

모든 셸 작업에 Conn을 쓰도록 에이전트에게 지시하세요. 제한 없는 내장 셸 도구가 있는 클라이언트는 Conn을 우회할 수 있으므로 지시만으로 실행 환경이 격리되지는 않습니다. [신뢰 범위](../docs/security.md)를 참고하세요.

MCP가 없으면 `conn guide`로 같은 절차를 읽고, 다른 터미널이나 에이전트의 CLI 도구에서 사용합니다.

```sh
conn agent --agent-id demo run "pwd" --reason "파일을 변경하지 않고 현재 디렉터리를 확인합니다"
```

cmd.exe 대상에는 `cd`를 사용하세요. `run`은 요청, 입력, Enter, 결과 스냅샷을 하나의 연결에서 처리합니다. `request`, `type`, `enter`를 서로 다른 CLI 프로세스로 실행하면 제어권을 공유하지 않습니다. 지속적인 연결에는 MCP 또는 [JSON 프로토콜](../docs/protocol.md)을 사용하세요.
