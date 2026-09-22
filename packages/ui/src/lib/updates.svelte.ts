import type { ConnPorts, UpdateStatus } from '../runtime/ports';
import type { Lifetime } from '../runtime/lifetime';
export type { UpdateStatus } from '../runtime/ports';

/** Update state belongs to this app; host support is explicit and never simulated as success. */
export function createUpdates(host: ConnPorts['updates'], storage: NonNullable<ConnPorts['storage']>, lifetime: Lifetime) {
  const updater = $state({ status: { phase: host.supported ? 'idle' : 'unsupported', current: '', notes: '', preview: false, supported: host.supported, downloaded: 0 } as UpdateStatus, busy: false, previews: false, automatic: true });
  let initialized = false;
  async function initUpdates() {
    if (initialized || lifetime.disposed) return;
    initialized = true;
    try {
      const status = await host.invoke('status');
      if (lifetime.disposed) return;
      updater.status = host.supported ? status : { ...status, phase: 'unsupported', supported: false };
      updater.previews = storage.getItem('conn:update-previews') === 'true' || (storage.getItem('conn:update-previews') === null && updater.status.preview);
      updater.automatic = storage.getItem('conn:auto-update') !== 'false';
    } catch (error) {
      if (lifetime.disposed) return;
      initialized = false;
      updater.status.error = String(error);
    }
  }
  function updatePreference(kind: 'previews' | 'automatic', value: boolean) {
    updater[kind] = value;
    storage.setItem(kind === 'previews' ? 'conn:update-previews' : 'conn:auto-update', String(value));
  }
  async function updateAction(action: 'check' | 'download' | 'install', confirmed = false) {
    if (updater.busy || lifetime.disposed || !host.supported) return;
    updater.busy = true;
    updater.status.error = undefined;
    updater.status.phase = action === 'check' ? 'checking' : action === 'download' ? 'downloading' : 'installing';
    let finished = false;
    const poll = window.setInterval(async () => {
      try { const status = await host.invoke('status'); if (!finished && !lifetime.disposed) updater.status = status; } catch {}
    }, 500);
    const stop = lifetime.own(() => { finished = true; clearInterval(poll); });
    try {
      const status = await host.invoke(action, { previews: updater.previews, confirmed });
      if (!lifetime.disposed) updater.status = status;
    } catch (error) {
      if (!lifetime.disposed) { updater.status.phase = 'error'; updater.status.error = String(error); }
    } finally { stop(); if (!lifetime.disposed) updater.busy = false; }
  }
  function startAutoUpdates() {
    if (!host.supported) return () => {};
    let stopped = false;
    const run = async () => {
      await initUpdates();
      if (stopped || lifetime.disposed || !initialized || !updater.automatic || updater.busy || ['ready', 'installing'].includes(updater.status.phase)) return;
      await updateAction('check');
      if (!stopped && !lifetime.disposed && updater.automatic && updater.status.supported && updater.status.phase === 'available') await updateAction('download');
    };
    const first = window.setTimeout(run, 30000);
    const repeat = window.setInterval(run, 6 * 60 * 60 * 1000);
    return lifetime.own(() => { stopped = true; clearTimeout(first); clearInterval(repeat); });
  }
  return { updater, initUpdates, updateAction, updatePreference, startAutoUpdates, updateCapability: host };
}
