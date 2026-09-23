// Production web bundle, real Rust PTYs and one persistent matching Conn MCP.
// Actual vi editing and saving through the DOM; no replacement UI or injected input.
// Run on a disposable local/loopback fixture. Never use a production SSH config.
import {chromium, webkit} from 'playwright';
import {spawn} from 'node:child_process';
import {mkdtempSync, writeFileSync, readFileSync, existsSync, mkdirSync, openSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve, basename} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createInterface} from 'node:readline';
import {strict as assert} from 'node:assert';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const state = mkdtempSync(join(tmpdir(), 'conn-editor-input-'));
const evidence = process.env.CONN_EDITOR_OUTPUT ?? join(state, 'evidence');
const sshFixture = Boolean(process.env.CONN_AUTH_FIXTURE_RUNTIME || process.env.CONN_SSH_FIXTURE_CONFIG);
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
const errors = [], layoutWarnings = [], cases = [];
const browserName = process.env.CONN_EDITOR_BROWSER ?? 'chromium';
assert.ok(['chromium', 'webkit'].includes(browserName), 'Choose chromium or webkit');

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
  const requested = agent.tool('request_control', {reason: 'Verify that human Vim input preempts agent control in this disposable fixture.'});
  await page.locator('[data-request-key*=":control:"] .primary').click();
  const result = await requested;
  assert.equal(result.type, 'agent', 'Control request resolves to the live agent controller');
}
async function capture(name, extra = {}) {
  cases.push({name, ...extra});
  writeFileSync(join(evidence, 'cases.json'), JSON.stringify(cases, null, 2));
  await page.screenshot({path: join(evidence, `${name}.png`)});
  console.log(`PASS ${name}`);
}

try {
  await until(() => { if (server.exitCode !== null) throw new Error('Web host failed to start'); return existsSync(join(state, 'connection.json')); }, 'No web connection file');
  const metadata = JSON.parse(readFileSync(join(state, 'connection.json'), 'utf8'));
  browser = await (process.env.CONN_EDITOR_BROWSER === 'webkit' ? webkit : chromium).launch({headless: true});
  context = await browser.newContext({viewport: {width: 860, height: 640}, locale: 'en-US'});
  page = await context.newPage(); page.setDefaultTimeout(12_000);
  page.on('pageerror', error => {
    // Previously observed in native Mac and WebKit runs. Preserve it separately;
    // it is not evidence that editing failed or that the warning is harmless.
    if (error.message === 'ResizeObserver loop completed with undelivered notifications.') layoutWarnings.push(error.message);
    else errors.push(error.message);
  });
  await page.goto(metadata.url + '/#token=' + encodeURIComponent(metadata.bootstrapToken));
  await ready(); await delay(600);
  if (process.env.CONN_SSH_FIXTURE_CONFIG) {
    // A disposable key-authenticated loopback fixture also works without
    // installing a password-authenticated account on the macOS host.
    const config = process.env.CONN_SSH_FIXTURE_CONFIG;
    assert.ok(/^[A-Za-z0-9_./-]+$/.test(config), 'SSH fixture config path must contain no shell metacharacters');
    assert.ok(existsSync(config), 'SSH fixture config must exist');
    await human(`${process.env.CONN_SSH_COMPOUND_ENTRY === "1" ? "printf PRE_SSH; " : ""}ssh -F ${config} validation-host`);
    await until(async () => (await host().locator('.xterm-rows').innerText()).includes('remote$'), 'SSH fixture prompt missing');
    await human('printf "REMOTE_READY_%s\\n" "$((6*7))"');
    await until(async () => (await host().locator('.xterm-rows').innerText()).includes('REMOTE_READY_42'), 'SSH fixture ready marker missing');
  } else if (process.env.CONN_AUTH_FIXTURE_RUNTIME) {
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
  const target = sshFixture ? `/tmp/${basename(state)}.txt` : join(state, 'editor.txt');
  assert.match(target, /^[A-Za-z0-9_./-]+$/);
  for (const shared of [false, true]) {
    if (shared) {
      agent = new MCP(); await agent.start();
      await page.locator('[data-request-key^="connection:"] .primary').click();
      if ((await agent.tool('list_tabs')).tabs === 0) await share();
      await control();
    }
    for (const reconnect of [false, true]) {
      const marker = `EDITOR_TYPED_${shared ? 'SHARED' : 'PRIVATE'}_${reconnect ? 'REATTACHED' : 'LIVE'}`;
      await human(`vi -Nu NONE -n -i NONE ${target}`);
      await until(async () => (await host().innerText()).includes('~'), 'vi screen did not appear; fixture requires vi');
      if (reconnect) { await page.reload(); await ready(); await delay(500); }
      // Do not refocus the textarea here: that would hide a product focus regression.
      assert.equal(await input().evaluate(el => el === document.activeElement), true, 'Vim must retain terminal focus');
      await page.keyboard.press('Escape');
      await page.keyboard.type('ggdGi' + marker, {delay: 30});
      await until(async () => (await host().innerText()).includes(marker), 'Vim did not render human input');
      await page.keyboard.press('ArrowLeft');
      await page.keyboard.press('Backspace');
      await page.keyboard.type(marker.at(-2));
      await page.keyboard.press('Escape');
      await page.keyboard.type(':wq');
      await page.keyboard.press('Enter');
      await until(async () => (await host().locator('.xterm-rows > div').allTextContents()).some(row => row.trim() === (sshFixture ? 'remote$' : 'fixture$')), 'Vim did not return to the shell');
      if (!sshFixture) assert.equal(readFileSync(target, 'utf8'), marker + '\n', 'Vim saved the exact edited bytes');
      await human(`printf 'EDITOR_FILE_%s\\n' "$(cat -- ${target})"`);
      await until(async () => (await host().locator('.xterm-rows > div').allTextContents()).some(row => row.trim() === 'EDITOR_FILE_' + marker), 'Saved editor content missing from the real shell');
      if (shared) assert.equal((await agent.tool('snapshot')).controller.type, 'human', 'Typing in Vim retains human control');
      await capture(`${shared ? 'shared' : 'private'}-${reconnect ? 'reattached' : 'live'}`, {browser: browserName, ssh: sshFixture});
    }
  }
  // A real configuration-sized buffer exercises scrolling and the last-row
  // textarea placement that an empty-file test cannot cover.
  const lines = Array.from({length: 100}, (_, i) => `FIXTURE_${i}=value_${i}`);
  await human(`printf '%s\\n' '${lines.join("' '")}' > ${target}`);
  await human(`vi -Nu NONE -n -i NONE ${target}`);
  await until(async () => (await host().innerText()).includes('FIXTURE_0=value_0'), 'Long Vim buffer missing');
  await page.keyboard.type('G');
  await page.setViewportSize({width: 700, height: 440});
  await delay(500);
  assert.equal(await input().evaluate(el => el === document.activeElement), true, 'Resize must not steal Vim focus');
  await page.keyboard.type('A_EDITED', {delay: 30});
  await page.keyboard.press('Escape');
  await page.keyboard.type(':wq');
  await page.keyboard.press('Enter');
  await delay(500);
  await human(`printf 'EDITOR_LAST_%s\\n' "$(tail -n 1 ${target})"`);
  await until(async () => (await host().locator('.xterm-rows > div').allTextContents()).some(row => row.trim() === 'EDITOR_LAST_FIXTURE_99=value_99_EDITED'), 'Last-row edit did not persist');
  if (!sshFixture) assert.equal(readFileSync(target, 'utf8'), lines.slice(0, -1).concat(lines.at(-1) + '_EDITED').join('\n') + '\n');
  await capture('shared-long-buffer-resize', {browser: browserName, ssh: sshFixture});
  await human(`rm -- ${target}`);
  writeFileSync(join(evidence, 'runtime.json'), JSON.stringify({errors, layoutWarnings}, null, 2));
  if (layoutWarnings.length) console.warn(`OBSERVED ${layoutWarnings.length} ResizeObserver warning(s); see runtime.json`);
  assert.deepEqual(errors, [], 'Unexpected browser runtime errors');
  console.log(`PASS ${cases.length} actual vi edit/save scenarios (${browserName}, ${sshFixture ? 'SSH' : 'local'})\nEvidence: ${evidence}`);
} catch (error) {
  if (page) { await page.screenshot({path: join(evidence, 'failure.png')}).catch(() => {}); writeFileSync(join(evidence, 'failure-dom.txt'), await page.locator('body').innerText().catch(() => '')); }
  writeFileSync(join(evidence, 'errors.json'), JSON.stringify({errors, layoutWarnings}, null, 2));
  console.error(`Evidence: ${evidence}`); throw error;
} finally {
  agent?.close(); await browser?.close(); server.kill();
  if (server.exitCode === null) await Promise.race([new Promise(resolve => server.once('exit', resolve)), delay(5000).then(() => server.kill('SIGKILL'))]);
}
