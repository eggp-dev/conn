// Production UI, Rust owner boundary, real PTYs and persistent agent sockets.
// Synthetic fixtures only. Evidence lives outside the checkout by default.
import {chromium} from 'playwright';
import {spawn} from 'node:child_process';
import {mkdtempSync,writeFileSync,existsSync,mkdirSync,openSync,readFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import net from 'node:net';
import {strict as assert} from 'node:assert';
const dir=mkdtempSync(join(tmpdir(),'conn-context-'));
const output=process.env.CONN_CONTEXT_OUTPUT ?? join(dir,'evidence');mkdirSync(output,{recursive:true});
const port=process.env.CONN_HARNESS_UI_PORT??'1461',backend=process.env.CONN_HARNESS_PORT??'1463';
const env={...process.env,CONN_TEST_PORT:backend,CONN_TEST_ORIGIN:`http://127.0.0.1:${port}`,CONN_TEST_STATE:dir};
writeFileSync(join(dir,'app.json'),JSON.stringify({mode:'autopilot',gate:true,pacing:{enterGraceMs:0,minWriteIntervalMs:0,leaseTtlSecs:120,approvalTtlSecs:120}}));
writeFileSync(join(dir,'profiles.json'),JSON.stringify({version:1,revision:0,defaultProfile:'fixture',profiles:[{id:'fixture',name:'Fixture',program:'/bin/bash',args:['--noprofile','--norc'],cwd:dir}]}));
const log=openSync(join(dir,'harness.log'),'w');
const server=spawn(resolve('../../target/debug/conn-browser-harness'),[dir],{env,stdio:['ignore',log,log]});
const delay=ms=>new Promise(r=>setTimeout(r,ms));
let vite,browser,context,page;const clients=[],errors=[],stages=[];
async function connect(name){
 const socket=net.createConnection(join(dir,'conn.sock'));let seq=0,buffer='';const pending=new Map();
 await new Promise((r,j)=>{socket.once('connect',r);socket.once('error',j)});
 socket.on('data',chunk=>{buffer+=chunk;let at;while((at=buffer.indexOf('\n'))>=0){const m=JSON.parse(buffer.slice(0,at));buffer=buffer.slice(at+1);const p=pending.get(m.id);if(p){clearTimeout(p.timer);pending.delete(m.id);m.error?p.reject(Object.assign(Error(m.error.message),m.error)):p.resolve(m.result)}}});
 const client={call:(method,params={})=>new Promise((resolve,reject)=>{const id=++seq,timer=setTimeout(()=>{pending.delete(id);reject(Error(`RPC timeout: ${method}`))},18000);pending.set(id,{resolve,reject,timer});socket.write(JSON.stringify({id,method,params})+'\n')}),close(){for(const p of pending.values())clearTimeout(p.timer);socket.destroy();}};
 clients.push(client);client.hello=await client.call('hello',{kind:'agent',agentId:name});return client;
}
const state=()=>page.evaluate(async()=>{const {st}=await import('/src/lib/store.svelte.ts');return {active:st.active,order:st.order,tabs:st.tabs,activity:st.activity,notice:st.announcement};});
async function capture(name,extra={}){await delay(500);stages.push({name,...await state(),...extra});writeFileSync(join(output,'states.json'),JSON.stringify(stages,null,2));await page.screenshot({path:join(output,`${name}.png`)});console.log(`PASS ${name}`);}
async function until(test,what){for(let i=0;i<100;i++){if(await test())return;await delay(100)}throw Error(`Timed out: ${what}`);}
async function allow(agent){await page.locator('.decision-card .primary').click();await until(async()=>(await state()).activity.connections.some(a=>a.connId===agent.hello.conn),'admission resolved');}
// fill() cannot deliver terminal input; keyboard insertion emits the real onData path.
async function typeHuman(text){const input=page.locator('.xterm-helper-textarea:visible');await input.click();await page.keyboard.insertText(text);await input.press('Enter');await delay(300);}
async function select(id){const s=await state();await page.getByRole('tab').nth(s.order.indexOf(id)).click();await delay(250);}
async function shareDialog(){await page.locator('button.dot:not(.hidden), .sharing-trigger').first().click();await page.locator('.sharing-row button').click();await page.locator('.sharing').waitFor();}
async function stopSharing(){await page.locator('.pill').waitFor({state:'detached'});await shareDialog();await page.locator('.sharing footer .ghost').click();await page.locator('.sharing').waitFor({state:'detached'});await until(async()=>{const s=await state();return !s.tabs[s.active].shared},'sharing stopped');}
async function shareSelected(){await page.locator('.sharing input[type=checkbox]').first().check();await page.locator('.sharing footer .btn:not(.ghost)').click();await page.locator('.sharing').waitFor({state:'detached'});}
async function grantBackground(agent,session){
 const pending=agent.call('request_control',{reason:'Print a synthetic fixture marker.'});pending.catch(()=>{});
 await until(async()=>!!(await state()).tabs[session]?.ctlReq,'background request');
 assert.notEqual((await state()).active,session);
 await page.locator('.pill').click();await until(async()=>(await state()).active===session,'notice navigation');
 await until(async()=>page.evaluate(()=>document.activeElement?.getAttribute('data-request-key')?.includes(':control:')),'focus exact request card');
 assert.equal((await state()).tabs[session].controller.type,'human','navigation never grants');
 await page.locator('.decision-card .primary').click();await pending;
}
async function marker(agent,text){
 await agent.call('type',{text:`printf '%s\\n' ${text}`});
 const result=await agent.call('send_key',{key:'ENTER',intent:'Print synthetic test output.'});assert.equal(result.status,'executed');
 await until(async()=>(await agent.call('snapshot')).screen.some(r=>r===text),'standalone output');
}
async function grid(agent,label){
 await delay(700);
 const snapshot=await agent.call('snapshot');
 const visible=await page.locator('.xterm-rows:visible > div').allTextContents();
 const rows=visible.map(row=>row.replace(/\u00a0/g,' ').trimEnd());
 const result={label,size:snapshot.size,core:snapshot.screen,visible:rows,equal:JSON.stringify(snapshot.screen)===JSON.stringify(rows)};
 writeFileSync(join(output,`grid-${label}.json`),JSON.stringify(result,null,2));return result;
}
try{
 for(let i=0;!existsSync(join(dir,'connection.json'));i++){if(i>100||server.exitCode!==null)throw Error(`Rust startup failed: ${readFileSync(join(dir,'harness.log'),'utf8')}`);await delay(100)}
 vite=spawn(process.execPath,['node_modules/vite/bin/vite.js','--mode','browser-test','--host','127.0.0.1','--port',port,'--strictPort'],{env,stdio:['ignore',log,log]});
 await until(async()=>{try{return (await fetch(`http://127.0.0.1:${port}/`)).ok}catch{return false}},'Vite');
 browser=await chromium.launch();context=await browser.newContext({viewport:{width:1440,height:960},recordVideo:{dir:output,size:{width:1440,height:960}}});
 await context.addInitScript(()=>localStorage.setItem('ss:lang','ko'));page=await context.newPage();page.setDefaultTimeout(12000);page.on('pageerror',e=>errors.push(e.message));
 await page.goto(`http://127.0.0.1:${port}/`);await page.locator('.xterm-helper-textarea').waitFor();await delay(500);
 await typeHuman('export CONN_CONTEXT_MARKER=retained_shell_42');const first=(await state()).active;
 const agent=await connect('context-agent');assert.equal(agent.hello.admission,'pending');await allow(agent);
 const second=(await agent.call('open_tab',{reason:'Synthetic second terminal.'})).session;
 await until(async()=>(await state()).order.includes(second),'new tab');assert.equal((await state()).active,first);
 await capture('01-new-tab-notice');assert.equal((await state()).notice.target.session,second);
 await page.locator('.pill').waitFor({state:'detached'});await grantBackground(agent,second);await capture('02-notice-target-and-explicit-grant');
 await select(first);await marker(agent,'BACKGROUND_TWO');await page.locator('.collaboration-activity button').waitFor();
 await typeHuman("printf '%s\\n' HUMAN_FIRST");assert.equal((await agent.call('status')).controller.type,'agent');await capture('03-visible-background-activity');
 await agent.call('snapshot',{session:first});await until(async()=>(await state()).activity.connections.some(a=>a.session===first),'latest read target');
 assert.equal(await page.locator('.collaboration-activity button').count(),1,'control elsewhere remains visible after reading this tab');
 await page.locator('.collaboration-activity button').first().click();await until(async()=>(await state()).active===second,'activity navigation');await typeHuman("printf '%s\\n' HUMAN_SECOND");assert.equal((await agent.call('status')).controller.type,'human');
 await select(first);const third=(await agent.call('open_tab')).session;await until(async()=>(await state()).order.includes(third),'third tab');
 await page.locator('.pill').waitFor({state:'detached'});await agent.call('switch_tab',{tab:second});
 await until(async()=>(await state()).notice?.target?.session===second,'background movement notice');assert.equal((await state()).active,first);await capture('04-background-to-background-move');
 // Explicitly addressed operations update activity without polling reverting it.
 await agent.call('snapshot',{session:third});await agent.call('list_tabs');
 await until(async()=>(await state()).activity.connections.some(a=>a.session===third),'explicit session activity');
 // Clear an old request while its notice remains; clicking may navigate but never grants.
 await agent.call('switch_tab',{tab:second});const cancelled=agent.call('request_control',{reason:'Cancel this request by disconnecting.'});cancelled.catch(()=>{});
 await until(async()=>!!(await state()).tabs[second].ctlReq,'request before disconnect');agent.close();
 await until(async()=>!(await state()).tabs[second].ctlReq,'disconnect clears request');
 if(await page.locator('.pill').count())await page.locator('.pill').click();assert.equal((await state()).tabs[second].controller.type,'human');
 // Same name reconnect is a fresh admission; select and inspect the existing shell.
 const again=await connect('context-agent');assert.equal(again.hello.admission,'pending');await allow(again);
 assert.notEqual(again.hello.conn,agent.hello.conn);await again.call('switch_tab',{tab:first});await select(first);
 await typeHuman('printf "%s\\n" "$CONN_CONTEXT_MARKER"');
 await until(async()=>(await again.call('snapshot')).screen.includes('retained_shell_42'),'same PTY after reconnect');await capture('05-reconnect-retains-shell');
 // Long paths, URLs, Korean and line wrapping are compared with actual DOM rows.
 const path='/synthetic/'+('deep-path/'.repeat(24))+'끝';
 const url='https://example.invalid/?synthetic='+('abcdef0123456789'.repeat(16));
 await typeHuman(`printf '\\033[2J\\033[H%s\\n%s\\n%s\\n' '${path}' '${url}' '한글과 English: 가나라마바사 아자차카타파하'`);
 const before=await grid(again,'before');assert.ok(before.equal,'initial current grid matches rendered rows');
 await page.setViewportSize({width:860,height:720});const narrow=await grid(again,'narrow');
 await page.setViewportSize({width:1440,height:960});const restored=await grid(again,'restored');
 if(process.env.CONN_GRID_INVESTIGATE!=='1'){assert.ok(narrow.equal,'narrow current grid matches rendered rows');assert.ok(restored.equal,'restored current grid matches rendered rows')}
 await capture('06-output-resize',{grid:{before:before.equal,narrow:narrow.equal,restored:restored.equal}});
 await page.locator('.pill').waitFor({state:'detached'});await page.locator('button.dot:not(.hidden)').click();
 await page.locator('.center .seg button').filter({hasText:'Observe'}).click();
 await until(async()=>(await again.call('status')).mode==='observe','Observe mode');await page.locator('.center').press('Escape');
 await again.call('snapshot');await assert.rejects(()=>again.call('request_control'),e=>e.code==='wrong_mode');
 await again.call('switch_tab',{tab:third});await again.call('switch_tab',{tab:first});await select(third);await again.call('request_attention',{reason:'Read-only inspection completed.'});
 await until(async()=>(await state()).notice?.target?.session===first,'Observe attention');await page.locator('.pill').click();await until(async()=>(await state()).active===first,'Observe request target');await capture('06b-observe-navigation');
 // Remove all sharing. Preparation stays connection-scoped and reveals no shell.
 for(const id of [first,second,third]){await select(id);await stopSharing();}
 assert.equal((await again.call('list_tabs')).tabs,0);
 const request=await again.call('request_attention',{reason:'Choose a terminal for the synthetic fixture.'});assert.equal(request.scope,'connection');
 assert.equal((await again.call('request_attention',{reason:'duplicate'})).requestId,request.requestId);
 await page.locator('.terminal-preparation').waitFor();await capture('07-sessionless-request');
 await page.setViewportSize({width:780,height:640});await page.emulateMedia({reducedMotion:'reduce'});
 await page.evaluate(async()=>{const {setLang}=await import('/src/lib/i18n.svelte.ts');setLang('en')});await capture('07b-preparation-english-narrow');
 assert.equal(await page.locator('.terminal-preparation .primary').innerText(),'Choose terminal');
 const card=await page.locator('.terminal-preparation').boundingBox();assert.ok(card.x>=0 && card.x+card.width<=780 && card.y+card.height<=640);
 await page.evaluate(async()=>{const {setLang}=await import('/src/lib/i18n.svelte.ts');setLang('ko')});await page.setViewportSize({width:1440,height:960});
 await page.locator('.terminal-preparation .primary').click();await page.locator('.sharing').waitFor();assert.equal((await again.call('list_tabs')).tabs,0);
 await page.locator('.sharing header button').click();await page.locator('.sharing').waitFor({state:'detached'});
 await page.locator('.terminal-preparation .btn:not(.ghost):not(.primary)').click();await page.locator('.sharing').waitFor();
 await until(async()=>![first,second,third].includes((await state()).active),'new private tab selected');const prepared=(await state()).active;assert.equal((await state()).tabs[prepared].shared,false);assert.equal((await again.call('list_tabs')).tabs,0);await capture('08-new-private-terminal');
 await shareSelected();await until(async()=>!(await page.locator('.terminal-preparation').count()),'resolved preparation');
 await again.call('switch_tab',{tab:prepared});assert.equal((await again.call('status')).controller.type,'human');await capture('09-explicit-sharing-without-control');
 await stopSharing();again.close();await until(async()=>!(await state()).activity.connections.length,'closed connection activity');
 const fresh=await connect('context-agent');await allow(fresh);assert.equal((await fresh.call('list_tabs')).tabs,0,'same name inherits no selected sharing');
 await fresh.call('request_attention');await page.locator('.terminal-preparation').waitFor();await page.locator('.terminal-preparation .ghost').click();await until(async()=>!(await page.locator('.terminal-preparation').count()),'later dismisses');
 assert.equal((await fresh.call('request_attention')).requested,false,'dismiss cooldown');await fresh.call('request_attention',{cancel:true});
 await capture('10-no-inherited-sharing');assert.deepEqual(errors,[]);
 writeFileSync(join(output,'result.json'),JSON.stringify({status:'passed',stages:stages.length,errors},null,2));console.log(`Evidence: ${output}`);
}catch(error){if(page){await page.screenshot({path:join(output,'failure.png')}).catch(()=>{});console.error(await page.locator('body').innerText().catch(()=>''))}console.error(`Evidence: ${output}`);throw error;
}finally{for(const client of clients)client.close();await context?.close();if(page)await page.video()?.saveAs(join(output,'context.webm'));await browser?.close();vite?.kill();server.kill();}
