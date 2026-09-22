// Restore real conn-core owner checkpoints into the production xterm renderer.
// Fixtures cover every byte split, including incomplete UTF-8 and VT sequences.
import {strict as assert} from 'node:assert';
import {spawnSync} from 'node:child_process';
import {mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright';

const require = createRequire(import.meta.url);
const root = fileURLToPath(new URL('../../../', import.meta.url));
const temporary = mkdtempSync(join(tmpdir(), 'conn-checkpoint-parity-'));
const fixturesPath = join(temporary, 'fixtures.json');
let browser;
let failed = false;
try {
  const generated = spawnSync('cargo', [
    'test', '--locked', '-p', 'conn-core', '--test', 'renderer_checkpoint',
    'checkpoint_preserves_output_state_and_following_bytes_at_every_split', '--', '--exact',
  ], {
    cwd: root,
    env: {...process.env, CONN_CHECKPOINT_FIXTURES: fixturesPath},
    stdio: 'inherit',
  });
  if (generated.error) throw generated.error;
  assert.equal(generated.status, 0, 'conn-core checkpoint fixture generation failed');
  const fixtures = JSON.parse(readFileSync(fixturesPath, 'utf8'));
  assert.ok(fixtures.length > 0, 'checkpoint fixtures must not be empty');
  browser = await chromium.launch({headless: true});
  const page = await browser.newPage();
  await page.setContent('<div id="original"></div><div id="restored"></div>');
  await page.addScriptTag({path: require.resolve('@xterm/xterm')});
  await page.addScriptTag({path: require.resolve('@xterm/addon-unicode11')});
  const result = await page.evaluate(async fixtures => {
    const failures = [];
    let checked = 0;
    function snapshot(term) {
      const buffer = term.buffer.active;
      return {
        type: buffer.type, x: buffer.cursorX, y: buffer.cursorY,
        baseY: buffer.baseY, length: buffer.length, modes: term.modes,
        lines: Array.from({length: buffer.length}, (_, y) => {
          const line = buffer.getLine(y);
          return {wrap: line.isWrapped, cells: Array.from({length: term.cols}, (_, x) => {
            const cell = line.getCell(x);
            return [
              cell.getChars(), cell.getWidth(), cell.getFgColorMode(), cell.getFgColor(),
              cell.getBgColorMode(), cell.getBgColor(), cell.isBold(), cell.isItalic(),
              cell.isUnderline(), cell.isInverse(), cell.isInvisible(), cell.isDim(),
              cell.isBlink(), cell.isStrikethrough(),
            ];
          })};
        }),
      };
    }
    const write = (term, bytes) => new Promise(resolve => term.write(Uint8Array.from(bytes), resolve));
    const tail = Array.from(new TextEncoder().encode('\x1b8Z\nlast'));
    for (const fixture of fixtures) {
      const options = {cols: fixture.cols, rows: fixture.rows, scrollback: 5000, allowProposedApi: true};
      const original = new Terminal(options);
      const restored = new Terminal(options);
      try {
        for (const term of [original, restored]) {
          term.loadAddon(new Unicode11Addon.Unicode11Addon());
          term.unicode.activeVersion = '11';
        }
        original.open(document.getElementById('original'));
        restored.open(document.getElementById('restored'));
        await write(original, fixture.before);
        await write(restored, fixture.checkpoint);
        for (const [stage, bytes] of [
          ['initial', null], ['continuation', fixture.after], ['saved-cursor', tail], ['resized', []],
        ]) {
          if (stage === 'resized') { original.resize(26, 8); restored.resize(26, 8); }
          if (bytes) { await write(original, bytes); await write(restored, bytes); }
          const expected = snapshot(original);
          const actual = snapshot(restored);
          if (JSON.stringify(expected) !== JSON.stringify(actual)) {
            failures.push({fixture: fixture.fixture, split: fixture.split, stage, expected, actual});
            break;
          }
        }
      } finally {
        original.dispose();
        restored.dispose();
      }
      checked++;
      if (failures.length >= 10) break;
    }
    return {checked, total: fixtures.length, failures};
  }, fixtures);
  if (result.failures.length) {
    const output = join(temporary, 'results.json');
    writeFileSync(output, JSON.stringify(result, null, 2));
    throw new Error(`Checkpoint parity failed: ${JSON.stringify(result.failures.map(({fixture, split, stage}) => ({fixture, split, stage})))}. Details: ${output}`);
  }
  console.log(`PASS ${result.checked}/${result.total} owner checkpoint/xterm cases (grid, history, cursor, styles, modes, continuation, resize)`);
} catch (error) {
  failed = true;
  throw error;
} finally {
  await browser?.close();
  // Keep failed fixtures/results for diagnosis; successful runs leave no artifacts.
  if (!failed) rmSync(temporary, {recursive: true, force: true});
}
