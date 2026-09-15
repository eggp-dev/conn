import { invoke } from "./transport";
import { listen } from "./transport";
import { st } from "./store.svelte";

export type Pacing = { minWriteIntervalMs: number; enterGraceMs: number; leaseTtlSecs: number; approvalTtlSecs: number };
export type Mode = "observe" | "copilot" | "autopilot";
export type Ev = { event: string; session?: string; [k: string]: any };

/** Invoke a command against the active tab unless `session` is given. */
export const cmd = <T = any>(name: string, args: Record<string, unknown> = {}) =>
  invoke<T>(name, { session: st.active, ...args });
export const log = (msg: string) => invoke("log", { msg }).catch(() => {});
export const onEvent = (h: (ev: Ev) => void) => listen<Ev>("ss:event", (e) => h(e.payload));
export const onTabOpened = (h: (p: { session: string; agentId: string; reason?: string | null }) => void) => listen<{ session: string; agentId: string; reason?: string | null }>("ss:tab_opened", (e) => h(e.payload));
export const onOutput = (h: (p: { session: string; data: string }) => void) => listen<{ session: string; data: string }>("ss:output", (e) => h(e.payload));
export const b64ToBytes = (s: string) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));
