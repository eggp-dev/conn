import assert from 'node:assert/strict';
import test from 'node:test';
import { terminalResizeQueue } from '../../src/lib/terminalResize.ts';

const turn = () => new Promise(resolve => setImmediate(resolve));
const deferred = () => {
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};

test('a slow old resize cannot overtake the latest terminal size', async () => {
  const first = deferred();
  const calls: {rows:number;cols:number}[] = [];
  let pty = { rows:24, cols:80 };
  const queue = terminalResizeQueue(async size => {
    calls.push(size);
    if (calls.length === 1) await first.promise;
    pty = size;
  }, error => { throw error; });
  queue.resize({ rows:30, cols:83 });
  queue.resize({ rows:29, cols:100 });
  queue.resize({ rows:28, cols:137 });
  await turn();
  assert.deepEqual(calls, [{ rows:30, cols:83 }]);
  first.resolve(); await turn();
  assert.deepEqual(calls, [{ rows:30, cols:83 }, { rows:28, cols:137 }]);
  assert.deepEqual(pty, { rows:28, cols:137 });
  queue.dispose();
});

test('returning to the acknowledged size does not signal a redundant resize', async () => {
  const first = deferred();let calls=0;
  const queue = terminalResizeQueue(async () => { calls++;await first.promise; }, error => { throw error; });
  queue.resize({ rows:24, cols:80 });
  queue.resize({ rows:24, cols:100 });
  queue.resize({ rows:24, cols:80 });
  first.resolve();await turn();
  assert.equal(calls,1);queue.dispose();
});

test('a rejected resize does not strand the newest size or count as applied', async () => {
  const first=deferred();const errors:unknown[]=[];let calls=0;
  const queue=terminalResizeQueue(async () => {if(++calls===1)await first.promise;}, error=>errors.push(error));
  const size={rows:30,cols:104};queue.resize(size);queue.resize(size);
  const error=new Error('synthetic transport failure');first.reject(error);await turn();
  assert.deepEqual(errors,[error]);assert.equal(calls,2);queue.dispose();
});

test('closing a terminal discards pending sizes and late errors', async () => {
  const first=deferred();let calls=0;const errors:unknown[]=[];
  const queue=terminalResizeQueue(async () => {calls++;await first.promise;}, error=>errors.push(error));
  queue.resize({rows:30,cols:100});queue.resize({rows:20,cols:90});queue.dispose();
  first.reject(new Error('closed'));await turn();queue.resize({rows:40,cols:120});
  assert.equal(calls,1);assert.deepEqual(errors,[]);
});

test('one terminal never blocks another terminal resize', async () => {
  const first=deferred();let secondApplied=false;
  const one=terminalResizeQueue(()=>first.promise,error=>{throw error;});
  const two=terminalResizeQueue(async()=>{secondApplied=true;},error=>{throw error;});
  one.resize({rows:24,cols:80});two.resize({rows:30,cols:100});await turn();
  assert.equal(secondApplied,true);one.dispose();two.dispose();first.resolve();
});
