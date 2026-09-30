// Built shared UI, real Rust PTYs and distinct live MCP connections, including
// duplicate names. Isolated settings; no production shell or user profile edits.
import {chromium} from 'playwright';
import {spawn} from 'node:child_process';
import {mkdtempSync, mkdirSync, writeFileSync, readFileSync, openSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve, join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createInterface} from 'node:readline';
import assert from 'node:assert/strict';
const root=fileURLToPath(new URL('../../../',import.meta.url));
const state=mkdtempSync(join(tmpdir(),'conn-header-input-'));
const evidence=process.env.CONN_REGRESSION_OUTPUT??join(state,'evidence');mkdirSync(evidence,{recursive:true});
const binary=name=>resolve(process.env.CONN_TEST_TARGET_DIR??join(root,'target'),'debug',name+(process.platform==='win32'?'.exe':''));
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const agents=[], cases=[], errors=[];
let browser, page;
writeFileSync(join(state,'app.json'),JSON.stringify({mode:'autopilot',gate:true,pacing:{enterGraceMs:0,minWriteIntervalMs:0,leaseTtlSecs:120,approvalTtlSecs:120}}));
writeFileSync(join(state,'profiles.json'),JSON.stringify({version:1,revision:0,defaultProfile:'fixture',profiles:[{id:'fixture',name:'Header and input regression',backend:'local',shell:'posix',program:'/bin/bash',args:['--noprofile','--norc','-i'],env:{PS1:'fixture$ ',HISTFILE:'/dev/null'},cwd:state}]}));
const log=openSync(join(state,'runtime.log'),'w');
const server=spawn(binary('conn-web'),['serve','--state-dir',state,'--port','0','--ui-dir',resolve(root,'frontends/web/dist'),'--setup-home',join(state,'setup-home')],{stdio:['ignore',log,log]});
async function until(run,label) {
 const end=Date.now()+15000;
 do {const value=await run();if(value)return value;await delay(80);} while(Date.now()<end);
 throw new Error('Timeout: '+label);
}
class MCP {
 constructor(name) {
  this.id=0;this.pending=new Map();this.closed=false;this.name=name;
  this.child=spawn(binary('conn'),['--socket',join(state,'conn.sock'),'mcp','--agent-id',name,'--tools','static'],{stdio:['pipe','pipe',log]});
  createInterface({input:this.child.stdout}).on('line',line=>{
   const message=JSON.parse(line), pending=this.pending.get(message.id);if(!pending)return;
   this.pending.delete(message.id);clearTimeout(pending.timer);
   if(message.error)pending.reject(new Error(JSON.stringify(message.error)));else pending.resolve(message.result);
  });agents.push(this);
 }
 call(method,params={}) {
  const id=++this.id;
  return new Promise((resolve,reject)=>{
   const timer=setTimeout(()=>{this.pending.delete(id);reject(new Error('MCP timeout: '+method));},18000);
   this.pending.set(id,{resolve,reject,timer});this.child.stdin.write(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n');
  });
 }
 async start() {
  const response=await this.call('initialize',{protocolVersion:'2025-06-18',clientInfo:{name:'Conn regression test',version:'1'},capabilities:{}});
  assert.equal(response.serverInfo.version,JSON.parse(readFileSync(join(root,'packages/ui/package.json'),'utf8')).version);
  await this.call('tools/list');
 }
 async tool(name,args={}) {
  const result=await this.call('tools/call',{name:'terminal_'+name,arguments:args});
  const text=result.content.find(item=>item.type==='text').text;
  return result.isError?{error:text}:JSON.parse(text);
 }
 close() {if(this.closed)return;this.closed=true;this.child.stdin.end();this.child.kill();for(const pending of this.pending.values())clearTimeout(pending.timer);this.pending.clear();}
}
const input=()=>page.locator('.host:not([hidden]) .xterm-helper-textarea');
const summary=()=>page.locator('.tab-item .agents .trigger');
async function human(text='',keys=[]) {
 await input().focus();if(text)await page.keyboard.insertText(text);
 for(const key of keys){await page.keyboard.press(key);await delay(80);}await delay(200);
}
async function shareAll() {
 await page.locator('button.dot:not(.hidden)').click();await page.locator('.sharing-row button').click();
 const checks=page.locator('.sharing input[type="checkbox"]');
 await until(async()=>await checks.count()===agents.filter(agent=>!agent.closed).length,'live sharing candidates');
 for(const checkbox of await checks.all())await checkbox.check();
 await page.locator('.sharing footer .btn:not(.ghost)').click();await page.locator('.sharing').waitFor({state:'hidden'});
}
async function attach(name) {
 const agent=new MCP(name);await agent.start();await page.locator('[data-request-key^="connection:"] .primary').click();
 await page.locator('[data-request-key^="connection:"]').waitFor({state:'hidden'});await shareAll();
 assert.equal((await agent.tool('list_tabs')).tabs,1);await agent.tool('snapshot');return agent;
}
async function control(agent) {
 const promise=agent.tool('request_control',{reason:'Run harmless regression markers in the disposable terminal.'});promise.catch(()=>{});
 await page.locator('[data-request-key*=":control:"] .primary').click();
 const response=await promise;assert.equal(response.type,'agent');
}
async function run(agent,marker) {
 assert.ok(!(await agent.tool('type',{text:"printf '%s\\n' "+marker})).error);
 const sent=await agent.tool('send_key',{key:'ENTER',intent:'Print a harmless regression marker.'});
 if(sent.status==='pending') {
  await page.locator('[data-request-key*=":review:"] .btn.ok').click();
  await until(async()=>['granted','executed'].includes((await agent.tool('check_approval',{approvalId:sent.approvalId})).state),'review completes');
 } else assert.equal(sent.status,'executed');
 await until(async()=>(await agent.tool('snapshot')).screen.some(row=>row.trim()===marker),'actual output '+marker);
}
async function capture(name) {await page.screenshot({path:join(evidence,name+'.png')});cases.push(name);console.log('PASS '+name);}
async function layout(count) {
 await until(async()=>await summary().count()===1 && (await summary().getAttribute('aria-label')).startsWith(`Show ${count} agent connections`),'summary count '+count);
 const metrics=await page.evaluate(()=>{
  const item=document.querySelector('.tab-item'), list=document.querySelector('.tablist'), close=item.querySelector('.x'), badge=item.querySelector('.agents');
  const box=element=>{const b=element.getBoundingClientRect();return {left:b.left,right:b.right,top:b.top,bottom:b.bottom};};
  return {item:box(item),list:box(list),close:box(close),badge:box(badge),client:list.clientWidth,scroll:list.scrollWidth};
 });
 assert.ok(metrics.scroll<=metrics.client+1,JSON.stringify(metrics));
 for(const control of [metrics.close,metrics.badge])assert.ok(control.left>=metrics.list.left && control.right<=metrics.list.right+1,JSON.stringify(metrics));
 assert.equal(await page.locator('.tab-item .x').isVisible(),true);assert.equal(await page.locator('.tabs .new').isVisible(),true);
}
try {
 const meta=await until(()=>{try{return JSON.parse(readFileSync(join(state,'connection.json'),'utf8'));}catch{return null;}},'web host starts');
 browser=await chromium.launch();page=await browser.newPage({viewport:{width:1120,height:720},reducedMotion:'reduce'});page.setDefaultTimeout(15000);
 page.on('pageerror',error=>errors.push(error.message));
 await page.goto(meta.url+'/#token='+encodeURIComponent(meta.bootstrapToken));
 await page.locator('.host:not([hidden])[data-terminal-ready="true"]').waitFor();await human("bind 'set editing-mode emacs'",['Enter']);
 const agent=await attach('same-agent');await control(agent);await run(agent,'REGRESSION_BASELINE');
 for(const [name,text,keys] of [['backspace','x',['Backspace']],['home-delete','x',['Home','Delete']],['left-empty','',['ArrowLeft']],['ctrl-l-empty','',['Control+l']],['ctrl-w','word',['Control+w']]]) {
  await human('',['Control+c']);await human(text,keys);await control(agent);await run(agent,'REGRESSION_'+name.replaceAll('-','_'));await capture('input-'+name);
 }
 await human('',['Control+c']);await human("printf '%s\\n' HUMAN_SUFFIX",['Home','Control+u']);await control(agent);
 const before=(await agent.tool('snapshot')).screen;
 assert.match((await agent.tool('type',{text:"printf '%s\\n' AGENT_PREFIX; "})).error,/input_pending/);
 assert.match((await agent.tool('send_key',{key:'ENTER',intent:'Check retained input protection.'})).error,/input_pending/);
 assert.deepEqual((await agent.tool('snapshot')).screen,before);await capture('retained-input-blocked');await human('',['Control+c']);
 for(let count=2;count<=5;count++){await attach('same-agent');await layout(count);}
 await capture('repeated-five-summary');
 for(let count=6;count<=7;count++)await attach('long-agent-name-repeated-for-overflow-abcdefghijklmnopqrst');
 for(const width of [720,1120,1280,1920]) {
  await page.setViewportSize({width,height:720});await layout(7);await summary().click();
  const popup=page.getByRole('dialog',{name:'Agent connections (7)',exact:true});await popup.waitFor();assert.equal(await popup.locator('li').count(),7);
  const names=await popup.locator('.agent-name').allTextContents();assert.equal(names.filter(name=>name==='same-agent').length,5);assert.equal(names.filter(name=>name.startsWith('long-agent-name')).length,2);
  assert.equal(new Set(await popup.locator('li small').allTextContents()).size,7,'same names retain distinct connection IDs');
  const bounds=await popup.boundingBox();assert.ok(bounds.x>=0 && bounds.x+bounds.width<=width && bounds.y+bounds.height<=720);
  assert.equal(await popup.evaluate(element=>Boolean(element.closest('.tablist'))),false,'details escape the clipping container');
  await capture('seven-connections-'+width);await page.keyboard.press('Escape');await popup.waitFor({state:'hidden'});
  assert.equal(await summary().evaluate(element=>element===document.activeElement),true,'Escape restores trigger focus');
 }
 await control(agents.at(-1));await layout(7);
 assert.equal(await summary().locator('.name').textContent(),agents.at(-1).name,'current controller becomes the representative');
 assert.ok((await summary().boundingBox()).width<=132,'long representative stays bounded');
 await summary().click();await page.getByRole('dialog',{name:'Agent connections (7)',exact:true}).waitFor();
 assert.ok((await page.locator('.connections .control').textContent()).includes(agents.at(-1).name),'full controller name remains accessible');
 await capture('long-controller-summary');await page.keyboard.press('Escape');await human('',['Control+c']);
 await summary().focus();await page.keyboard.press('Enter');await page.getByRole('dialog',{name:'Agent connections (7)',exact:true}).waitFor();
 await page.keyboard.press('Tab');assert.equal(await page.locator('.tab-item .x').evaluate(element=>element===document.activeElement),true,'Tab continues to close control');
 await summary().click();for(const other of agents.slice(1))other.close();await layout(1);
 await page.getByRole('dialog',{name:'Agent connections (1)',exact:true}).waitFor();assert.equal(await page.locator('.connections li').count(),1);
 await page.keyboard.press('Escape');await capture('after-disconnect');
 await page.locator('.app-menu .trigger').click();await page.getByRole('menuitem',{name:/^Settings/}).click();
 await page.locator('.category-list').getByRole('button',{name:'Appearance',exact:true}).click();await page.getByRole('button',{name:'한국어',exact:true}).click();
 await page.locator('.sheet header .btn.ghost').click();assert.match(await summary().getAttribute('aria-label'),/에이전트 연결 1개 보기/);
 await summary().click();await page.getByRole('dialog',{name:'에이전트 연결 (1개)',exact:true}).waitFor();await capture('connections-korean');await page.keyboard.press('Escape');
 await summary().click();agent.close();await summary().waitFor({state:'hidden'});await page.locator('.connections').waitFor({state:'hidden'});
 assert.equal(await page.getByRole('tab').evaluate(element=>element===document.activeElement),true,'last disconnect restores the tab focus');
 await page.locator('.tabs .new').click();await until(async()=>await page.getByRole('tab').count()===2,'new session stays usable');
 await page.locator('.tab-item .x').last().click();await until(async()=>await page.getByRole('tab').count()===1,'close control stays usable');
 assert.deepEqual(errors,[]);await capture('session-controls');
} catch(error) {process.exitCode=1;console.error(error);if(page)await page.screenshot({path:join(evidence,'failure.png')});}
finally {
 writeFileSync(join(evidence,'results.json'),JSON.stringify({cases,errors,passed:!process.exitCode},null,2));
 for(const agent of agents)agent.close();await browser?.close();server.kill();
}
