import type { createBridge } from './bridge';
import type { Lifetime } from '../runtime/lifetime';
export type LiveAgent = { conn: number; agentId: string; affordances: string[] };

/** One poll loop per app, alive only while a component watches the current session. */
export function createLiveAgents(bridge: ReturnType<typeof createBridge>, activeSession: () => string, lifetime: Lifetime) {
  const liveAgents = $state<{ list: LiveAgent[] }>({ list: [] });
  let watchers = 0;
  let timer: ReturnType<typeof setInterval> | undefined;
  let revision = 0;
  async function poll() {
    const session = activeSession();
    const current = ++revision;
    try {
      const list = await bridge.cmd<LiveAgent[]>('agents', {session});
      if (!lifetime.disposed && watchers && current === revision && session === activeSession()) liveAgents.list = list;
    } catch { if (!lifetime.disposed && current === revision) liveAgents.list = []; }
  }
  function watchLiveAgents() {
    if (lifetime.disposed) return () => {};
    if (!watchers++) timer = setInterval(poll, 1500);
    void poll();
    let stopped = false;
    return () => {
      if (stopped || lifetime.disposed) return;
      stopped = true;
      if (!--watchers) { revision++; clearInterval(timer); liveAgents.list = []; }
    };
  }
  lifetime.own(() => { revision++; clearInterval(timer); watchers = 0; liveAgents.list = []; });
  return { liveAgents, watchLiveAgents };
}
