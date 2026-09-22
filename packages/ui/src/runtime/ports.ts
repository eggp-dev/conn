import type { OwnerMeta } from './generated';
export { OWNER_PROTOCOL } from './generated';
export type { OwnerHello, OwnerOutput, OwnerRequest } from './generated';
/** Host capabilities injected by the entry point. No platform implementation lives in the UI. */
export type UpdateStatus = { phase: string; current: string; version?: string; notes: string; preview: boolean; supported: boolean; downloaded: number; total?: number; error?: string };
export type OwnerRequestMeta = Omit<OwnerMeta, 'epoch'>;
export interface ConnTransport {
  invoke<T = any>(name: string, args?: Record<string, unknown>, meta?: OwnerRequestMeta): Promise<T>;
  listen<T>(name: string, handler: (event: { payload: T }) => void): Promise<() => void>;
  /** Retire this attachment and surface explicit reconnection; never replay mutations. */
  reset?(cause: unknown): void;
  dispose?(): void;
}
export interface ConnPorts {
  transport: ConnTransport;
  attention: { isFocused(): Promise<boolean>; request(active: boolean): Promise<void> };
  updates: {
    supported: boolean;
    reason?: string;
    invoke(action: 'status' | 'check' | 'download' | 'install', options?: { previews?: boolean; confirmed?: boolean }): Promise<UpdateStatus>;
  };
  links?: { open(url: string): Promise<void> };
  clipboard?: { writeText(text: string): Promise<void> };
  storage?: Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;
  platform?: 'macos' | 'linux' | 'windows' | 'web';
}
