import { newTimeline, type TimelineState } from "./timeline";
import type { Mode, Pacing } from "./bridge";
import type { ThemeId } from "./themes";

export type Toast = { id: number; text: string; cls: string };
export type Target = { path: string; exists: boolean; isDir: boolean; gitRepo: boolean; entries: number | null; protected: boolean };
export type SegmentVerdict = { text: string; command: string | null; opaque: string | null; policy: "allow" | "confirm" | "deny"; label?: string; targets: Target[] };
export type Analysis = { cwd: string | null; segments: SegmentVerdict[]; policy: "allow" | "confirm" | "deny"; label?: string; isolationViolation: boolean };


/** Everything that belongs to one session (tab). */
export type TabState = {
  id: string;
  title: string;
  /** Privacy is resolved before the terminal subscribes to output. */
  statusReady: boolean;
  shellIntegration: { state: string; shell?: string; reason?: string };
  externalPrivate: boolean;
  externalStarting: boolean;
  externalInputAvailable: boolean;
  profileId: string | null;
  profileName: string | null;
  reviewRequired: boolean;
  processAlive: boolean;
  policyBlockedUntil: number;
  attended: boolean;
  entrustedTo: string | null;
  attention: null | { agentId: string; reason?: string };
  /** The agent that opened this tab, if an agent did. */
  openedBy: string | null;
  controller: { type: "human" | "agent"; agentId?: string };
  mode: Mode;
  effectiveMode: Mode;
  gate: boolean;
  pacing: Pacing;
  mask: string[] | null;
  allows: string[];
  agents: string[];
  typing: boolean;
  proposal: null | { id: string; agentId: string; text: string; ready: boolean; intent?: string };
  approval: null | { id: string; agentId: string; cmd: string; label: string; at: number; intent?: string; analysis?: Analysis };
  grace: null | { execId: string; cmd: string; ms: number; start: number; intent?: string };
  ctlReq: null | { id: string; agentId: string; reason?: string; originalRequest?: import("./timeline").OriginalRequest };
  lastAgent: null | { agentId: string; lastCmd?: string; connected: boolean };
  handback: boolean;
  timeline: TimelineState;
  cursor: { x: number; y: number; w: number; h: number; row: number };
  wash: null | { x: number; y: number; color: string; out: boolean; key: number };
  /** Set while the human is away and an agent held the conn: paused badge. */
  paused: boolean;
};

export function newTab(id: string, n: number): TabState {
  return {
    id, title: `Terminal ${n}`, statusReady: false, shellIntegration: { state: "unavailable" }, externalPrivate: false, externalStarting: false, externalInputAvailable: false, profileId: null, profileName: null, reviewRequired: false, processAlive: true, policyBlockedUntil: 0, attended: false, entrustedTo: null, attention: null, openedBy: null,
    controller: { type: "human" }, mode: "autopilot", effectiveMode: "autopilot", gate: false,
    pacing: { minWriteIntervalMs: 0, enterGraceMs: 0, leaseTtlSecs: 60, approvalTtlSecs: 300 },
    mask: null, allows: [], agents: [], typing: false, proposal: null, approval: null, grace: null, ctlReq: null,
    lastAgent: null, handback: false, timeline: newTimeline(), cursor: { x: 0, y: 0, w: 8, h: 16, row: 0 }, wash: null, paused: false,
  };
}

/** Island announcement: a short message shown in the centre, then collapsed to the dot. */
export type Announcement = { id: number; text: string; detail?: string; ms: number; color: string; kind: "conn" | "approval" | "attention" | "info" | "warn" };

export const st = $state({
  booted: false,
  externalPending: false,
  backendOnline: false,
  socket: "",
  shell: "",
  tabs: {} as Record<string, TabState>,
  order: [] as string[],
  active: "" as string,
  tabSeq: 0,
  toasts: [] as Toast[],
  announcement: null as Announcement | null,
  leaveChip: null as null | { session: string; agentId: string; until: number },
  theme: (localStorage.getItem("ss:theme") as ThemeId) || "midnight",
  followSystem: localStorage.getItem("ss:followSystem") === "1",
  effects: localStorage.getItem("ss:effects") !== "0",
  fontSize: Number(localStorage.getItem("ss:fontSize") || 13),
  paletteOpen: false,
  centerOpen: false,
  settingsOpen: false,
  timelineOpen: false,
  settingsTab: "agents" as "profiles" | "agents" | "automation" | "pacing" | "policy" | "appearance" | "diagnostics",
});

/** The tab the human is looking at. Safe to call before boot (returns a placeholder). */
const placeholder = newTab("", 0);
export function cur(): TabState {
  return st.tabs[st.active] ?? placeholder;
}
export function tab(id: string): TabState | undefined {
  return st.tabs[id];
}
export function tabIndex(id: string): number {
  return st.order.indexOf(id) + 1;
}

let toastSeq = 0;
export function toast(text: string, cls = "") {
  const id = ++toastSeq;
  st.toasts.push({ id, text, cls });
  setTimeout(() => { st.toasts = st.toasts.filter((t) => t.id !== id); }, 4000);
}

let annSeq = 0;
let annTimer: number | null = null;
export function announce(text: string, color: string, kind: Announcement["kind"] = "info", ms = 2600, detail?: string) {
  const id = ++annSeq;
  st.announcement = { id, text, detail: detail?.trim() || undefined, ms, color, kind };
  if (annTimer) clearTimeout(annTimer);
  annTimer = window.setTimeout(() => { if (st.announcement?.id === id) st.announcement = null; }, ms);
}
