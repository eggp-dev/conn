// Production web bundle, real Rust PTYs and one persistent matching Conn MCP.
// Owner actions use the DOM. Transport fault injection only delays an existing
// resize frame unchanged; no replacement UI, owner commands or application state.
import {chromium} from 'playwright';
import {spawn} from 'node:child_process';
import {mkdtempSync, writeFileSync, readFileSync, existsSync, mkdirSync, openSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createInterface} from 'node:readline';
import {strict as assert} from 'node:assert';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const state = mkdtempSync(join(tmpdir(), 'conn-input-rendering-'));
const evidence = process.env.CONN_INPUT_REVIEW_OUTPUT ?? join(state, 'evidence');
mkdirSync(evidence, {recursive: true});
const binary = name => resolve(root, 'target/debug', name + (process.platform === 'win32' ? '.exe' : ''));
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(run, message, milliseconds = 12_000) {
  const started = Date.now();
  do { const result = await run(); if (result) return result; await delay(80); } while (Date.now() - started < milliseconds);
  throw new Error(message);
}
assert.ok(existsSync(resolve(root, 'frontends/web/dist/index.html')), 'Build @conn/web before running the rendering test');
writeFileSync(join(state, 'app.json'), JSON.stringify({mode: 'autopilot', gate: true, pacing: {enterGraceMs: 0, minWriteIntervalMs: 0, leaseTtlSecs: 120, approvalTtlSecs: 120}}));
writeFileSync(join(state, 'profiles.json'), JSON.stringify({version: 1, revision: 0, defaultProfile: 'fixture', profiles: [{
  id: 'fixture', name: 'Input rendering fixture', backend: 'local', shell: 'posix',
  program: process.env.CONN_REPRO_SHELL ?? '/bin/bash', args: process.env.CONN_REPRO_SHELL ? ['-i'] : ['--noprofile', '--norc', '-i'],
  env: {PS1: 'fixture$ ', TERM: 'xterm-256color', HISTFILE: '/dev/null', HISTCONTROL: ''}, cwd: state,
}]}));
const log = openSync(join(state, 'server.log'), 'w');
const server = spawn(binary('conn-web'), ['serve', '--state-dir', state, '--port', '0', '--ui-dir', resolve(root, 'frontends/web/dist'), '--setup-home', join(state, 'setup-home')], {stdio: ['ignore', log, log]});
let browser, context, page, agent;
const errors = [], cases = [], resizes = [], outputFrames = [], timers = new Set();
let resizeDelay = null;

class MCP {
  constructor() {
    this.pending = new Map(); this.id = 0;
    this.child = spawn(binary('conn'), ['--socket', join(state, 'conn.sock'), 'mcp', '--agent-id', 'input-rendering', '--tools', 'static'], {stdio: ['pipe', 'pipe', log]});
    createInterface({input: this.child.stdout}).on('line', line => {
      const message = JSON.parse(line), pending = this.pending.get(message.id);
      if (!pending) return;
      this.pending.delete(message.id); clearTimeout(pending.timer);
      if (message.error) pending.reject(new Error(JSON.stringify(message.error))); else pending.resolve(message.result);
    });
    this.child.on('exit', () => {
      for (const pending of this.pending.values()) { clearTimeout(pending.timer); pending.reject(new Error('MCP exited')); }
      this.pending.clear();
    });
  }
  call(method, params = {}) {
    const id = ++this.id;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`MCP timeout: ${method}`)); }, 18_000);
      this.pending.set(id, {resolve, reject, timer});
      this.child.stdin.write(JSON.stringify({jsonrpc: '2.0', id, method, params}) + '\n');
    });
  }
  async start() {
    const initialized = await this.call('initialize', {protocolVersion: '2025-06-18', clientInfo: {name: 'Conn input rendering test', version: '1'}, capabilities: {}});
    const version = JSON.parse(readFileSync(resolve(root, 'packages/ui/package.json'), 'utf8')).version;
    assert.equal(initialized.serverInfo.version, version, 'MCP binary and UI release versions match');
    await this.call('tools/list');
  }
  async tool(name, args = {}) {
    const result = await this.call('tools/call', {name: `terminal_${name}`, arguments: args});
    const text = result.content?.find(item => item.type === 'text')?.text;
    if (result.isError) throw new Error(`${name}: ${text}`);
    return JSON.parse(text);
  }
  close() { this.child.stdin.end(); this.child.kill(); }
}

const host = () => page.locator('.host:not([hidden])');
const input = () => host().locator('.xterm-helper-textarea');
const ready = () => page.locator('.host:not([hidden])[data-terminal-ready="true"]').waitFor();
async function human(text) { await ready(); await input().focus(); await page.keyboard.insertText(text); await page.keyboard.press('Enter'); await delay(250); }
async function share() {
  const trigger = page.locator('.sharing-trigger');
  if (await trigger.isVisible()) await trigger.click();
  else { await page.locator('button.dot:not(.hidden)').click(); await page.locator('.sharing-row button').click(); }
  await page.locator('.sharing input[type="checkbox"]').first().check();
  await page.locator('.sharing footer .btn:not(.ghost)').click();
  await until(async () => (await agent.tool('list_tabs')).tabs === 1, 'Shared shell not visible to MCP');
}
async function control() {
  const requested = agent.tool('request_control', {reason: 'Verify long input, resizing and approval motion in this fixture shell.'});
  await page.locator('[data-request-key*=":control:"] .primary').click();
  const result = await requested;
  assert.equal(result.type, 'agent', 'Control request resolves to the live agent controller');
}
async function execute(text, intent = 'Print synthetic fixture output.') {
  await agent.tool('type', {text});
  const result = await agent.tool('send_key', {key: 'ENTER', intent});
  assert.equal(result.status, 'executed', JSON.stringify(result));
  return result;
}
async function capture(name, extra = {}) {
  cases.push({name, ...extra});
  writeFileSync(join(evidence, 'cases.json'), JSON.stringify(cases, null, 2));
  await page.screenshot({path: join(evidence, `${name}.png`)});
  console.log(`PASS ${name}`);
}
async function grid(label) {
  await delay(700);
  const snapshot = await agent.tool('snapshot');
  const visible = (await host().locator('.xterm-rows > div').allTextContents()).map(row => row.replace(/\u00a0/g, ' ').trimEnd());
  const cursor = await host().evaluate((element, cols) => {
    const cell = element.querySelector('.xterm-cursor'), screen = element.querySelector('.xterm-screen');
    const row = cell?.closest('.xterm-rows > div');
    if (!cell || !screen || !row) return null;
    return {row: Array.from(row.parentElement.children).indexOf(row), col: Math.round((cell.getBoundingClientRect().left - screen.getBoundingClientRect().left) / (screen.clientWidth / cols))};
  }, snapshot.size.cols);
  const result = {label, size: snapshot.size, core: snapshot.screen, visible, cursor: snapshot.cursor, visibleCursor: cursor};
  writeFileSync(join(evidence, `grid-${label}.json`), JSON.stringify(result, null, 2));
  assert.deepEqual(visible, snapshot.screen, `${label}: human and MCP grids agree`);
  assert.deepEqual(cursor, snapshot.cursor, `${label}: human and MCP cursor positions agree`);
  return result;
}
function delayNextResize(milliseconds) { resizeDelay = milliseconds; return resizes.length; }
function sizesSince(index) { return resizes.slice(index).map(({rows, cols}) => ({rows, cols})); }
async function startMotionSampling() {
  await page.evaluate(() => {
    window.__connRenderingSamples = [];
    const sample = () => {
      const host = document.querySelector('.host:not([hidden])');
      window.__connRenderingSamples.push({width: innerWidth, height: innerHeight, hostHeight: host.clientHeight, screenHeight: host.querySelector('.xterm-screen').clientHeight, transform: getComputedStyle(host).transform});
      window.__connRenderingFrame = requestAnimationFrame(sample);
    };
    sample();
  });
}
async function stopMotionSampling() {
  return page.evaluate(() => {
    cancelAnimationFrame(window.__connRenderingFrame);
    const frames = window.__connRenderingSamples;
    delete window.__connRenderingFrame; delete window.__connRenderingSamples;
    return frames;
  });
}

try {
  await until(() => { if (server.exitCode !== null) throw new Error('Web host failed to start'); return existsSync(join(state, 'connection.json')); }, 'No web connection file');
  const metadata = JSON.parse(readFileSync(join(state, 'connection.json'), 'utf8'));
  browser = await chromium.launch({headless: true});
  context = await browser.newContext({viewport: {width: 860, height: 640}, locale: 'en-US'});
  page = await context.newPage(); page.setDefaultTimeout(12_000);
  page.on('pageerror', error => errors.push(error.message));
  await page.routeWebSocket('**/api/ws', socket => {
    const upstream = socket.connectToServer();
    socket.onMessage(message => {
      const request = JSON.parse(message.toString());
      if (request.name === 'resize') {
        const record = {rows: request.args.rows, cols: request.args.cols, sequence: request.sequence, requestedAt: Date.now(), forwardedAt: null};
        resizes.push(record);
        const forward = () => { record.forwardedAt = Date.now(); upstream.send(message); };
        if (resizeDelay !== null) {
          const milliseconds = resizeDelay; resizeDelay = null;
          const timer = setTimeout(() => { timers.delete(timer); forward(); }, milliseconds); timers.add(timer); return;
        }
        forward(); return;
      }
      // Never retain raw input, including the optional fixture password.
      upstream.send(message);
    });
    upstream.onMessage(message => {
      const response = JSON.parse(message.toString());
      if (response.event === 'ss:output') {
        const {size, outputSeq, generation, streamSeq, epoch, reset} = response.payload;
        outputFrames.push({size, outputSeq, generation, streamSeq, epoch, reset});
      }
      socket.send(message);
    });
  });
  await page.goto(metadata.url + '/#token=' + encodeURIComponent(metadata.bootstrapToken));
  await ready(); await delay(600);
  if (process.env.CONN_AUTH_FIXTURE_RUNTIME) {
    const runtime = process.env.CONN_AUTH_FIXTURE_RUNTIME;
    const password = readFileSync(join(runtime, 'password'), 'utf8').trim();
    assert.ok(password.startsWith('SYNTHETIC_'), 'Only a disposable synthetic SSH fixture is supported');
    assert.ok(/^[A-Za-z0-9_./-]+$/.test(runtime), 'SSH fixture runtime path must contain no shell metacharacters');
    await human(`ssh -F /dev/null -tt -p 22222 -o StrictHostKeyChecking=yes -o UserKnownHostsFile=${runtime}/known_hosts -o PreferredAuthentications=password -o PubkeyAuthentication=no -o IdentityAgent=none fixture@127.0.0.1`);
    await until(async () => (await host().locator('.xterm-rows').innerText()).includes('password:'), 'SSH fixture password prompt missing');
    await human(password);
    await human("exec env LC_ALL=C.UTF-8 PS1='remote$ ' bash --noprofile --norc -i");
    await human('printf "REMOTE_READY_%s\\n" "$((6*7))"');
    await until(async () => (await host().locator('.xterm-rows').innerText()).includes('REMOTE_READY_42'), 'SSH fixture ready marker missing');
  }
  agent = new MCP(); await agent.start();
  await page.locator('[data-request-key^="connection:"] .primary').click();
  if ((await agent.tool('list_tabs')).tabs === 0) await share();
  await control(); await delay(500);

  const resizeStart = delayNextResize(350);
  await page.setViewportSize({width: 700, height: 640}); await delay(80);
  await page.setViewportSize({width: 860, height: 640}); await delay(750);
  const requested = sizesSince(resizeStart);
  assert.ok(requested.length >= 2 && requested[0].cols < requested.at(-1).cols, 'Exercise narrow then wide resize');
  assert.ok(resizes[resizeStart].forwardedAt - resizes[resizeStart].requestedAt >= 300, 'The real resize frame was delayed');
  assert.deepEqual((await grid('delayed-resize')).size, requested.at(-1), 'Late resize cannot restore an obsolete PTY size');
  await capture('delayed-resize', {requested});

  const metrics = await context.newCDPSession(page);
  for (const deviceScaleFactor of [1.25, 1.5, 2, 1]) {
    await metrics.send('Emulation.setDeviceMetricsOverride', {width: 860, height: 640, deviceScaleFactor, mobile: false});
    await delay(350);
    const geometry = await host().evaluate(element => ({hostWidth: element.clientWidth, screenWidth: element.querySelector('.xterm-screen').clientWidth, dpr: devicePixelRatio}));
    assert.equal(geometry.dpr, deviceScaleFactor, 'Browser applied the requested display density');
    assert.ok(geometry.hostWidth - geometry.screenWidth >= 0 && geometry.hostWidth - geometry.screenWidth <= 32, `Terminal fills pane after density change: ${JSON.stringify(geometry)}`);
    await grid(`density-${deviceScaleFactor}`);
    await capture(`density-${deviceScaleFactor}`, geometry);
  }
  await metrics.detach();

  await execute("printf 'synthetic row\\n%.0s' {1..70}", 'Fill the fixture screen to exercise approval dock motion.');
  const beforeDock = await grid('before-dock-motion');
  for (const latency of [0, 500]) {
    const start = delayNextResize(latency);
    await startMotionSampling();
    await agent.tool('type', {text: `rm -- '${join(state, 'missing-dock-' + 'x'.repeat(120))}'`});
    const review = await agent.tool('send_key', {key: 'ENTER', intent: 'Exercise approval motion without executing the fixture command.'});
    assert.equal(review.status, 'pending');
    const deny = page.locator('[data-request-key*=":review:"] .btn.danger');
    await deny.waitFor(); await delay(latency ? 10 : 90); await deny.click({force: true});
    await until(async () => (await agent.tool('check_approval', {approvalId: review.approvalId})).state === 'denied', 'Dock review was not denied');
    const restored = await grid(`dock-motion-${latency}`);
    const motion = {frames: await stopMotionSampling(), sizes: sizesSince(start)};
    writeFileSync(join(evidence, `dock-motion-${latency}.json`), JSON.stringify(motion, null, 2));
    assert.ok(motion.frames.every(frame => frame.width === 860 && frame.height === 640), 'Window stays fixed throughout dock motion');
    if (!latency) assert.ok(motion.frames.some(frame => frame.transform !== 'none' && frame.transform !== 'matrix(1, 0, 0, 1, 0, 0)'), 'Exercise actual terminal motion');
    assert.ok(motion.sizes.length >= 2 && motion.sizes[0].rows < motion.sizes.at(-1).rows, 'Exercise dock shrink then restore');
    assert.ok(motion.sizes.every(size => size.cols === beforeDock.size.cols), 'Dock motion leaves terminal width unchanged');
    assert.deepEqual(restored.size, motion.sizes.at(-1), 'Delayed dock shrink cannot replace restored terminal size');
    const final = motion.frames.at(-1);
    assert.ok(final.hostHeight - final.screenHeight >= 0 && final.hostHeight - final.screenHeight < 32, 'Restored terminal fills available height');
    await capture(`fixed-window-dock-motion-${latency}`);
  }

  for (const length of [240, 700, 1500, 3500, 7000]) {
    const text = 'abcdefghijklmnopqrst'.repeat(Math.ceil(length / 20)).slice(0, length);
    await execute(`printf '%s\\n' '${text}'`);
    const printed = await grid(`printed-${length}`);
    assert.ok(printed.core.slice(0, printed.cursor.row).join('').endsWith(text.slice(-Math.min(length, printed.size.cols * 2))), 'Long command printed its expected final bytes');
    await capture(`output-${length}`);
  }
  const prompt = process.env.CONN_AUTH_FIXTURE_RUNTIME ? 'remote$' : 'fixture$';
  const cols = (await agent.tool('snapshot')).size.cols;
  for (const padding of [0, cols * 2]) {
    const body = JSON.stringify({padding: 'x'.repeat(padding), agentMode: {state: 'unavailable'}});
    for (const [ending, format] of [['none', '%s'], ['lf', '%s\\n'], ['crlf', '%s\\r\\n']]) {
      await execute(`printf '${format}' '${body}'`, 'Compare synthetic JSON output with and without a trailing newline.');
      const label = `line-ending-${ending}-${padding ? 'wrapped' : 'short'}`;
      const result = await grid(label), last = result.visible[result.cursor.row];
      if (ending === 'none') assert.ok(last.endsWith(`}}${prompt}`), 'No newline: prompt follows the final JSON bytes');
      else { assert.equal(last, prompt, 'Newline: prompt starts on a separate line'); assert.ok(result.visible[result.cursor.row - 1].endsWith('}}')); }
      await capture(label);
    }
  }

  for (const text of ['long-input/'.repeat(100), '한글/경로/🙂/'.repeat(65)]) {
    const kind = text.startsWith('한') ? 'unicode' : 'ascii';
    await agent.tool('type', {text: `printf '%s\\n' '${text}'`});
    const size = (await agent.tool('snapshot')).size;
    const requestsBefore = resizes.length;
    for (let i = 0; i < 3; i++) {
      await page.keyboard.press('Control+Shift+,');
      await page.getByRole('dialog', {name: 'Settings', exact: true}).waitFor(); await delay(350);
      assert.deepEqual((await agent.tool('snapshot')).size, size, 'Settings must not resize the PTY');
      await page.keyboard.press('Escape');
      await page.getByRole('dialog', {name: 'Settings', exact: true}).waitFor({state: 'hidden'}); await delay(350);
    }
    assert.equal(resizes.length, requestsBefore, 'Settings modal must not send a terminal resize');
    await grid(`settings-${kind}`); await capture(`settings-${kind}`);
    assert.equal((await agent.tool('send_key', {key: 'ENTER', intent: 'Print the line after opening and closing Settings.'})).status, 'executed');
    await delay(500);
  }

  const absent = join(state, 'missing-' + 'deep/'.repeat(1000) + 'file');
  await agent.tool('type', {text: `rm -- '${absent}'`});
  const approval = await agent.tool('send_key', {key: 'ENTER', intent: 'Remove an absent synthetic fixture path.'});
  assert.equal(approval.status, 'pending');
  const allowSession = page.getByRole('button', {name: 'A allow this session', exact: true});
  await allowSession.waitFor(); await page.setViewportSize({width: 700, height: 540}); await delay(600);
  const box = await allowSession.boundingBox();
  assert.ok(box && box.y >= 0 && box.y + box.height <= 540, 'Long review keeps approval actions visible');
  await capture('long-approval-narrow');
  await page.setViewportSize({width: 860, height: 640}); await delay(600);
  assert.ok(await allowSession.isEnabled()); await allowSession.click();
  await until(async () => (await agent.tool('check_approval', {approvalId: approval.approvalId})).state === 'granted', 'Session allowance did not grant approval');
  await grid('allowed-session'); await capture('allow-session');
  await execute(`rm -- '${join(state, 'never-created')}'`, 'Verify the same session rule on another absent fixture path.');
  await delay(500);
  await agent.tool('type', {text: 'sudo id'});
  const unrelated = await agent.tool('send_key', {key: 'ENTER', intent: 'Verify another risk still asks, without running it.'});
  assert.equal(unrelated.status, 'pending');
  await page.locator('[data-request-key*=":review:"] .btn.danger').click();
  await until(async () => (await agent.tool('check_approval', {approvalId: unrelated.approvalId})).state === 'denied', 'Unrelated risk denial missing');
  await delay(500);

  const long = 'cursor-edit/'.repeat(85);
  await agent.tool('type', {text: `printf '<%s>\\n' '${long}END'`});
  const before = await grid('cursor-before-edit');
  await input().focus();
  for (let i = 0; i < 4; i++) await page.keyboard.press('ArrowLeft');
  const moved = await grid('cursor-moved');
  const start = Math.min(before.cursor.row, moved.size.rows - 1) * before.size.cols + before.cursor.col - 4;
  assert.deepEqual(moved.cursor, {row: Math.floor(start / before.size.cols), col: start % before.size.cols});
  await page.keyboard.insertText('EDIT'); await page.keyboard.press('End'); await page.keyboard.press('Enter');
  await until(async () => (await agent.tool('snapshot')).screen.join('').includes('EDITEND>'), 'Human cursor edit output missing');
  assert.equal((await agent.tool('snapshot')).controller.type, 'human');
  await grid('human-edited'); await capture('human-edited');
  if (process.env.CONN_AUTH_FIXTURE_RUNTIME) {
    await human('exit');
    await until(async () => (await agent.tool('snapshot')).screen.some(row => row.trim() === 'fixture$'), 'Outer local shell prompt did not return');
    await human("printf 'LOCAL_BACK_%s\\n' 42");
    await until(async () => (await agent.tool('snapshot')).screen.some(row => row === 'LOCAL_BACK_42'), 'Outer shell marker missing');
    await capture('ssh-returned-to-local');
  }
  assert.equal(await page.locator('[data-request-key^="connection:"]').count(), 0, 'Persistent MCP must not request repeat admission');
  assert.deepEqual(errors, [], 'Browser runtime errors');
  writeFileSync(join(evidence, 'transport.json'), JSON.stringify({resizes, outputFrames, pageErrors: errors, sshFixture: Boolean(process.env.CONN_AUTH_FIXTURE_RUNTIME)}, null, 2));
  if (!process.env.CONN_AUTH_FIXTURE_RUNTIME) console.log('SKIP optional SSH fixture: CONN_AUTH_FIXTURE_RUNTIME is not set');
  console.log(`PASS ${cases.length} production input/rendering scenarios\nEvidence: ${evidence}`);
} catch (error) {
  if (page) { await page.screenshot({path: join(evidence, 'failure.png')}).catch(() => {}); writeFileSync(join(evidence, 'failure-dom.txt'), await page.locator('body').innerText().catch(() => '')); }
  writeFileSync(join(evidence, 'transport.json'), JSON.stringify({resizes, outputFrames, pageErrors: errors}, null, 2));
  console.error(`Evidence: ${evidence}`); throw error;
} finally {
  for (const timer of timers) clearTimeout(timer);
  agent?.close(); await browser?.close(); server.kill();
  if (server.exitCode === null) await Promise.race([new Promise(resolve => server.once('exit', resolve)), delay(5000).then(() => server.kill('SIGKILL'))]);
}
