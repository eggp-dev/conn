import assert from 'node:assert/strict';
import test from 'node:test';
import { terminalOutputQueue } from '../../src/lib/terminalOutput.ts';

test('resize markers wait for old bytes and precede the new shell redraw', () => {
  const events: string[] = [];
  const completions: (() => void)[] = [];
  const queue = terminalOutputQueue(
    size => events.push(`size ${size.rows}x${size.cols}`),
    (data, done) => { events.push(new TextDecoder().decode(data)); completions.push(done); },
  );
  const frame = (rows: number, text = '') => ({size:{rows,cols:104}, data:new TextEncoder().encode(text)});
  queue.push(frame(38, 'old output'));
  queue.push(frame(27));
  queue.push(frame(27, 'small redraw'));
  queue.push(frame(38));
  queue.push(frame(38, 'restored redraw'));
  assert.deepEqual(events, ['size 38x104', 'old output']);
  completions.shift()!();
  assert.deepEqual(events.slice(2), ['size 27x104', 'size 27x104', 'small redraw']);
  completions.shift()!();
  assert.deepEqual(events.slice(5), ['size 38x104', 'size 38x104', 'restored redraw']);
  completions.shift()!();queue.dispose();
});

test('disposing a renderer discards frames waiting for an asynchronous write', () => {
  const applied: number[] = [];let complete!: () => void;
  const queue=terminalOutputQueue(size=>applied.push(size.rows),(_data,done)=>{complete=done;});
  queue.push({size:{rows:38,cols:104},data:new Uint8Array([65])});
  queue.push({size:{rows:27,cols:104},data:new Uint8Array()});
  queue.dispose();complete();
  queue.push({size:{rows:20,cols:80},data:new Uint8Array()});
  assert.deepEqual(applied,[38]);
});
