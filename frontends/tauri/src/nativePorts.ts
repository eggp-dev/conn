import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow, UserAttentionType } from '@tauri-apps/api/window';
import { OWNER_PROTOCOL, type ConnPorts, type UpdateStatus, type OwnerHello, type OwnerRequestMeta } from '@conn/ui/runtime';

/** Native APIs stay at this entry point; all state and scheduling lives in @conn/ui. */
export async function createNativePorts(onDisconnect: (cause: unknown, uncertain?: boolean) => void): Promise<ConnPorts> {
  let attachment = await invoke<OwnerHello>('owner_attach', { protocol: OWNER_PROTOCOL, takeover: true });
  let stopped = false;
  const connectionListeners = new Set<(event: { payload: { connected: boolean; epoch: number; runtimeId: string } }) => void>();
  const signal = (connected: boolean) => { for (const listener of connectionListeners) listener({ payload: { connected, epoch: attachment.epoch, runtimeId: attachment.runtimeId } }); };
  const window = getCurrentWindow();
  const transport = {
    async invoke<T = any>(name: string, args: Record<string, unknown> = {}, meta?: OwnerRequestMeta): Promise<T> {
      if (stopped) throw new Error('Native connection disposed');
      if (!meta) throw new Error('Owner operations must use the common Conn client');
      try { return await invoke<T>('dispatch', { name, args, meta: { ...meta, epoch: attachment.epoch } }); }
      catch (cause) {
        // A retired view must ask the user to reconnect, never take ownership in a retry loop.
        if (!stopped && String(cause) === 'attachment_fenced') { signal(false); onDisconnect(cause); }
        throw cause;
      }
    },
    async listen<T>(name: string, handler: (event: { payload: T }) => void): Promise<() => void> {
      if (name === 'ss:connection') {
        const listener = handler as (event: { payload: { connected: boolean; epoch: number; runtimeId: string } }) => void;
        connectionListeners.add(listener);
        queueMicrotask(() => { if (!stopped && connectionListeners.has(listener)) listener({ payload: { connected: true, epoch: attachment.epoch, runtimeId: attachment.runtimeId } }); });
        return () => { connectionListeners.delete(listener); };
      }
      return listen<T>(name, handler, { target: { kind: 'WebviewWindow', label: getCurrentWebviewWindow().label } });
    },
    reset(cause: unknown) { if (!stopped) { stopped = true; signal(false); onDisconnect(cause, true); } },
    dispose() { stopped = true; connectionListeners.clear(); },
  };
  return {
    transport,
    attention: { isFocused: () => window.isFocused(), request: active => window.requestUserAttention(active ? UserAttentionType.Informational : null) },
    updates: { supported: true, invoke: (action, options = {}) => invoke<UpdateStatus>('app_update', { action, ...options }) },
    storage: localStorage,
    platform: /Mac/.test(navigator.platform) ? 'macos' : /Win/.test(navigator.platform) ? 'windows' : 'linux',
  };
}
