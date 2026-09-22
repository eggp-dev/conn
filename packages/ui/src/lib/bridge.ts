import type { ConnClient } from '../runtime/client';
import type { createStore } from './store.svelte';
import type { createI18n } from './i18n.svelte';
import type { AdmissionChange } from "./collaboration/contracts";
import type { SessionEvent } from "./collaboration/events";

export type Pacing = { minWriteIntervalMs: number; enterGraceMs: number; leaseTtlSecs: number; approvalTtlSecs: number };
export type Mode = "observe" | "copilot" | "autopilot";
export type Ev = SessionEvent;
export const TOOLS = ["snapshot", "request_control", "type", "send_key", "interrupt", "release_control", "check_approval", "request_attention", "open_tab", "switch_tab"];
/** i18n key and affordance mask (`null` = every tool), shared by the palette and settings. */
export const TOOL_PRESETS: [string, string[] | null][] = [
  ["s.preset.observe", ["snapshot", "request_attention", "check_approval", "switch_tab"]],
  ["s.preset.notabs", TOOLS.filter((a) => a !== "open_tab")],
  ["s.preset.all", null],
];

export type OutputFrame = { epoch: number; streamSeq: number; reset?: boolean; session: string; data: string; outputSeq: number; generation: number; size: { rows: number; cols: number } };
export const b64ToBytes = (s: string) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));

export function createBridge(client: ConnClient, store: ReturnType<typeof createStore>, language: ReturnType<typeof createI18n>) {
  const {invoke, listen} = client;
  const {st, toast} = store;
  const {t} = language;
  /** Invoke a command against the active tab unless `session` is given. */
  const cmd = <T = any>(name: string, args: Record<string, unknown> = {}) =>
    invoke<T>(name, { session: st.active, ...args });
  /** A rejected mode change must remain visible and must not permit a following grant. */
  async function changeMode(mode: Mode, session = st.active): Promise<boolean> {
    try { await cmd("set_mode", { mode, session }); return true; }
    catch (e) {
      const message = String(e);
      toast(message.includes("input_pending") || message.includes("shell input is pending") ? t("mode.input_pending") : message, "warn");
      return false;
    }
  }
  const log = (msg: string) => invoke("log", { msg }).catch(() => {});
  const onEvent = (h: (ev: Ev) => void) => listen<Ev>("ss:event", (e) => h(e.payload));
  const onAdmission = (h: (p: AdmissionChange) => void) => listen<AdmissionChange>("ss:admission", (e) => h(e.payload));
  const onTabOpened = (h: (p: { session: string; agentId: string; reason?: string | null; focus?: boolean }) => void) => listen<{ session: string; agentId: string; reason?: string | null; focus?: boolean }>("ss:tab_opened", (e) => h(e.payload));
  const onOutput = (h: (p: OutputFrame) => void) => listen<OutputFrame>("ss:output", (e) => h(e.payload));

  return { cmd, changeMode, log, onEvent, onAdmission, onTabOpened, onOutput };
}
