import { OWNER_PROTOCOL, type ConnTransport, type OwnerRequestMeta, type OwnerHello } from '@conn/ui/runtime';

type Hello = OwnerHello;
type Problem = 'authentication' | 'occupied' | 'offline' | 'version' | 'reconnect';
type Handler = (event: { payload: any }) => void;
type Pending = { resolve(value: any): void; reject(error: Error): void; timer: ReturnType<typeof setTimeout> };
function failure(code: string, message = code) { return Object.assign(new Error(message), { code }); }

/** Transport only. Ordering, terminal recovery and app lifetime live in @conn/ui. */
export class WebTransport implements ConnTransport {
  private socket?: WebSocket;
  private hello?: Hello;
  private listeners = new Map<string, Set<Handler>>();
  private pending = new Map<number, Pending>();
  private nextId = 0;
  private stopped = false;
  private connected = false;
  private connecting?: Promise<void>;
  private retry?: ReturnType<typeof setTimeout>;
  private retryDelay = 500;
  constructor(private problem: (kind: Problem) => void) {}

  connect(takeover = false): Promise<void> {
    if (this.stopped) return Promise.reject(failure('disposed'));
    if (this.connected) return Promise.resolve();
    return this.connecting ??= this.open(takeover).finally(() => { this.connecting = undefined; });
  }
  private async open(takeover: boolean) {
    const response = await fetch('/api/info', { credentials: 'same-origin', cache: 'no-store' });
    if (!response.ok) throw failure('offline', `Conn host returned ${response.status}`);
    const info = await response.json();
    if (!info.authenticated) { this.problem('authentication'); throw failure('authentication'); }
    if (info.protocol !== OWNER_PROTOCOL) { this.problem('version'); throw failure('protocol_mismatch', 'This Conn host requires a different client version.'); }
    if (this.stopped) throw failure('disposed');
    await new Promise<void>((resolve, reject) => {
      const url = new URL('/api/ws', location.href);
      url.protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
      const socket = this.socket = new WebSocket(url);
      let ready = false, fenced = false;
      const timer = setTimeout(() => { reject(failure('timeout', 'Conn connection timed out.')); socket.close(); }, 10_000);
      socket.onopen = () => socket.send(JSON.stringify({ type: 'hello', protocol: OWNER_PROTOCOL, takeover }));
      socket.onmessage = event => {
        if (this.socket !== socket || this.stopped) return;
        let message: any;
        try { message = JSON.parse(event.data); } catch { socket.close(); return; }
        if (message.type === 'hello') {
          clearTimeout(timer);
          if (message.protocol !== OWNER_PROTOCOL) { this.problem('version'); reject(failure('protocol_mismatch')); socket.close(); return; }
          this.hello = message; ready = true; this.connected = true; this.retryDelay = 500;
          this.emit('ss:connection', { connected: true, ...message }); resolve(); return;
        }
        if (message.type === 'attachment_fenced' || (!ready && message.error === 'owner_attached')) {
          fenced = true; this.problem('occupied'); reject(failure('owner_attached')); socket.close(); return;
        }
        if (!ready && message.type === 'error') { reject(failure(message.error ?? 'connection_failed')); socket.close(); return; }
        if (message.type === 'resync_required') { socket.close(); return; }
        if (typeof message.id === 'number') {
          const pending = this.pending.get(message.id);
          if (!pending) return;
          this.pending.delete(message.id); clearTimeout(pending.timer);
          if (message.error) pending.reject(failure(message.code ?? 'command_failed', message.error));
          else pending.resolve(message.result);
          return;
        }
        if (typeof message.event === 'string' && message.epoch === this.hello?.epoch) this.emit(message.event, message.payload);
      };
      socket.onerror = () => { /* close owns cleanup and retry */ };
      socket.onclose = () => {
        clearTimeout(timer);
        if (this.socket !== socket) return;
        this.connected = false;
        this.failPending();
        this.emit('ss:connection', { connected: false, ...this.hello });
        if (!ready) reject(failure(fenced ? 'owner_attached' : 'offline'));
        if (ready && !fenced && !this.stopped) this.scheduleReconnect();
      };
    });
  }
  private scheduleReconnect() {
    if (this.stopped || this.retry) return;
    this.problem('offline');
    this.retry = setTimeout(() => {
      this.retry = undefined;
      void this.connect().catch(error => {
        if (!['authentication', 'owner_attached', 'protocol_mismatch', 'disposed'].includes(error.code)) {
          this.retryDelay = Math.min(this.retryDelay * 2, 5000); this.scheduleReconnect();
        }
      });
    }, this.retryDelay);
  }
  private request(message: Record<string, unknown>): Promise<any> {
    if (!this.connected || this.socket?.readyState !== WebSocket.OPEN) return Promise.reject(failure('offline'));
    const id = ++this.nextId;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(failure('outcome_unknown', 'Command response was lost; its outcome is unknown.')); }, 30_000);
      this.pending.set(id, { resolve, reject, timer });
      try { this.socket!.send(JSON.stringify({ id, ...message })); }
      catch { clearTimeout(timer); this.pending.delete(id); reject(failure('outcome_unknown')); }
    });
  }
  async invoke<T>(name: string, args: Record<string, unknown> = {}, meta?: OwnerRequestMeta): Promise<T> {
    if (!meta) throw failure('missing_operation', 'Use the shared owner client for Conn commands.');
    const epoch = this.hello?.epoch;
    try { return await this.request({ name, args, epoch, ...meta }); }
    catch (error: any) {
      // Query the original result once; never resubmit a mutation after a lost reply.
      if (error.code !== 'outcome_unknown' || !this.connected || epoch !== this.hello?.epoch) throw error;
      const outcome = await this.request({ type: 'outcome', epoch, operationId: meta.operationId });
      if (outcome.status === 'completed') {
        if (outcome.error) throw failure('command_failed', outcome.error);
        return outcome.result;
      }
      throw error;
    }
  }
  async listen<T>(name: string, handler: (event: { payload: T }) => void) {
    let listeners = this.listeners.get(name);
    if (!listeners) this.listeners.set(name, listeners = new Set());
    listeners.add(handler);
    if (name === 'ss:connection') handler({ payload: { connected: this.connected, ...this.hello } as T });
    return () => { listeners.delete(handler); if (!listeners.size) this.listeners.delete(name); };
  }
  private emit(name: string, payload: any) { for (const handler of this.listeners.get(name) ?? []) handler({ payload }); }
  private failPending() {
    for (const pending of this.pending.values()) { clearTimeout(pending.timer); pending.reject(failure('outcome_unknown', 'Connection lost; the command may have completed.')); }
    this.pending.clear();
  }
  reset(_cause: unknown) {
    if (this.stopped) return;
    this.stopped = true; this.connected = false; clearTimeout(this.retry); this.retry = undefined;
    this.failPending();
    this.emit('ss:connection', { connected: false, ...this.hello });
    this.socket?.close();
    this.problem('reconnect');
  }
  dispose() {
    this.stopped = true; clearTimeout(this.retry); this.failPending(); this.listeners.clear();
    this.socket?.close(); this.connected = false;
  }
}
