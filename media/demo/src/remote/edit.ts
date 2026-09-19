import timeline from "./timeline.json" with { type: "json" };

/** Editorial copy and timing for the remote-takeover film. Times are seconds on the condensed take. */
export type RemoteLanguage = "en" | "ko";
export const remoteFps = 30;
export const footageSeconds = timeline.filmSeconds;
/** The agent's reply arrives once the screen has settled; hold that last, still picture while it is shown. */
export const replySeconds = 4.6;
export const closingSeconds = 6.0;
export const remoteDuration = Math.round((footageSeconds + replySeconds + closingSeconds) * remoteFps);

type Event = { t: number; event: string; name?: string; cmd?: string; label?: string; x?: number; y?: number; text?: string };
const events = timeline.events as Event[];
const beat = (name: string) => events.find((e) => e.event === "beat" && e.name === name)!.t;
const card = (needle: string) => events.find((e) => e.event === "card" && e.cmd?.includes(needle))!.t;
const clickAfter = (t: number) => events.find((e) => e.event === "click" && e.t >= t)!.t;

export const moments = {
  login: beat("login"), join: beat("join_card"), allowed: clickAfter(beat("join_card")),
  firstCard: card("status"), rmCard: card("rm "), denied: clickAfter(card("rm ")),
  asked: events.find((e) => e.event === "agent_prompt")!.t, takeover: beat("takeover"), continued: beat("continue"), startCard: card("start"), healthCard: card("health"),
  healthApproved: clickAfter(card("health")), end: footageSeconds,
};
/** What the agent actually wrote when it finished, verbatim from the take. */
export const agentReply = events.filter((e) => e.event === "agent_turn_done").at(-1)!.text!;
export const clicks = events.filter((e) => e.event === "click").map((e) => ({ t: e.t, x: e.x!, y: e.y!, label: e.label! }));

/** A caption, or a message passed between the human and the agent client (`from`). */
export type Cue = { start: number; end: number; line: string; sub?: string; from?: "you" | "agent" };
const m = moments;
const copy = {
  en: {
    login: ["You log in.", "Your password never reaches your agent."],
    ask: ["You", "The API on staging is down. Look at my Conn terminal and get it running again."],
    join: ["It asks to join. Once.", ""],
    picks: ["It picks up the shell you are already in.", ""],
    review: ["On your server, every command waits for you.", ""],
    wants: ["It wants to delete the whole cache.", ""],
    no: ["You say no.", ""],
    type: ["Then you just type.", "Your keystroke takes the terminal back."],
    tell: ["You", "I cleared the stale lock myself, so keep the cache. Read the terminal and continue."],
    reply: ["Claude Code", agentReply],
    agentNote: "your agent, in its own app",
    worksWith: "Works with",
    reads: ["It reads what you did.", ""],
    finish: ["And finishes the job.", ""],
    headline: ["Your agent works in your terminal.", "You keep the keyboard."],
    closing: "Conn: a terminal you share with the agent you already use.",
    disclosure: "Real Conn, a real SSH login and a real Claude Code session in one continuous take. Prepared scenario; still moments shortened.",
  },
  ko: {
    login: ["로그인은 내가 합니다.", "암호는 에이전트에게 가지 않습니다."],
    ask: ["나", "staging의 API가 죽었어. 내 Conn 터미널을 보고 다시 살려 줘."],
    join: ["참여해도 되는지 묻습니다. 한 번만.", ""],
    picks: ["내가 들어와 있던 셸을 그대로 이어받습니다.", ""],
    review: ["내 서버에서는 모든 명령이 나를 기다립니다.", ""],
    wants: ["캐시를 통째로 지우겠다고 합니다.", ""],
    no: ["안 됩니다.", ""],
    type: ["그리고 그냥 직접 칩니다.", "키를 누르는 순간 터미널은 다시 내 것입니다."],
    tell: ["나", "오래된 잠금은 내가 풀었으니 캐시는 그대로 둬. 터미널을 읽고 이어서 해."],
    reply: ["Claude Code", "API가 :8080에서 다시 올라왔습니다. `./api.sh health`가 `{\"status\": \"ok\", \"cache\": \"warm\"}`을 돌려주고, 캐시는 그대로이며, 제어권은 반납했습니다."],
    agentNote: "내가 쓰는 에이전트, 별도의 앱",
    worksWith: "함께 쓰는 에이전트",
    reads: ["내가 한 일을 읽습니다.", ""],
    finish: ["그리고 작업을 끝냅니다.", ""],
    headline: ["에이전트는 내 터미널에서 일하고,", "키보드는 내가 쥡니다."],
    closing: "Conn: 지금 쓰는 에이전트와 함께 쓰는 터미널.",
    disclosure: "실제 Conn, 실제 SSH 로그인, 실제 Claude Code 세션을 한 번에 이어서 촬영했습니다. 준비된 시나리오이며 화면이 멈춘 구간만 줄였습니다.",
  },
} as const;

export const remoteCopy = copy;
export function cues(language: RemoteLanguage): Cue[] {
  const c = copy[language];
  const say = (start: number, end: number, pair: readonly [string, string]): Cue => ({ start, end, line: pair[0], sub: pair[1] || undefined });
  const message = (start: number, end: number, pair: readonly [string, string], from: "you" | "agent"): Cue => ({ start, end, line: pair[1], sub: pair[0], from });
  return [
    say(0.3, m.asked - 1.2, c.login),
    message(m.asked - 1.2, m.asked + 2.3, c.ask, "you"),
    say(m.asked + 2.3, m.allowed + 0.9, c.join),
    say(m.allowed + 0.9, m.firstCard - 0.2, c.picks),
    say(m.firstCard - 0.2, m.rmCard - 0.3, c.review),
    say(m.rmCard - 0.3, m.denied - 0.1, c.wants),
    say(m.denied - 0.1, m.takeover + 0.2, c.no),
    say(m.takeover + 0.2, m.continued - 0.1, c.type),
    message(m.continued - 0.1, m.continued + 3.6, c.tell, "you"),
    say(m.continued + 3.6, m.healthCard - 0.2, c.reads),
    say(m.healthCard - 0.2, m.end + 0.2, c.finish),
    message(m.end + 0.2, m.end + replySeconds, c.reply, "agent"),
  ];
}

/** Camera: scale and the point of the recording (0..1) kept at the centre of the window. */
export type Shot = { t: number; scale: number; x: number; y: number };
export const camera: Shot[] = [
  // Open close on the login, then hold the whole window: nothing is cropped while the story unfolds.
  { t: 0, scale: 1.5, x: 0.28, y: 0.28 },
  { t: m.join - 1.5, scale: 1.5, x: 0.28, y: 0.28 },
  { t: m.join + 0.1, scale: 1, x: 0.5, y: 0.5 },
  { t: m.end, scale: 1, x: 0.5, y: 0.5 },
];

/** The 15-second edition: three moments of the same take, joined with dissolves, then the closing card. */
export const shortParts = [
  { from: 0.5, to: 4.9 },
  { from: m.rmCard - 0.5, to: m.takeover + 3.1 },
] as const;
export const shortClosingSeconds = 3.6;
export const shortDissolve = 0.4;
export const shortDuration = Math.round((shortParts.reduce((sum, p) => sum + (p.to - p.from), 0) + shortClosingSeconds) * remoteFps);
