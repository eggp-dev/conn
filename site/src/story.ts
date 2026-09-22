// The scroll story: one recorded take, walked through beat by beat. Times are seconds in
// public/story/take.mp4; scripts/encode-story.sh puts a keyframe on every one of them, so change
// both together. Each beat plays from `start` at its recorded speed and rests on `hold`.
import type { Lang } from "./i18n/landing";

export const beats = [
  { key: "login", start: 0, hold: 5.2 },
  { key: "join", start: 5.6, hold: 9.2 },
  { key: "review", start: 9.4, hold: 15.9 },
  { key: "proposal", start: 16.3, hold: 25.9 },
  { key: "takeover", start: 26.0, hold: 29.8 },
  { key: "done", start: 37.4, hold: 46.6 },
] as const;

/** The beat that waits for the visitor's own key, and the one that key leads to. */
export const proposalBeat = 3;
export const takeoverBeat = 4;

type Line = readonly [title: string, sub: string];
type StoryCopy = {
  heading: string;
  label: string;
  rail: readonly string[];
  lines: readonly Line[];
  /** Shown instead of the takeover line when the visitor pressed a key to get there. */
  tookOver: Line;
  hintKey: string;
  hintTouch: string;
  note: string;
};

// The first lines are the film's own captions (media/demo/src/remote/edit.ts). In the take the human
// clicks Deny and then types; in the product a keystroke alone also denies a pending approval.
export const storyCopy: Record<Lang, StoryCopy> = {
  en: {
    heading: "Scroll through one real session.",
    label: "One recorded session, beat by beat",
    rail: ["Login", "Join", "Review", "Proposal", "Takeover", "Done"],
    lines: [
      ["You log in.", "Your password never reaches your agent."],
      ["It asks to join. Once.", "Until you allow it, it learns nothing about your sessions."],
      ["Risky commands on your server wait for you.", "You see the exact line and what the agent says it is for."],
      ["It wants to delete the whole cache.", "The card is waiting. So is the keyboard."],
      ["You say no. Then you just type.", "Your keystroke takes the terminal back."],
      ["It reads what you did, and finishes the job.", "The cache is intact, control is back with you, and there is a record."],
    ],
    tookOver: ["You took the keyboard back.", "In Conn it is the same: one key, and the terminal is yours."],
    hintKey: "Press any key to take the keyboard back",
    hintTouch: "Tap the terminal to take the keyboard back",
    note: "The same take as the film above, at its recorded speed. Only the size was changed.",
  },
  ko: {
    heading: "실제 세션 하나를 스크롤로 따라가 보세요.",
    label: "촬영된 세션 하나를 장면별로",
    rail: ["로그인", "참여", "검토", "제안", "회수", "완료"],
    lines: [
      ["로그인은 내가 합니다.", "암호는 에이전트에게 가지 않습니다."],
      ["참여해도 되는지 묻습니다. 한 번만.", "허용하기 전에는 세션에 대해 아무것도 알 수 없습니다."],
      ["내 서버의 위험한 명령은 나를 기다립니다.", "정확한 명령 줄과 에이전트가 밝힌 의도를 보고 결정합니다."],
      ["캐시를 통째로 지우겠다고 합니다.", "카드가 기다리고 있습니다. 키보드도요."],
      ["안 됩니다. 그리고 그냥 직접 칩니다.", "키를 누르는 순간 터미널은 다시 내 것입니다."],
      ["내가 한 일을 읽고, 작업을 끝냅니다.", "캐시는 그대로이고, 제어권은 내게 돌아왔으며, 기록이 남습니다."],
    ],
    tookOver: ["키보드를 되찾았습니다.", "Conn에서도 똑같습니다. 키 하나면 터미널은 다시 내 것입니다."],
    hintKey: "아무 키나 눌러 키보드를 되찾으세요",
    hintTouch: "터미널을 탭해 키보드를 되찾으세요",
    note: "위 영상과 같은 테이크이며 촬영된 속도 그대로입니다. 크기만 바꿨습니다.",
  },
};
