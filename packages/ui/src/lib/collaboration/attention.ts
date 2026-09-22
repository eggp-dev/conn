import type { ConnPorts } from '../../runtime/ports';
import type { Lifetime } from '../../runtime/lifetime';
/** Inactivity changes delivery only, never session access. Dedupe belongs to the mounted app. */
export function createAttention(host: ConnPorts['attention'], lifetime: Lifetime) {
  let previous = new Set<string>();
  let lastSignal = 0;
  let revision = 0;
  lifetime.own(() => { revision++; previous.clear(); void host.request(false).catch(() => {}); });
  async function signalPending(keys: string[]) {
    if (lifetime.disposed) return;
    const current = ++revision;
    const next = new Set(keys), fresh = keys.some(key => !previous.has(key)); previous = next;
    document.title = keys.length ? `(${keys.length}) Conn` : 'Conn';
    if (!keys.length) { await host.request(false); return; }
    if (!fresh || Date.now() - lastSignal < 10000 || await host.isFocused() || current !== revision || lifetime.disposed) return;
    lastSignal = Date.now();
    await host.request(true);
  }
  return { signalPending };
}
