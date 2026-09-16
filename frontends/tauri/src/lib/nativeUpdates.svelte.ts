import { invoke as nativeInvoke } from '@tauri-apps/api/core';
import { invoke } from './transport';
import { checkRelease, type Platform } from './updates';

export type UpdateStatus = { phase: string; current: string; version?: string; notes: string; preview: boolean; supported: boolean; downloaded: number; total?: number; error?: string };
export const updater = $state({ status: { phase: 'idle', current: '', notes: '', preview: false, supported: false, downloaded: 0 } as UpdateStatus, busy: false, previews: false, automatic: true });
const native = import.meta.env.MODE !== 'browser-test';
let initialized = false;
export async function initUpdates() {
  if (initialized) return;
  initialized = true;
  try {
    if (native) updater.status = await nativeInvoke('app_update', { action: 'status' });
    else { const p = await invoke<Platform>('update_info'); updater.status.current = p.version; }
    updater.previews = localStorage.getItem('conn:update-previews') === 'true' || (localStorage.getItem('conn:update-previews') === null && updater.status.preview);
    updater.automatic = localStorage.getItem('conn:auto-update') !== 'false';
  } catch { initialized = false; }
}
export function updatePreference(kind: 'previews' | 'automatic', value: boolean) {
  updater[kind] = value;
  localStorage.setItem(kind === 'previews' ? 'conn:update-previews' : 'conn:auto-update', String(value));
}
export async function updateAction(action: 'check' | 'download' | 'install', confirmed = false) {
  if (updater.busy) return;
  updater.busy = true;
  updater.status.error = undefined;
  updater.status.phase = action === 'check' ? 'checking' : action === 'download' ? 'downloading' : 'installing';
  let finished = false;
  let poll: ReturnType<typeof setInterval> | undefined;
  try {
    if (native) {
      poll = setInterval(async () => { try { const status = await nativeInvoke<UpdateStatus>('app_update', { action: 'status' }); if (!finished) updater.status = status; } catch {} }, 500);
      updater.status = await nativeInvoke('app_update', { action, previews: updater.previews, confirmed });
    } else if (action === 'check') {
      const controller = new AbortController(); const timeout = setTimeout(() => controller.abort(), 20000);
      try {
        const platform = await invoke<Platform>('update_info');
        const release = await checkRelease(platform, updater.previews, controller.signal);
        updater.status = { ...updater.status, current: platform.version, phase: release ? 'available' : 'current', version: release?.tag.replace(/^v/, ''), notes: release?.notes ?? '', preview: release?.preview ?? false, supported: false };
      } finally { clearTimeout(timeout); }
    }
  } catch (error) { updater.status.phase = 'error'; updater.status.error = String(error); }
  finally { finished = true; if (poll) clearInterval(poll); updater.busy = false; }
}
export function startAutoUpdates() {
  // The browser harness and external automation windows never initiate updates.
  if (!native) return () => {};
  let stopped = false;
  const run = async () => {
    await initUpdates();
    if (stopped || !initialized || !updater.automatic || updater.busy || ['ready','installing'].includes(updater.status.phase)) return;
    await updateAction('check');
    if (!stopped && updater.automatic && updater.status.supported && updater.status.phase === 'available') await updateAction('download');
  };
  const first = setTimeout(run, 30000);
  const repeat = setInterval(run, 6 * 60 * 60 * 1000);
  return () => { stopped = true; clearTimeout(first); clearInterval(repeat); };
}
