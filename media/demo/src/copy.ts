import type { ClipId } from "./footage";

export type Language = "en" | "ko";
export type SceneCopy = { title: string; caption: string };
export const copy: Record<Language, Record<ClipId, SceneCopy>> = {
  en: {
    intro: { title: "Your agent. One shared terminal.",
      caption: "Ask Codex to help with the failing test already on your screen." },
    request: { title: "Start from the same context.",
      caption: "Codex reads the shared terminal and asks to work in it." },
    fix: { title: "Follow the work as it happens.",
      caption: "The agent fixes the discount calculation and runs the tests." },
    reclaim: { title: "Add what the agent missed.",
      caption: "Take back input control. Add a test: a discount must never make the price negative." },
    resume: { title: "Pick up where each other leaves off.",
      caption: "Codex reads your new test, updates the fix, and checks all three cases." },
    timeline: { title: "Keep the collaboration in view.",
      caption: "Revisit commands, handoffs, and the requests behind them." },
  },
  ko: {
    intro: { title: "쓰던 에이전트와, 하나의 터미널에서.",
      caption: "지금 화면에서 실패한 테스트를 Codex에게 함께 고치자고 합니다." },
    request: { title: "같은 상황에서 시작합니다.",
      caption: "Codex가 공유 터미널을 읽고, 작업을 이어갈 제어권을 요청합니다." },
    fix: { title: "작업 과정을 함께 봅니다.",
      caption: "에이전트가 할인 계산을 수정한 뒤 테스트를 실행합니다." },
    reclaim: { title: "빠진 조건은 내가 더합니다.",
      caption: "직접 이어받아 테스트를 더합니다. 할인해도 가격은 음수가 되면 안 됩니다." },
    resume: { title: "서로의 작업을 이어갑니다.",
      caption: "Codex가 내가 추가한 테스트를 읽고, 수정한 뒤 세 조건을 모두 검증합니다." },
    timeline: { title: "함께한 과정까지 남습니다.",
      caption: "명령과 제어권 전환, 그때의 요청을 타임라인에서 다시 살펴봅니다." },
  },
};

export const closingCopy: Record<Language, { title: string; disclosure: string }> = {
  en: { title: "Your agent. One shared terminal.", disclosure: "Real Codex CLI + Conn · Human role automated · Pauses shortened" },
  ko: { title: "쓰던 에이전트와, 하나의 터미널에서.", disclosure: "실제 Codex CLI + Conn · 사람 역할은 자동 조작 · 대기 시간 단축" },
};
