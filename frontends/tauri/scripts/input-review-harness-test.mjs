// Production UI, Rust owner boundary, real PTYs and persistent agent sockets.
// Synthetic fixtures only. Evidence lives outside the checkout by default.
import {chromium} from 'playwright';
import {spawn} from 'node:child_process';
import {mkdtempSync,writeFileSync,existsSync,mkdirSync,openSync,readFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import net from 'node:net';
import {strict as assert} from 'node:assert';
const dir=mkdtempSync(join(tmpdir(),'conn-input-review-'));
const output=process.env.CONN_INPUT_REVIEW_OUTPUT ?? join(dir,'evidence');mkdirSync(output,{recursive:true});
const port=process.env.CONN_HARNESS_UI_PORT??'1471',backend=process.env.CONN_HARNESS_PORT??'1473';
const env={...process.env,CONN_TEST_PORT:backend,CONN_TEST_ORIGIN:`http://127.0.0.1:${port}`,CONN_TEST_STATE:dir};
writeFileSync(join(dir,'app.json'),JSON.stringify({mode:'autopilot',gate:true,pacing:{enterGraceMs:0,minWriteIntervalMs:0,leaseTtlSecs:120,approvalTtlSecs:120}}));
writeFileSync(join(dir,'profiles.json'),JSON.stringify({version:1,revision:0,defaultProfile:'fixture',profiles:[{id:'fixture',name:'Fixture',program:process.env.CONN_REPRO_SHELL??'/bin/bash',args:process.env.CONN_REPRO_SHELL?['-i']:['--noprofile','--norc'],env:{PS1:'fixture$ ',TERM:'xterm-256color'},cwd:dir}]}));
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
async function grid(agent,label){
 await delay(700);
 const snapshot=await agent.call('snapshot');
 const visible=await page.locator('.xterm-rows:visible > div').allTextContents();
 const rows=visible.map(row=>row.replace(/\u00a0/g,' ').trimEnd());
 const cursor=await page.evaluate(cols=>{
  const cell=document.querySelector('.host:not([hidden]) .xterm-cursor');
  const screen=document.querySelector('.host:not([hidden]) .xterm-screen');
  const row=cell?.closest('.xterm-rows > div');
  if(!cell||!screen||!row)return null;
  return {row:Array.from(row.parentElement.children).indexOf(row),col:Math.round((cell.getBoundingClientRect().left-screen.getBoundingClientRect().left)/(screen.clientWidth/cols))};
 },snapshot.size.cols);
 const result={label,size:snapshot.size,core:snapshot.screen,visible:rows,cursor:snapshot.cursor,visibleCursor:cursor,equal:JSON.stringify(snapshot.screen)===JSON.stringify(rows)};
 assert.deepEqual(cursor,snapshot.cursor,`${label}: user and agent cursor positions agree`);
 writeFileSync(join(output,`grid-${label}.json`),JSON.stringify(result,null,2));return result;
}
try{
 for(let i=0;!existsSync(join(dir,'connection.json'));i++){if(i>100||server.exitCode!==null)throw Error('Rust startup failed');await delay(100)}
 vite=spawn(process.execPath,['node_modules/vite/bin/vite.js','--mode','browser-test','--host','127.0.0.1','--port',port,'--strictPort'],{env,stdio:['ignore',log,log]});
 await until(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/`)).ok}catch{return false}},'Vite');
 browser=await chromium.launch();context=await browser.newContext({viewport:{width:860,height:640}});
 await context.addInitScript(()=>localStorage.setItem('ss:lang','ko'));page=await context.newPage();page.setDefaultTimeout(12000);
 page.on('pageerror',e=>errors.push(e.message));await page.goto(`http://127.0.0.1:${port}/`);await page.locator('.xterm-helper-textarea').waitFor();await delay(700);
 if(process.env.CONN_AUTH_FIXTURE_RUNTIME){
  const runtime=process.env.CONN_AUTH_FIXTURE_RUNTIME;
  const password=readFileSync(join(runtime,'password'),'utf8').trim();assert.ok(password.startsWith('SYNTHETIC_'));
  await typeHuman(`ssh -F /dev/null -tt -p 22222 -o StrictHostKeyChecking=yes -o UserKnownHostsFile=${runtime}/known_hosts -o PreferredAuthentications=password -o PubkeyAuthentication=no -o IdentityAgent=none fixture@127.0.0.1`);
  await until(async()=>(await page.locator('.xterm-rows').innerText()).includes('password:'),'SSH password prompt');
  await typeHuman(password);
  await typeHuman("exec env LC_ALL=C.UTF-8 PS1='remote$ ' bash --noprofile --norc -i");
  await typeHuman('printf "REMOTE_READY_%s\\n" "$((6*7))"');
  await until(async()=>(await page.locator('.xterm-rows').innerText()).includes('REMOTE_READY_42'),'remote marker');
 }
 const agent=await connect('input-repro');await allow(agent);
 const control=agent.call('request_control',{reason:'Reproduce long command rendering.'});control.catch(()=>{});
 await page.locator('.decision-card .primary').click();await control;await delay(500);
 for(const length of [240,700,1500,3500,7000]){
  const text='abcdefghijklmnopqrst'.repeat(Math.ceil(length/20)).slice(0,length);
  await agent.call('type',{text:`printf '%s\\n' '${text}'`});

  const execution=await agent.call('send_key',{key:'ENTER',intent:'Print synthetic fixture output.'});
  assert.equal(execution.status,'executed','ordinary SSH command executes in Autopilot');
  await delay(500);assert.ok((await grid(agent,`printed-${length}`)).equal);await capture(`output-${length}`);
 }
 // The modal settings sheet must leave the underlying PTY dimensions alone.
 for(const text of ['long-input/'.repeat(100), '한글/경로/🙂/'.repeat(65)]){
  await agent.call('type',{text:`printf '%s\\n' '${text}'`});
  const originalSize=(await agent.call('snapshot')).size;
  await capture('before-settings-'+(text.startsWith('한')?'unicode':'ascii'));
  for(let i=0;i<3;i++){
   await page.keyboard.press('Control+Shift+,');await delay(350);
   assert.equal(await page.evaluate(async()=>(await import('/src/lib/store.svelte.ts')).st.settingsOpen),true);
   assert.deepEqual((await agent.call('snapshot')).size,originalSize,'settings must not resize the PTY');
   await capture(`settings-${i}-${text.startsWith('한')?'unicode':'ascii'}`);
   await page.keyboard.press('Escape');await delay(350);
  }
  assert.ok((await grid(agent,'edited-long-line')).equal);
  await capture('after-settings-'+(text.startsWith('한')?'unicode':'ascii'));
  const result=await agent.call('send_key',{key:'ENTER',intent:'Print the line after a panel resize.'});
  assert.equal(result.status,'executed');await delay(500);
 }
 const missing=join(dir,'missing-'+('deep/'.repeat(1000))+'file');
 await agent.call('type',{text:`rm -- '${missing}'`});const approval=await agent.call('send_key',{key:'ENTER',intent:'Remove an absent synthetic fixture path.'});assert.equal(approval.status,'pending');
 await page.locator('.decision-card .btn.ok').waitFor();await capture('long-approval');await page.setViewportSize({width:700,height:540});await delay(600);await capture('approval-narrow');
 const box=await page.getByRole('button',{name:'A 세션 허용',exact:true}).boundingBox();
 assert.ok(box && box.y>=0 && box.y+box.height<=540,'approval actions remain visible for a long command');
 await page.setViewportSize({width:860,height:640});await delay(600);
 assert.equal((await state()).tabs[(await state()).active].reviewRequired,false);
 const allowSession=page.getByRole('button',{name:'A 세션 허용',exact:true});assert.ok(await allowSession.isEnabled());
 await allowSession.click();
 await until(async()=>(await agent.call('check_approval',{approvalId:approval.approvalId})).state==='granted','approval granted');
 await delay(700);assert.ok((await grid(agent,'approved')).equal);await capture('after-allow');
 await until(async()=>(await state()).tabs[(await state()).active].allows.includes('delete files'),'session allowance shown');
 await agent.call('type',{text:`rm -- '${join(dir,'never-created')}'`});
 assert.equal((await agent.call('send_key',{key:'ENTER',intent:'Verify the same session rule on an absent fixture path.'})).status,'executed');
 await delay(500);
 await agent.call('type',{text:'sudo id'});
 assert.equal((await agent.call('send_key',{key:'ENTER',intent:'Verify another risk still asks, without running it.'})).status,'pending');
 await page.locator('.decision-card .btn.danger').click();await delay(500);
 const blocked=await page.evaluate(async()=>{
  const {cmd}=await import('/src/lib/bridge');return cmd('policy_test',{cmd:'rm -rf /'});
 });
 assert.equal(blocked.policy,'deny','session allowance never lifts deny rules');
 await delay(500);
 await agent.call('type',{text:"printf '%s\\n' FINAL_MARKER"});const last=await agent.call('send_key',{key:'ENTER',intent:'Print final marker.'});assert.equal(last.status,'executed');
 await delay(500);assert.ok((await grid(agent,'final')).equal);await capture('final');
 // Human takeover, cursor editing and execution use the real terminal input path.
 const long='cursor-edit/'.repeat(85);
 await agent.call('type',{text:`printf '<%s>\\n' '${long}END'`});
 await delay(500);
 const before=await grid(agent,'cursor-before-edit');assert.ok(before.equal);
 await page.locator('.xterm-helper-textarea:visible').focus();
 for(let i=0;i<4;i++)await page.keyboard.press('ArrowLeft');
 await delay(300);
 const moved=await grid(agent,'cursor-moved');assert.ok(moved.equal);
 // Human takeover opens the handback dock and can reduce the row count.
 const row=Math.min(before.cursor.row,moved.size.rows-1);
 const start=row*before.size.cols+before.cursor.col-4;
 assert.deepEqual(moved.cursor,{row:Math.floor(start/before.size.cols),col:start%before.size.cols});
 await page.keyboard.insertText('EDIT');await page.keyboard.press('End');await page.keyboard.press('Enter');
 await until(async()=>(await agent.call('snapshot')).screen.join('').includes('EDITEND>'),'edited command output');
 assert.equal((await state()).tabs[(await state()).active].controller.type,'human');
 assert.ok((await grid(agent,'human-edited')).equal);await capture('human-edited');
 if(process.env.CONN_AUTH_FIXTURE_RUNTIME){
  await typeHuman('exit');
  await until(async()=>page.evaluate(async()=>{const {cmd}=await import('/src/lib/bridge');return (await cmd('status')).completionPromptReady;}),'outer shell lifecycle restored');
  await typeHuman("printf 'LOCAL_BACK_%s\\n' 42");
  await until(async()=>(await agent.call('snapshot')).screen.some(row=>row==='LOCAL_BACK_42'),'outer shell restored');
 }
 assert.deepEqual(errors,[]);
 console.log('Evidence:',output);
}finally{for(const client of clients)client.close();await context?.close();await browser?.close();vite?.kill();server.kill();}
