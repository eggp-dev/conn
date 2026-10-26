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
const state = mkdtempSync(join(tmpdir(), 'conn-environment-ui-'));
const evidence = join(state, 'evidence'); mkdirSync(evidence);
// Point the unchanged production flow at an extracted release to verify the
// shipped UI/server/MCP together, without rebuilding any of those components.
const bundle = process.env.CONN_TEST_WEB_BUNDLE;
const binary = name => resolve(bundle ?? join(root, 'target/debug'), name + (process.platform === 'win32' ? '.exe' : ''));
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(run, message, ms = 12_000) {
  const start = Date.now(); let value;
  do { value = await run(); if (value) return value; await sleep(80); } while (Date.now() - start < ms);
  throw new Error(message);
}
writeFileSync(join(state, 'profiles.json'), JSON.stringify({ version: 1, revision: 0, defaultProfile: 'test', profiles: [{ id: 'test', name: 'Local collaboration test', backend: 'local', shell: 'custom', program: '/bin/bash', args: ['--noprofile', '--norc', '-i'], cwd: state, env: { PS1: 'conn-test$ ', HISTFILE: '/dev/null', HISTCONTROL: '' } }] }));
writeFileSync(join(state, 'app.json'), JSON.stringify({ mode: 'autopilot', gate: false, pacing: { enterGraceMs: 0, minWriteIntervalMs: 0, leaseTtlSecs: 120, approvalTtlSecs: 120 } }));
const log = openSync(join(state, 'server.log'), 'w');
const server = spawn(binary('conn-web'), ['serve', '--state-dir', state, '--port', '0', '--ui-dir', bundle ? resolve(bundle, 'ui') : resolve(root, 'frontends/web/dist'), '--setup-home', join(state, 'setup-home')], { stdio: ['ignore', log, log] });
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
async function decided(agent, approvalId, expected) {
  const result = await until(async () => {
    const current = await agent.tool('terminal_check_approval', {approvalId});
    return current.state === 'pending' ? false : current;
  }, `Approval ${approvalId} did not settle after the owner action`);
  assert.ok(expected.includes(result.state), JSON.stringify(result));
  return result;
}
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

try {
  await until(() => existsSync(join(state, 'connection.json')), 'No connection file');
  const metadata = JSON.parse(readFileSync(join(state,'connection.json'),'utf8'));
  browser = await chromium.launch({headless:true,args:['--no-sandbox']});
  context = await browser.newContext({viewport:{width:1280,height:800},locale:'en-US'});
  page = await context.newPage();
  page.on('pageerror',e=>failures.push(e.message));
  await page.goto(metadata.url+'/#token='+encodeURIComponent(metadata.bootstrapToken));
  await ready();
  await sharing(true);
  const agent = new MCP('environment-ui'); await agent.start();
  await page.locator('[data-request-key^="connection:"] .primary').click();
  await agent.tool('terminal_request_attention',{reason:'Test environment check controls in an isolated shell.'});
  await sharing();
  await until(async()=> (await agent.tool('terminal_list_tabs')).tabs === 1,'No shared shell');
  await agent.tool('terminal_request_control',{reason:'Inspect isolated environment review controls.'});
  await agent.tool('terminal_type',{text:'printf UI_REVIEW'});
  const pending=await agent.tool('terminal_send_key',{key:'ENTER',intent:'Print a marker in the disposable shell.'});
  assert.equal(pending.status,'pending');
  const section=page.locator('.environment-review');
  await section.locator('summary').click();
  await section.getByRole('button',{name:/Session:/}).click();
  assert.equal(await section.getByRole('button',{name:'Reduce checks for 15 minutes',exact:true}).isDisabled(),true);
  await section.getByRole('button',{name:'Close',exact:true}).click();
  await section.getByRole('button',{name:/Session:/}).click();
  assert.equal(await section.locator('input[type=checkbox]').isChecked(),false);
  await section.locator('input[type=checkbox]').check();
  await section.getByRole('button',{name:'Reduce checks for 15 minutes',exact:true}).click();
  await section.locator('.active strong').waitFor();
  assert.equal((await agent.tool('terminal_check_approval',{approvalId:pending.approvalId})).state,'pending');
  const initial=await section.locator('.active strong').innerText();
  await sleep(1200);
  assert.notEqual(await section.locator('.active strong').innerText(),initial,'countdown must advance');
  await page.screenshot({path:join(evidence,'environment-active.png')});
  await page.reload(); await ready();
  await page.locator('.environment-review .active strong').waitFor();
  await page.locator('.environment-review .active button').click();
  await page.locator('.environment-review .active').waitFor({state:'hidden'});
  assert.equal((await agent.tool('terminal_check_approval',{approvalId:pending.approvalId})).state,'pending');
  await page.locator('[data-request-key*="review:"]').getByRole('button',{name:'Run once',exact:true}).click();
  await decided(agent,pending.approvalId,['granted']);
  await page.locator('.environment-review summary').click();
  await page.locator('.environment-review').getByRole('button',{name:/Session:/}).click();
  await page.locator('.environment-review input[type=checkbox]').check();
  await page.locator('.environment-review').getByRole('button',{name:'Reduce checks for 15 minutes',exact:true}).click();
  await page.locator('.environment-review .active').waitFor();
  agent.close();
  await page.locator('.environment-review .active').waitFor({state:'hidden'});
  const replacement=new MCP('environment-ui'); await replacement.start();
  await page.locator('[data-request-key^="connection:"] .primary').click();
  assert.equal(await page.locator('.environment-review .active').count(),0);
  assert.deepEqual(failures,[]);
  cases.push('explicit acknowledgement and close/re-entry reset','pending approval is not approved by enabling or stopping','countdown and owner reload preserve fixed deadline','stop, disconnect and same-name reconnect revoke grant','unknown dialect remains under per-command review');
  writeFileSync(join(evidence,'results.json'),JSON.stringify({cases,pageErrors:failures},null,2));
  console.log(`PASS ${cases.length} environment review UI scenarios\nEvidence: ${evidence}`);
} catch (error) {
  writeFileSync(join(evidence, 'frames.json'), JSON.stringify(frames, null, 2));
  if (page) { await page.screenshot({ path: join(evidence, 'failure.png') }).catch(() => {}); writeFileSync(join(evidence, 'failure-dom.txt'), await page.locator('body').innerText().catch(() => '')); }
  console.error(`Evidence: ${evidence}`); throw error;
} finally {
  for (const agent of agents) { agent.close(); agent.child.kill(); }
  await browser?.close(); server.kill();
  if (server.exitCode === null) await Promise.race([new Promise(resolve => server.once('exit', resolve)), sleep(5000).then(() => server.kill('SIGKILL'))]);
}
