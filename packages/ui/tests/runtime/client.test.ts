import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { createClient } from '../../src/runtime/client.ts';
import { createLifetime } from '../../src/runtime/lifetime.ts';

function fixture() {
  const handlers = new Map<string, (event: any) => void>();
  const calls: { name: string; args: any; meta: any; resolve: (result: any) => void; reject: (error: any) => void }[] = [];
  let stops = 0, disposed = 0;
  const resets: unknown[] = [];
  const transport = {
    invoke: (name: string, args: any, meta: any) => new Promise<any>((resolve, reject) => calls.push({ name, args, meta, resolve, reject })),
    listen: async (name: string, handler: any) => { handlers.set(name, handler); return () => { stops++; handlers.delete(name); }; },
    dispose: () => { disposed++; },
    reset: (cause: unknown) => { resets.push(cause); },
  };
  const lifetime = createLifetime(), client = createClient(transport, lifetime);
  const emit = (name: string, payload: any) => handlers.get(name)?.({ payload });
  emit('ss:connection', { connected: true, epoch: 1, runtimeId: 'test' });
  return { client, lifetime, calls, emit, handlers, resets, get stops() { return stops; }, get disposed() { return disposed; } };
}
const tick = () => new Promise(resolve => setImmediate(resolve));

test('per-session FIFO preserves resize/input/terminal replies without blocking another session or slow reads', async () => {
  const f = fixture();
  const profile = f.client.invoke('profiles_test');
  const a = f.client.invoke('resize', { session: 'a', cols: 80 });
  const b = f.client.invoke('input', { session: 'a', text: 'x' });
  const c = f.client.invoke('terminal_response', { session: 'a', text: 'reply' });
  const other = f.client.invoke('input', { session: 'b', text: 'y' });
  await tick();
  assert.deepEqual(f.calls.map(x => x.name), ['profiles_test', 'resize', 'input']);
  assert.deepEqual(f.calls.map(x => x.meta.sequence), [undefined, 1, 1]);
  f.calls[2].resolve('other'); assert.equal(await other, 'other');
  f.calls[1].resolve(null); await a; await tick();
  assert.equal(f.calls[3].name, 'input'); assert.equal(f.calls[3].meta.sequence, 2);
  f.calls[3].resolve(null); await b; await tick();
  assert.equal(f.calls[4].name, 'terminal_response'); assert.equal(f.calls[4].meta.sequence, 3);
  f.calls[4].resolve(null); await c;
  f.calls[0].resolve('late'); await profile; f.lifetime.dispose();
});

test('disconnect fences queued input and does not replay after a new epoch', async () => {
  const f = fixture();
  const first = f.client.invoke('input', { session: 'a', text: 'first' });
  const second = f.client.invoke('input', { session: 'a', text: 'second' });
  const failures = Promise.all([assert.rejects(first, /disconnected/), assert.rejects(second, /disconnected/)]);
  await tick();
  f.emit('ss:connection', { connected: false, epoch: 1 });
  f.emit('ss:connection', { connected: true, epoch: 2 });
  f.calls[0].resolve(null); await failures;
  assert.equal(f.calls.length, 1);
  const next = f.client.invoke('input', { session: 'a', text: 'new human action' }); await tick();
  assert.equal(f.calls[1].meta.sequence, 1); f.calls[1].resolve(null); await next;
  f.lifetime.dispose();
});

test('output gaps block input and request one checkpoint; stale and duplicate frames never render', async () => {
  const f = fixture(), frames: any[] = [], sync: any[] = [];
  await f.client.listen('ss:output', e => frames.push(e.payload));
  await f.client.listen('ss:terminal_sync', e => sync.push(e.payload));
  const frame = (streamSeq: number, reset = false, epoch = 1) => f.emit('ss:output', { session: 'a', epoch, streamSeq, reset, data: '' });
  frame(4, true); frame(4); frame(6); frame(7); frame(100, false, 0);
  assert.equal(frames.length, 1); assert.equal(f.calls.length, 1); assert.equal(f.calls[0].name, 'attach_output');
  await assert.rejects(f.client.invoke('input', { session: 'a', text: 'unsafe replay' }), /synchronizing/);
  frame(8, true); f.calls[0].resolve(null); frame(9);
  assert.deepEqual(frames.map(x => x.streamSeq), [4, 8, 9]);
  assert.deepEqual(sync.map(x => x.ready), [true, false, true]);
  f.lifetime.dispose();
});

test('connection state replays for late subscribers and disposal releases every event listener once', async () => {
  const f = fixture(), connections: any[] = [];
  await f.client.listen('ss:connection', e => connections.push(e.payload));
  const stop = await f.client.listen('ss:output', () => assert.fail('late event'));
  stop(); stop();
  assert.equal(connections.at(-1).epoch, 1);
  f.lifetime.dispose(); f.lifetime.dispose(); await tick();
  assert.equal(f.stops, 2); assert.equal(f.disposed, 1); assert.equal(f.handlers.size, 0);
  await assert.rejects(f.client.invoke('input', { session: 'a' }), /disposed/);
});

test('unconsumed or uncertain input failure fences queued keys until fresh attachment without replay', async () => {
  for (const code of ['owner_busy', 'outcome_unknown', 'terminal_sequence_gap']) {
    const f = fixture();
    const first = f.client.invoke('input', { session: 'a', text: 'unknown' });
    const queued = f.client.invoke('input', { session: 'a', text: 'must not follow' });
    const failed = Promise.all([assert.rejects(first, new RegExp(code)), assert.rejects(queued, /disconnected/)]);
    await tick(); f.calls[0].reject(Object.assign(new Error(code), { code })); await failed;
    assert.equal(f.resets.length, 1); assert.equal(f.calls.length, 1);
    await assert.rejects(f.client.invoke('input', { session: 'a', text: 'still blocked' }), /disconnected/);
    f.emit('ss:connection', { connected: true, epoch: 2 });
    const next = f.client.invoke('input', { session: 'a', text: 'new explicit action' }); await tick();
    assert.equal(f.calls[1].meta.sequence, 1);
    assert.equal(f.calls[1].args.text, 'new explicit action');
    f.calls[1].resolve(null); await next; f.lifetime.dispose();
  }
});

test('failed checkpoint exposes owner reattachment instead of permanent silent synchronization', async () => {
  const f = fixture(); const connections: any[] = [];
  await f.client.listen('ss:connection', e => connections.push(e.payload));
  await f.client.listen('ss:output', () => {});
  f.emit('ss:output', { session: 'a', epoch: 1, streamSeq: 1, reset: true });
  f.emit('ss:output', { session: 'a', epoch: 1, streamSeq: 3 });
  f.calls[0].reject(new Error('checkpoint temporarily unavailable')); await tick();
  assert.equal(f.resets.length, 1); assert.equal(connections.at(-1).connected, false);
  await assert.rejects(f.client.invoke('input', { session: 'a', text: 'blocked' }), /disconnected/);
  f.lifetime.dispose();
});
