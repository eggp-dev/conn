import { cmd } from "./bridge";

export type LiveAgent = { conn: number; agentId: string; affordances: string[] };
/** Agents connected to the active tab. One poll loop, alive only while a view watches. */
export const liveAgents = $state<{ list: LiveAgent[] }>({ list: [] });
let watchers = 0;
let timer: ReturnType<typeof setInterval> | undefined;
async function poll() { try { liveAgents.list = await cmd<LiveAgent[]>("agents"); } catch { liveAgents.list = []; } }
/** Call from an `$effect` that tracks `st.active`; returns its cleanup. */
export function watchLiveAgents() {
  if (!watchers++) timer = setInterval(poll, 1500);
  void poll();
  return () => { if (!--watchers) { clearInterval(timer); liveAgents.list = []; } };
}
