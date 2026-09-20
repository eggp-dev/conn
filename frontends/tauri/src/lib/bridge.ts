import { invoke } from "./transport";
import { listen } from "./transport";
import { st, toast } from "./store.svelte";
import { t } from "./i18n.svelte";

export type Pacing = { minWriteIntervalMs: number; enterGraceMs: number; leaseTtlSecs: number; approvalTtlSecs: number };
export type Mode = "observe" | "copilot" | "autopilot";
export type Ev = { event: string; session?: string; [k: string]: any };
export const TOOLS = ["snapshot", "request_control", "type", "send_key", "interrupt", "release_control", "check_approval", "request_attention", "open_tab", "switch_tab"];
/** i18n key and affordance mask (`null` = every tool), shared by the palette and settings. */
export const TOOL_PRESETS: [string, string[] | null][] = [
  ["s.preset.observe", ["snapshot", "request_attention", "check_approval", "switch_tab"]],
  ["s.preset.notabs", TOOLS.filter((a) => a !== "open_tab")],
  ["s.preset.all", null],
];

/** Invoke a command against the active tab unless `session` is given. */
export const cmd = <T = any>(name: string, args: Record<string, unknown> = {}) =>
  invoke<T>(name, { session: st.active, ...args });
/** A rejected mode change must remain visible and must not permit a following grant. */
export async function changeMode(mode: Mode, session = st.active): Promise<boolean> {
  try { await cmd("set_mode", { mode, session }); return true; }
  catch (e) {
    const message = String(e);
    toast(message.includes("input_pending") || message.includes("shell input is pending") ? t("mode.input_pending") : message, "warn");
    return false;
  }
}
export const log = (msg: string) => invoke("log", { msg }).catch(() => {});
export const onEvent = (h: (ev: Ev) => void) => listen<Ev>("ss:event", (e) => h(e.payload));
export const onAdmission = (h: (p: { connId: number; agentId: string; state: "pending" | "granted" | "denied" | "closed" }) => void) => listen<{ connId: number; agentId: string; state: "pending" | "granted" | "denied" | "closed" }>("ss:admission", (e) => h(e.payload));
export const onTabOpened = (h: (p: { session: string; agentId: string; reason?: string | null; focus?: boolean }) => void) => listen<{ session: string; agentId: string; reason?: string | null; focus?: boolean }>("ss:tab_opened", (e) => h(e.payload));
export const onOutput = (h: (p: { session: string; data: string; outputSeq: number; generation: number }) => void) => listen<{ session: string; data: string; outputSeq: number; generation: number }>("ss:output", (e) => h(e.payload));
export const b64ToBytes = (s: string) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));
