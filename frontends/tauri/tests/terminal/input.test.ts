import assert from 'node:assert/strict';
import test from 'node:test';
import { createRequire } from 'node:module';
import { observeTerminalInput, type InputOrigin } from '../../src/lib/terminalInput.ts';

// xterm's parser works without opening a DOM renderer. Exercise the installed
// implementation so dependency updates cannot silently change input provenance.
if (!('self' in globalThis)) Object.defineProperty(globalThis, 'self', {value:globalThis, configurable:true});
const require = createRequire(import.meta.url);
const { Terminal } = require('@xterm/xterm') as typeof import('@xterm/xterm');
type Packet = { data: string; origin: InputOrigin };
const parsed = (term: InstanceType<typeof Terminal>, text: string) => new Promise<void>(resolve => term.write(text, resolve));

test('real xterm DSR and DA replies are terminal responses, not human takeover', async () => {
  const term = new Terminal({allowProposedApi:true});
  const packets: Packet[] = [];
  const observer = observeTerminalInput(term, (data, origin) => packets.push({data, origin}));
  try {
    await parsed(term, '\x1b[5n\x1b[6n\x1b[c\x1b[>c\x1b[?6n');
    assert.deepEqual(packets.map(p => p.data), ['\x1b[0n','\x1b[1;1R','\x1b[?1;2c','\x1b[>0;276;0c','\x1b[?1;1R']);
    assert.ok(packets.every(p => p.origin === 'terminal'));
    term.input('local input', true);
    assert.deepEqual(packets.at(-1), {data:'local input', origin:'human'});
    await parsed(term, '\x1b[5n');
    assert.deepEqual(packets.at(-1), {data:'\x1b[0n', origin:'terminal'});
  } finally { observer.dispose(); term.dispose(); }
});

test('real xterm paste and Unicode user input retain human provenance', async () => {
  const term = new Terminal({allowProposedApi:true});
  // The public paste method only needs the textarea value when no DOM is opened.
  const core = (term as unknown as {_core:{textarea:{value:string}}})._core;
  core.textarea = {value:''};
  const packets: Packet[] = [];
  const observer = observeTerminalInput(term, (data, origin) => packets.push({data, origin}));
  try {
    await parsed(term, '\x1b[?2004h');
    term.paste('pasted\ntext');
    term.input('한글', true);
    term.input('programmatic response', false);
    assert.deepEqual(packets, [
      {data:'\x1b[200~pasted\rtext\x1b[201~', origin:'human'},
      {data:'한글', origin:'human'},
      {data:'programmatic response', origin:'terminal'},
    ]);
  } finally { observer.dispose(); term.dispose(); }
});

test('disposal detaches provenance and data listeners before a replacement observer', async () => {
  const term = new Terminal({allowProposedApi:true});
  const old: Packet[] = [], fresh: Packet[] = [];
  const first = observeTerminalInput(term, (data, origin) => old.push({data, origin}));
  term.input('first', true);
  first.dispose(); first.dispose();
  const second = observeTerminalInput(term, (data, origin) => fresh.push({data, origin}));
  try {
    await parsed(term, '\x1b[5n');
    term.input('second', true);
    assert.deepEqual(old, [{data:'first', origin:'human'}]);
    assert.deepEqual(fresh, [{data:'\x1b[0n', origin:'terminal'}, {data:'second', origin:'human'}]);
  } finally { second.dispose(); term.dispose(); }
});

test('unavailable xterm provenance falls back to human input rather than preserving a writer', async () => {
  const term = new Terminal({allowProposedApi:true});
  const packets: Packet[] = [];
  // Expose only the supported public API to simulate changed internals.
  const publicOnly = {onData: term.onData.bind(term)};
  const observer = observeTerminalInput(publicOnly, (data, origin) => packets.push({data, origin}));
  try {
    await parsed(term, '\x1b[5n');
    term.input('local input', true);
    assert.equal(packets.length, 2);
    assert.ok(packets.every(p => p.origin === 'human'));
  } finally { observer.dispose(); term.dispose(); }
});
