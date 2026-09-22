import assert from 'node:assert/strict';
import test from 'node:test';
import { terminalResizeQueue } from '../../../../packages/ui/src/lib/terminalResize.ts';
const turn = () => new Promise(resolve => setImmediate(resolve));

test('size requests reach the shared queue before intervening input', () => {
  const operations: unknown[] = [];
  const sizes = terminalResizeQueue(async size => { operations.push(size); }, error => { throw error; });
  sizes.resize({ rows:30, cols:83 });
  sizes.resize({ rows:29, cols:100 });
  operations.push('input');
  sizes.resize({ rows:28, cols:137 });
  assert.deepEqual(operations, [{ rows:30, cols:83 }, { rows:29, cols:100 }, 'input', { rows:28, cols:137 }]);
  sizes.dispose();
});

test('only consecutive equal measurements are discarded', () => {
  const calls: unknown[] = [];
  const queue = terminalResizeQueue(async size => { calls.push(size); }, error => { throw error; });
  queue.resize({ rows:24, cols:80 }); queue.resize({ rows:24, cols:80 });
  queue.resize({ rows:24, cols:100 }); queue.resize({ rows:24, cols:80 });
  assert.equal(calls.length,3); queue.dispose();
});

test('a failed size can be retried and an attachment can invalidate deduplication', async () => {
  let calls=0; const errors: unknown[]=[];
  const queue=terminalResizeQueue(async()=>{if(++calls===1)throw new Error('failed');},error=>errors.push(error));
  const size={rows:30,cols:104}; queue.resize(size); await turn(); queue.resize(size); await turn();
  assert.equal(calls,2); assert.equal(errors.length,1);
  queue.invalidate(); queue.resize(size); assert.equal(calls,3); queue.dispose();
});

test('disposal rejects new measurements and silences late errors', async () => {
  let reject!: (error: Error) => void; let calls=0; const errors: unknown[]=[];
  const queue=terminalResizeQueue(()=>{calls++;return new Promise((_,no)=>reject=no);},error=>errors.push(error));
  queue.resize({rows:30,cols:100}); queue.dispose(); reject(new Error('closed')); await turn();
  queue.resize({rows:40,cols:120}); assert.equal(calls,1); assert.deepEqual(errors,[]);
});
