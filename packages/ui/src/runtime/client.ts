import type { ConnTransport } from './ports';
import type { Lifetime } from './lifetime';

type Event = { payload: any };
type Handler = (event: Event) => void;
type Subscription = { handlers: Set<Handler>; ready: Promise<void>; stop?: () => void };
type Stream = { epoch: number; sequence: number };
const ordered = new Set(['input', 'terminal_response', 'resize']);

/** One owner client per app. Both native and web use these ordering and recovery rules. */
export function createClient(transport: ConnTransport, lifetime: Lifetime) {
  const queues = new Map<string, Promise<unknown>>();
  const sequences = new Map<string, number>();
  const versions = new Map<string, number>();
  const streams = new Map<string, Stream>();
  const recovering = new Set<string>();
  const subscriptions = new Map<string, Subscription>();
  let operationId = 0;
  let connectionVersion = 0;
  let epoch = -1;
  let lastConnection: Event | undefined;
  let needsAttachment = false;
  let resetRequested = false;

  const closed = () => new Error('Owner view disconnected or disposed; refresh its current state before retrying.');
  function emit(name: string, event: Event) {
    if (lifetime.disposed) return;
    for (const handler of subscriptions.get(name)?.handlers ?? []) handler(event);
  }
  function invalidate(session: string) {
    versions.set(session, (versions.get(session) ?? 0) + 1);
  }
  function resetAttachment(error: unknown) {
    if (lifetime.disposed || resetRequested) return;
    resetRequested = true;
    // Never guess whether a rejected operation consumed its sequence. A fresh
    // owner epoch fences every old queued action without resending any input.
    if (!needsAttachment) receive('ss:connection', { payload: { ...lastConnection?.payload, connected: false, error: String(error) } });
    transport.reset?.(error);
  }
  function receive(name: string, event: Event) {
    if (lifetime.disposed) return;
    if (name === 'ss:connection') {
      lastConnection = event;
      if (!event.payload.connected) {
        needsAttachment = true;
        connectionVersion++;
        sequences.clear(); streams.clear(); recovering.clear();
      } else { needsAttachment = false; resetRequested = false; }
      if (Number.isSafeInteger(event.payload.epoch)) epoch = event.payload.epoch;
    }
    if (name === 'ss:output') {
      const frame = event.payload;
      if (!Number.isSafeInteger(frame.epoch) || !Number.isSafeInteger(frame.streamSeq)) return;
      if (frame.epoch < epoch) return;
      if (frame.epoch > epoch) { epoch = frame.epoch; streams.clear(); }
      const previous = streams.get(frame.session);
      if (previous && previous.epoch === frame.epoch && frame.streamSeq <= previous.sequence) return;
      if (!frame.reset && (!previous || frame.streamSeq !== previous.sequence + 1)) {
        if (!recovering.has(frame.session)) {
          recovering.add(frame.session); invalidate(frame.session);
          emit('ss:terminal_sync', { payload: { session: frame.session, ready: false } });
          // Never replay input. Recover only the owner's terminal state at a new checkpoint.
          void invoke('attach_output', { session: frame.session }).catch(error => {
            emit('ss:terminal_sync', { payload: { session: frame.session, ready: false, error: String(error) } });
            resetAttachment(error);
          });
        }
        return;
      }
      streams.set(frame.session, { epoch: frame.epoch, sequence: frame.streamSeq });
      if (frame.reset) recovering.delete(frame.session);
      emit(name, event);
      if (frame.reset) emit('ss:terminal_sync', { payload: { session: frame.session, ready: true } });
      return;
    }
    emit(name, event);
  }
  function subscribe(name: string): Subscription {
    const existing = subscriptions.get(name);
    if (existing) return existing;
    const entry: Subscription = { handlers: new Set(), ready: Promise.resolve() };
    subscriptions.set(name, entry);
    // terminal_sync is a client lifecycle event, not a second backend endpoint.
    if (name !== 'ss:terminal_sync') entry.ready = transport.listen(name, event => receive(name, event)).then(stop => {
      if (lifetime.disposed) stop(); else entry.stop = stop;
    }).catch(error => { subscriptions.delete(name); throw error; });
    return entry;
  }
  // Disconnect fences queued work even when a view has no terminals.
  const connection = subscribe('ss:connection');
  void connection.ready.catch(() => {});

  function invoke<T = any>(name: string, args: Record<string, unknown> = {}): Promise<T> {
    if (lifetime.disposed) return Promise.reject(closed());
    const session = typeof args.session === 'string' ? args.session : '';
    const version = connectionVersion;
    const sessionVersion = versions.get(session) ?? 0;
    const run = async (): Promise<T> => {
      if (lifetime.disposed || needsAttachment || version !== connectionVersion || sessionVersion !== (versions.get(session) ?? 0)) throw closed();
      if (ordered.has(name) && recovering.has(session)) throw new Error('Terminal is synchronizing. Wait before entering more input.');
      const sequence = ordered.has(name) ? (sequences.get(session) ?? 0) + 1 : undefined;
      if (sequence !== undefined) sequences.set(session, sequence);
      let result: T;
      try { result = await transport.invoke<T>(name, args, { operationId: ++operationId, sequence }); }
      catch (error) {
        const code = String((error as { code?: unknown })?.code ?? error);
        if (name === 'attach_output' || (ordered.has(name) && /owner_busy|terminal_sequence_(gap|stale)|outcome_(unknown|pending)|attachment_fenced|offline/.test(code))) resetAttachment(error);
        throw error;
      }
      if (lifetime.disposed || version !== connectionVersion) throw closed();
      if (name === 'close_tab') {
        invalidate(session); sequences.delete(session); streams.delete(session); recovering.delete(session);
      }
      return result;
    };
    if (!ordered.has(name)) return run();
    if (!session) return Promise.reject(new Error('Ordered owner commands require a session.'));
    const previous = queues.get(session) ?? Promise.resolve();
    const result = previous.then(run, run);
    queues.set(session, result);
    void result.finally(() => { if (queues.get(session) === result) queues.delete(session); }).catch(() => {});
    return result;
  }
  async function listen<T>(name: string, handler: (event: { payload: T }) => void) {
    if (lifetime.disposed) return () => {};
    const entry = subscribe(name);
    entry.handlers.add(handler);
    try { await entry.ready; } catch (error) { entry.handlers.delete(handler); throw error; }
    if (name === 'ss:connection' && lastConnection && !lifetime.disposed) handler(lastConnection);
    let done = false;
    return lifetime.own(() => { if (!done) { done = true; entry.handlers.delete(handler); } });
  }
  lifetime.own(() => {
    connectionVersion++;
    for (const entry of subscriptions.values()) { entry.handlers.clear(); entry.stop?.(); }
    subscriptions.clear(); queues.clear(); sequences.clear(); streams.clear(); recovering.clear();
    transport.dispose?.();
  });
  return { invoke, listen };
}
export type ConnClient = ReturnType<typeof createClient>;
