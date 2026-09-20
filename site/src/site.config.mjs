// Which of ../docs the site publishes. The repository and the site address come from @conn/brand,
// shared with the films and the release tooling.
import brand from "@conn/brand/brand.json" with { type: "json" };

export const repository = brand.repository;
export const origin = brand.site;
/** When the hero film on the landing pages was published (video markup needs a date). Update after a re-render. */
export const filmPublished = "2026-09-20";

/** Sidebar groups. Items are file names in ../docs without `.md`; a `.ko.md` sibling becomes the Korean page. */
export const docs = [
  {
    label: "Start",
    ko: "시작하기",
    items: ["getting-started", "first-collaboration", "agent-integrations", "faq"],
  },
  {
    label: "How it works",
    ko: "동작 원리",
    items: ["security", "policy", "architecture", "shell-integration"],
  },
  {
    label: "Reference",
    ko: "레퍼런스",
    items: ["protocol", "backends", "extensions", "external-automation", "platform-support"],
  },
];

/**
 * What a search result shows under each docs page's title, per language (160 characters at most;
 * scripts/sync.mjs fails the build when one is missing or longer). Say what the page answers, in
 * the words someone would search with.
 */
export const descriptions = {
  "getting-started": {
    en: "Install Conn on macOS, Windows or Linux, connect Claude Code, Codex, Cursor or GitHub Copilot over MCP, and take turns with the agent in one terminal.",
    ko: "macOS·Windows·Linux에 Conn을 설치하고 Claude Code, Codex, Cursor, GitHub Copilot을 MCP로 연결해 한 터미널에서 에이전트와 번갈아 작업하는 방법.",
  },
  "first-collaboration": {
    en: "A five-minute exercise in a throwaway folder: let the agent work in your terminal, take the keyboard back mid-task, fix it yourself, and hand it back.",
    ko: "임시 폴더에서 5분이면 끝나는 첫 협업 예제: 에이전트가 내 터미널에서 일하게 하고, 중간에 키보드를 되찾아 직접 고친 뒤 다시 맡깁니다.",
  },
  "agent-integrations": {
    en: "Connect an AI coding agent to your terminal with one click: MCP setup for Claude Code, Codex, Cursor and GitHub Copilot, plus manual configuration.",
    ko: "AI 코딩 에이전트를 내 터미널에 연결하기: Claude Code, Codex, Cursor, GitHub Copilot용 MCP 설정을 버튼 한 번으로, 또는 직접 구성하는 방법.",
  },
  faq: {
    en: "Answers about Conn: subscriptions, whether the agent can see passwords, what happens when you type, SSH sessions, what is recorded, and updates.",
    ko: "Conn 자주 묻는 질문: 추가 구독이 필요한지, 에이전트가 암호를 볼 수 있는지, 내가 입력하면 어떻게 되는지, SSH, 기록, 업데이트.",
  },
  security: {
    en: "Conn's trust model: what an AI agent can read and run in your shared terminal, how approvals and hidden input work, what is stored, and what Conn won't claim.",
    ko: "Conn의 신뢰 모델: 공유 터미널에서 AI 에이전트가 읽고 실행할 수 있는 것, 승인과 숨김 입력의 동작, 저장되는 데이터, 그리고 Conn이 보장하지 않는 것.",
  },
  policy: {
    en: "Write the YAML policy that decides which agent commands run, which wait for your approval (rm, sudo, git push --force) and which are always denied.",
    ko: "에이전트 명령 중 무엇을 바로 실행하고, 무엇을 승인 대기시키고(rm, sudo, git push --force), 무엇을 항상 거부할지 정하는 YAML 정책 작성법.",
  },
  architecture: {
    en: "How Conn arbitrates one PTY between a human and AI agents: session authority, the core terminal grid agents read, sharing transitions and the extension host.",
    ko: "Conn이 하나의 PTY를 사람과 AI 에이전트 사이에서 중재하는 방식: 세션 제어권, 에이전트가 읽는 코어 터미널 그리드, 공유 전환, 확장 호스트.",
  },
  "shell-integration": {
    en: "How Conn records the commands you run in Bash and Zsh next to the agent's, with exit codes and working directories, without logging your keystrokes.",
    ko: "Bash·Zsh에서 내가 실행한 명령을 에이전트의 명령과 나란히 기록하는 방식: 종료 코드와 작업 폴더는 남기고, 키 입력은 기록하지 않습니다.",
  },
  protocol: {
    en: "The local JSON protocol and MCP tools AI agents use to read a shared terminal, request control, type, run commands and wait for human approval in Conn.",
    ko: "AI 에이전트가 Conn의 공유 터미널을 읽고, 제어권을 요청하고, 입력·실행하고, 사람의 승인을 기다릴 때 쓰는 로컬 JSON 프로토콜과 MCP 도구.",
  },
  backends: {
    en: "Terminal profiles in Conn: local shells, WSL distributions, SSH servers and Docker containers, and how agent commands are reviewed in each of them.",
    ko: "Conn의 터미널 프로필: 로컬 셸, WSL 배포판, SSH 서버, Docker 컨테이너, 그리고 각 환경에서 에이전트 명령이 검토되는 방식.",
  },
  extensions: {
    en: "Conn's extension registry: terminal themes today, a versioned manifest and capability model as the plug-in point for reviewed integrations.",
    ko: "Conn의 확장 레지스트리: 지금은 터미널 테마를, 앞으로는 검토된 외부 연동을 붙일 수 있는 버전·권한 기반 매니페스트 모델.",
  },
  "external-automation": {
    en: "Let other tools open and drive Conn terminals through AppleScript on macOS or D-Bus on Linux, with private sessions an agent cannot see until you share them.",
    ko: "macOS의 AppleScript나 Linux의 D-Bus로 다른 도구가 Conn 터미널을 열고 조작하게 하기. 공유하기 전까지 에이전트에게 보이지 않는 비공개 세션.",
  },
  "platform-support": {
    en: "Which Conn builds exist for macOS (Apple Silicon), Windows x64 and Linux, how each is signed and updated, and what has been verified on each platform.",
    ko: "macOS(Apple Silicon), Windows x64, Linux용 Conn 빌드의 종류, 서명·업데이트 방식, 그리고 플랫폼별로 실제 검증된 범위.",
  },
};
