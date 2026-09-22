// Production web bundle + real Rust host + one persistent, matching MCP process.
// All owner actions use the rendered UI. No replacement app, runtime or store hook.
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import { mkdtempSync, writeFileSync, readFileSync, existsSync, mkdirSync, openSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';
import { strict as assert } from 'node:assert';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const state = mkdtempSync(join(tmpdir(), 'conn-web-mcp-'));
const evidence = join(state, 'evidence'); mkdirSync(evidence);
const binary = name => resolve(root, 'target/debug', name + (process.platform === 'win32' ? '.exe' : ''));
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(run, message, ms = 12_000) {
  const start = Date.now(); let value;
  do { value = await run(); if (value) return value; await sleep(80); } while (Date.now() - start < ms);
  throw new Error(message);
}
writeFileSync(join(state, 'profiles.json'), JSON.stringify({ version: 1, revision: 0, defaultProfile: 'test', profiles: [{ id: 'test', name: 'Local collaboration test', backend: 'local', shell: 'posix', program: '/bin/bash', args: ['--noprofile', '--norc', '-i'], cwd: state, env: { PS1: 'conn-test$ ', HISTFILE: '/dev/null', HISTCONTROL: '' } }] }));
writeFileSync(join(state, 'app.json'), JSON.stringify({ mode: 'autopilot', gate: true, pacing: { enterGraceMs: 0, minWriteIntervalMs: 0, leaseTtlSecs: 120, approvalTtlSecs: 120 } }));
const log = openSync(join(state, 'server.log'), 'w');
const server = spawn(binary('conn-web'), ['serve', '--state-dir', state, '--port', '0', '--ui-dir', resolve(root, 'frontends/web/dist'), '--setup-home', join(state, 'setup-home')], { stdio: ['ignore', log, log] });
let browser, context, page;
const agents = [], failures = [], cases = [], frames = [];
let rejectNextInput = false, rejectedInput = false;
class MCP {
  constructor(name) {
    this.pending = new Map(); this.id = 0;
    this.child = spawn(binary('conn'), ['--socket', join(state, 'conn.sock'), 'mcp', '--agent-id', name, '--tools', 'static'], { stdio: ['pipe', 'pipe', log] });
    createInterface({ input: this.child.stdout }).on('line', line => {
      const message = JSON.parse(line), pending = this.pending.get(message.id);
      if (!pending) return; this.pending.delete(message.id); clearTimeout(pending.timer);
      if (message.error) pending.reject(new Error(JSON.stringify(message.error))); else pending.resolve(message.result);
    });
    agents.push(this);
  }
  call(method, params = {}) {
    const id = ++this.id;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`MCP timeout: ${method} ${params.name ?? ''}`)); }, 18_000);
      this.pending.set(id, { resolve, reject, timer });
      this.child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
    });
  }
  async start() {
    await this.call('initialize', { protocolVersion: '2025-06-18', clientInfo: { name: 'Conn integration test', version: '1' }, capabilities: {} });
    await this.call('tools/list');
  }
  async tool(name, args = {}) {
    const result = await this.call('tools/call', { name, arguments: args });
    const text = result.content?.find(c => c.type === 'text')?.text;
    if (result.isError) throw new Error(`${name}: ${text}`);
    return JSON.parse(text);
  }
  close() { this.child.stdin.end(); for (const p of this.pending.values()) clearTimeout(p.timer); this.pending.clear(); }
}
const ready = () => page.locator('.host:not([hidden])[data-terminal-ready="true"]').waitFor();
const input = () => page.locator('.host:not([hidden]) .xterm-helper-textarea');
async function human(text, enter = true) { await ready(); await input().focus(); await page.keyboard.insertText(text); if (enter) await page.keyboard.press('Enter'); }
async function sharing(stop = false) {
  const preparation = page.locator('[data-request-key^="preparation:"] .primary');
  if (!stop && await preparation.isVisible()) await preparation.click();
  else {
    const trigger = page.locator('.sharing-trigger');
    if (await trigger.isVisible()) await trigger.click();
    else await page.locator('button.dot:not(.hidden)').click();
    await page.locator('.sharing-row button').click();
  }
  await page.locator('.sharing').waitFor();
  if (stop) await page.locator('.sharing footer .ghost').click();
  else { await page.locator('.sharing input[type="checkbox"]').first().check(); await page.locator('.sharing footer .btn:not(.ghost)').click(); }
  await page.locator('.sharing').waitFor({ state: 'hidden' });
}
let verifiedControlUx = false;
async function approveControl(agent) {
  await input().focus();
  const beforeGrid = await page.locator('.host:not([hidden])').boundingBox();
  const control = agent.tool('terminal_request_control', { reason: 'Verify the shared shell through the production web owner.' });
  control.catch(() => {});
  // Control may return a pollable pending result before the UI acts.
  await page.locator('[data-request-key*="\u003acontrol:"] .primary').waitFor();
  if (!verifiedControlUx) {
    const panel = page.locator('.control-morph');
    assert.equal(await page.locator('.interaction-dock [data-request-key*="\u003acontrol:"]').count(), 0);
    assert.equal(await input().evaluate(el => el === document.activeElement), true, 'new request must not steal typing focus');
    const afterGrid = await page.locator('.host:not([hidden])').boundingBox();
    assert.deepEqual([afterGrid.width,afterGrid.height], [beforeGrid.width,beforeGrid.height], 'badge morph never changes the PTY layout');
    await page.screenshot({path:join(evidence,'control-open.png')});
    await panel.getByRole('button',{name:'Later',exact:true}).click();
    await panel.waitFor({state:'hidden'});
    assert.equal(await page.locator('button.dot.pending').count(), 1, 'collapsed request remains pending');
    await page.screenshot({path:join(evidence,'control-collapsed.png')});
    await page.locator('button.dot.pending').click();
    await panel.waitFor();
    await page.keyboard.press('Escape');
    await panel.waitFor({state:'hidden'});
    await page.emulateMedia({reducedMotion:'reduce'});
    await page.setViewportSize({width:640,height:420});
    await page.locator('button.dot.pending').click();
    await panel.waitFor();
    const box = await panel.boundingBox();
    assert.ok(box.x >= 0 && box.x+box.width <= 640 && box.y+box.height <= 420, 'compact request remains inside the viewport');
    const primaryBox = await panel.locator('.primary').boundingBox();
    assert.ok(primaryBox && primaryBox.y + primaryBox.height < 420, 'compact request keeps the decision buttons on screen');
    await sleep(400);
    await page.screenshot({path:join(evidence,'control-compact-reduced.png')});
    await page.setViewportSize({width:1280,height:800});
    await page.emulateMedia({reducedMotion:'no-preference'});
    await sleep(600);
    verifiedControlUx = true;
    cases.push('badge control morph, no focus or PTY resize, collapse/reopen/Escape, compact and reduced motion');
  }
  await page.locator('[data-request-key*="\u003acontrol:"] .primary').click();
  const result = await control;
  if (result.status === 'pending') {
    const decided = await agent.tool('terminal_check_approval', { approvalId: result.approvalId });
    assert.ok(['granted', 'executed'].includes(decided.state), JSON.stringify(decided));
  }
}
async function marker(agent, text, allowInitialReview = false) {
  await agent.tool('terminal_type', { text: `printf '%s\\n' '${text}'` });
  const result = await agent.tool('terminal_send_key', { key: 'ENTER', intent: 'Print a synthetic verification marker.' });
  if (allowInitialReview && result.status === 'pending') {
    await page.locator('[data-request-key*="\u003areview:"] .btn.ok').click();
    assert.equal((await agent.tool('terminal_check_approval', { approvalId: result.approvalId })).state, 'granted');
  } else assert.equal(result.status, 'executed', JSON.stringify(result));
  return until(async () => { const snap = await agent.tool('terminal_snapshot'); return snap.screen?.some(row => row.trim() === text) ? snap : false; }, `Missing marker ${text}`);
}
try {
  await until(() => { if (server.exitCode !== null) throw new Error('Web host failed to start'); return existsSync(join(state, 'connection.json')); }, 'No connection file');
  const metadata = JSON.parse(readFileSync(join(state, 'connection.json'), 'utf8'));
  browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  context = await browser.newContext({ viewport: { width: 1280, height: 800 }, locale: 'en-US' });
  page = await context.newPage();
  page.on('pageerror', error => failures.push(error.message));
  await page.routeWebSocket('**/api/ws', socket => {
    const upstream = socket.connectToServer();
    socket.onMessage(message => {
      const request = JSON.parse(message.toString());
      if (rejectNextInput && request.name === 'input') {
        rejectNextInput = false; rejectedInput = true;
        // A server capacity failure before sequence consumption. Forward every
        // other real frame unchanged; do not simulate any app or PTY state.
        socket.send(JSON.stringify({ id: request.id, error: 'owner_busy', code: 'owner_busy' }));
      } else upstream.send(message);
    });
    upstream.onMessage(message => { frames.push(JSON.parse(message.toString())); socket.send(message); });
  });
  await page.goto(metadata.url + '/#token=' + encodeURIComponent(metadata.bootstrapToken));
  await ready(); assert.equal(new URL(page.url()).hash, '');
  await until(async () => (await page.locator('.host:not([hidden]) .xterm-rows').innerText()).includes('conn-test$'), 'Initial shell prompt');
  const sid = await page.locator('.host:not([hidden])').getAttribute('data-session');
  await human('export CONN_RETAINED=retained_42');
  await sleep(200);
  await sharing(true);
  cases.push('owner input and private-shell transition');

  const agent = new MCP('web-collaboration'); await agent.start();
  await page.locator('[data-request-key^="connection:"]').waitFor();
  await until(async () => {
    const card = await page.locator('.connection-dock').boundingBox();
    const terminal = await page.locator('.host:not([hidden])').boundingBox();
    return card && terminal && terminal.y >= card.y + card.height;
  }, 'Connection admission must reserve space instead of hiding the prompt');
  await page.screenshot({path:join(evidence,'connection-admission.png')});
  await page.locator('[data-request-key^="connection:"] .primary').click();
  const hidden = await agent.tool('terminal_list_tabs'); assert.equal(hidden.tabs, 0, JSON.stringify(hidden));
  await agent.tool('terminal_request_attention', { reason: 'Choose a shell for the collaboration test.' });
  await page.locator('[data-request-key^="preparation:"]').waitFor();
  await sharing();
  await until(async () => (await agent.tool('terminal_list_tabs')).tabs === 1, 'Shared shell not visible');
  await page.locator('[data-request-key^="preparation:"]').waitFor({state:'hidden'});
  await approveControl(agent);
  writeFileSync(join(evidence, 'first-snapshot.json'), JSON.stringify(await agent.tool('terminal_snapshot'), null, 2));
  // Sharing invalidates knowledge of the private shell's command context. Review
  // the first command explicitly; its real completion establishes the next prompt.
  await marker(agent, 'FIRST_MCP_OK', true);
  await marker(agent, 'SECOND_MCP_OK');
  assert.equal(await page.locator('[data-request-key^="connection:"]').count(), 0, 'one persistent MCP connection must not re-request admission');
  cases.push('sessionless attention, admission, selected sharing, lease and repeated safe commands');

  // A real risk review; deny it so no destructive fixture action is executed.
  await agent.tool('terminal_type', { text: 'rm -rf ./synthetic-only' });
  const risk = await agent.tool('terminal_send_key', { key: 'ENTER', intent: 'Exercise a risky-command denial in the isolated test directory.' });
  assert.equal(risk.status, 'pending');
  const reviewCard = page.locator('[data-request-key*="\u003areview:"]');
  assert.equal(await reviewCard.getByRole('button',{name:'Run once',exact:true}).count(), 1);
  assert.equal(await reviewCard.locator('.scope-choice').getAttribute('open'), null, 'broad grant stays behind an explicit scope choice');
  await sleep(600);
  assert.deepEqual(await page.locator('.app').evaluate(el=>[el.scrollLeft,el.scrollTop]),[0,0],'focus after a compact-window transition must not pan the application frame');
  await page.screenshot({path:join(evidence,'command-review.png')});
  const deny = page.locator('[data-request-key*="\u003areview:"] button').filter({ hasText: /deny|reject/i }).first();
  await deny.click();
  assert.equal((await agent.tool('terminal_check_approval', { approvalId: risk.approvalId })).state, 'denied');
  await agent.tool('terminal_interrupt'); await sleep(180);
  await marker(agent, 'AFTER_IDLE_INTERRUPT_OK');
  cases.push('risky command review and denial; idle Ctrl-C returns to a usable shell prompt');
  await agent.tool('terminal_type', {text:'rm ./synthetic-only'});
  const typingRisk = await agent.tool('terminal_send_key', {key:'ENTER',intent:'Verify human typing preempts pending execution.'});
  assert.equal(typingRisk.status,'pending');
  await human('a',false);
  await until(async () => (await agent.tool('terminal_check_approval',{approvalId:typingRisk.approvalId})).state === 'denied', 'typing did not deny the pending command');
  await page.keyboard.press('Control+c'); await sleep(180);
  await approveControl(agent);
  await marker(agent,'AFTER_HUMAN_LETTER_OK');
  cases.push('typing a in the shell takes over instead of approving an execution');


  const long = 'long-' + 'abcdef'.repeat(500);
  await agent.tool('terminal_type', { text: `printf '%s\\n' '${long}'` });
  assert.equal((await agent.tool('terminal_send_key', { key: 'ENTER', intent: 'Print synthetic long text for renderer verification.' })).status, 'executed');
  await sleep(250);
  await marker(agent, 'AFTER_LONG_INPUT_OK');
  await page.screenshot({ path: join(evidence, 'long-command.png') });
  await agent.tool('terminal_release_control');
  await page.reload(); await ready();
  assert.equal(await page.locator('.host:not([hidden])').getAttribute('data-session'), sid);
  await approveControl(agent);
  await agent.tool('terminal_type', { text: 'printf "%s\\n" "$CONN_RETAINED"' });
  assert.equal((await agent.tool('terminal_send_key', { key: 'ENTER', intent: 'Read the shell variable set before the browser reload.' })).status, 'executed');
  await until(async () => (await agent.tool('terminal_snapshot')).screen.some(row => row.trim() === 'retained_42'), 'Reload replaced or lost the original shell');
  cases.push('long wrapping input and renderer reload preserve session and process state');

  const second = await context.newPage();
  await second.goto(metadata.url); await second.getByRole('button', { name: 'Continue here' }).waitFor();
  await second.getByRole('button', { name: 'Continue here' }).click();
  await second.locator('.host:not([hidden])[data-terminal-ready="true"]').waitFor();
  await page.getByRole('button', { name: 'Continue here' }).waitFor();
  assert.equal(await second.locator('.host:not([hidden])').getAttribute('data-session'), sid);
  await page.getByRole('button', { name: 'Continue here' }).click(); await ready(); await second.close();
  cases.push('duplicate owner refuses silent attachment; explicit takeover preserves the shell and fences the old renderer');

  // New agent-created tabs cannot silently move the human's selected view.
  const opened = await agent.tool('terminal_open_tab', { reason: 'Verify background tab visibility.' });
  await until(async () => (await page.getByRole('tab').count()) === 2, 'Agent tab not rendered');
  assert.equal(await page.locator('.host:not([hidden])').getAttribute('data-session'), sid);
  const tabs = await agent.tool('terminal_list_tabs');
  writeFileSync(join(evidence, 'tab-result.json'), JSON.stringify({ opened, tabs }, null, 2));
  await page.screenshot({ path: join(evidence, 'background-agent-tab.png') });
  cases.push('agent tab appears in the common tab strip without moving the human view');

  rejectNextInput = true;
  await human('SHOULD_NEVER_REPLAY', false);
  await page.getByRole('button', { name: 'Reconnect', exact: true }).waitFor();
  assert.ok(rejectedInput);
  await page.getByRole('button', { name: 'Reconnect', exact: true }).click();
  await ready();
  assert.equal(await page.locator('.host:not([hidden])').getAttribute('data-session'), sid);
  await agent.tool('terminal_switch_tab', { tab: sid });
  assert.ok(!(await agent.tool('terminal_snapshot')).screen.join('\n').includes('SHOULD_NEVER_REPLAY'));
  await human("printf '%s\\n' RECOVERED_INPUT_OK");
  await until(async () => (await agent.tool('terminal_snapshot')).screen.some(row => row.trim() === 'RECOVERED_INPUT_OK'), 'New owner epoch did not restore ordered input');
  cases.push('unconsumed input failure requires explicit reattachment, never replays old input and restores the sequence');

  agent.close();
  const replacement = new MCP('web-collaboration'); await replacement.start();
  await page.locator('[data-request-key^="connection:"]').waitFor();
  await page.locator('[data-request-key^="connection:"] .ghost').click();
  cases.push('same-name new MCP process requests fresh admission');
  assert.ok(frames.some(f => f.event === 'ss:output' && f.payload.reset && f.payload.epoch > 0 && f.payload.streamSeq > 0));
  assert.deepEqual(failures, [], 'browser runtime errors');
  writeFileSync(join(evidence, 'results.json'), JSON.stringify({ cases, pageErrors: failures, outputFrames: frames.filter(f => f.event === 'ss:output').length }, null, 2));
  console.log(`PASS ${cases.length} production web/MCP scenarios\nEvidence: ${evidence}`);
} catch (error) {
  writeFileSync(join(evidence, 'frames.json'), JSON.stringify(frames, null, 2));
  if (page) { await page.screenshot({ path: join(evidence, 'failure.png') }).catch(() => {}); writeFileSync(join(evidence, 'failure-dom.txt'), await page.locator('body').innerText().catch(() => '')); }
  console.error(`Evidence: ${evidence}`); throw error;
} finally {
  for (const agent of agents) { agent.close(); agent.child.kill(); }
  await browser?.close(); server.kill();
  if (server.exitCode === null) await Promise.race([new Promise(resolve => server.once('exit', resolve)), sleep(5000).then(() => server.kill('SIGKILL'))]);
}
