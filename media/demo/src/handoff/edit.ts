/**
 * Fresh, real Codex CLI + Conn recordings. Editorial text lives only in the
 * reserved gutters. No terminal output or application UI is recreated here.
 *
 * Source contract: 1920 × 1080, Codex left / Conn right, 72px top and 80px
 * bottom gutters. handoff.mp4 preserves the order of human correction,
 * refreshed observation and agent continuation. Any removed waiting is
 * documented in the accompanying edit map and disclosed in the closing card.
 */
export const handoffFps = 30;
export const handoffDuration = 70 * handoffFps;
export const shortDuration = 15 * handoffFps;
export const handoffClips = {
  request: { file: "footage/handoff/request.mp4", duration: 19, trim: 0 },
  handoff: { file: "footage/handoff/handoff.mp4", duration: 30, trim: 0 },
  result: { file: "footage/handoff/result.mp4", duration: 9, trim: 0 },
} as const;

// These offsets select actual footage; change them only against the edit map.
export const hookTrim = 3;
export const shortTrim = 0;

export type HandoffLanguage = "en" | "ko";
export type CaptionCue = { start: number; end: number; title: string; caption: string };

export const handoffCopy: Record<HandoffLanguage, {
  main: CaptionCue[];
  short: CaptionCue[];
  headline: string[];
  closing: string;
  disclosure: string;
  poster: string[];
  posterDetail: string;
  watch: string;
}> = {
  en: {
    main: [
      { start: 0, end: 6, title: "A small correction. Keep going.", caption: "“I changed the directory. Read the terminal and continue here.”" },
      { start: 6, end: 12, title: "A moment earlier", caption: "Ask your existing agent to work in the shared terminal." },
      { start: 12, end: 25, title: "Your agent, in your terminal.", caption: "Codex checks the draft folder, then waits before writing." },
      { start: 25, end: 39, title: "Take a turn.", caption: "Type in Conn to take control. Change the folder yourself." },
      { start: 39, end: 55, title: "Continue from what you changed.", caption: "Codex reads the terminal again and works in the new folder." },
      { start: 55, end: 64, title: "The result, where you wanted it.", caption: "The file is in workspace. Nothing was written in draft." },
    ],
    short: [
      { start: 0, end: 6, title: "I changed the directory.", caption: "Take control by typing in Conn." },
      { start: 6, end: 12, title: "Continue from here.", caption: "Your agent reads the change and keeps going." },
    ],
    headline: ["Keep your agent.", "Share your terminal."],
    closing: "Download Conn · macOS, Windows, Linux",
    disclosure: "Real Codex CLI + Conn · Human role automated · Waiting shortened",
    poster: ["Your agent works.", "You step in.", "Keep going."],
    posterDetail: "One shared terminal. A correction that becomes the next step.",
    watch: "Watch the collaboration",
  },
  ko: {
    main: [
      { start: 0, end: 6, title: "직접 고치고, 함께 이어갑니다.", caption: "“경로는 내가 고쳤어. 터미널을 다시 읽고 여기서 이어가.”" },
      { start: 6, end: 12, title: "그 직전", caption: "쓰던 에이전트에게 같은 터미널에서 작업을 부탁합니다." },
      { start: 12, end: 25, title: "내 터미널에서 일하는 에이전트.", caption: "Codex가 draft 폴더를 확인하고, 쓰기 전에 기다립니다." },
      { start: 25, end: 39, title: "이번에는 내 차례.", caption: "Conn에 직접 입력해 제어권을 가져오고, 폴더를 바꿉니다." },
      { start: 39, end: 55, title: "내가 바꾼 곳에서 이어갑니다.", caption: "Codex가 터미널을 다시 읽고, 새 폴더에서 작업합니다." },
      { start: 55, end: 64, title: "원하던 위치에, 원하는 결과.", caption: "파일은 workspace에 생겼고, draft에는 만들지 않았습니다." },
    ],
    short: [
      { start: 0, end: 6, title: "경로는 내가 고쳤어.", caption: "Conn에 직접 입력하면 제어권을 가져옵니다." },
      { start: 6, end: 12, title: "여기서 이어가.", caption: "에이전트가 바뀐 화면을 읽고, 작업을 이어갑니다." },
    ],
    headline: ["쓰던 에이전트와,", "같은 터미널에서."],
    closing: "Conn 다운로드 · macOS, Windows, Linux",
    disclosure: "실제 Codex CLI + Conn · 사람 역할 자동 조작 · 대기 시간 단축",
    poster: ["에이전트가 일하고,", "내가 직접 고치고,", "함께 이어갑니다."],
    posterDetail: "같은 터미널에서.\n내가 바꾼 곳부터 이어가세요.",
    watch: "협업 영상 보기",
  },
};
