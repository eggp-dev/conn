// Real Rust Harness + production UI + a persistent socket agent. No alternate UI.
import {execFileSync,spawn} from 'node:child_process';
import {mkdtempSync,writeFileSync,existsSync,mkdirSync,openSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import net from 'node:net';
import {strict as assert} from 'node:assert';
const dir=mkdtempSync(join(tmpdir(),'conn-collaboration-'));
const session=`conn-harness-${process.pid}`, port=process.env.CONN_HARNESS_UI_PORT??'1437', backend=process.env.CONN_HARNESS_PORT??'1438';
const output=resolve('../../artifacts/collaboration-refactor');mkdirSync(output,{recursive:true});
const env={...process.env,CONN_TEST_PORT:backend,CONN_TEST_ORIGIN:`http://127.0.0.1:${port}`,CONN_TEST_STATE:dir};delete env.npm_config_prefix;
writeFileSync(join(dir,'app.json'),JSON.stringify({mode:'autopilot',gate:true,pacing:{enterGraceMs:0,minWriteIntervalMs:0,leaseTtlSecs:120,approvalTtlSecs:120}}));
const log=openSync(join(dir,'harness.log'),'w');
const server=spawn(resolve('../../target/debug/conn-browser-harness'),[dir],{env,stdio:['ignore',log,log]});
let vite,socket;const pending=new Map();let seq=0,buffer='';
const run=(...args)=>execFileSync('agent-browser',['--session',session,...args],{encoding:'utf8',timeout:20000});
const value=s=>JSON.parse(run('eval',s));
const delay=ms=>new Promise(r=>setTimeout(r,ms));
const call=(method,params={})=>new Promise((resolve,reject)=>{const id=++seq;const timer=setTimeout(()=>{pending.delete(id);reject(Error(`RPC timeout: ${method}`));},18000);pending.set(id,{resolve,reject,timer});socket.write(JSON.stringify({id,method,params})+'\n');});
try {
  for(let i=0;!existsSync(join(dir,'connection.json'));i++){if(i>100||server.exitCode!==null)throw Error('Harness startup failed');await delay(100);}
  vite=spawn(process.execPath,['node_modules/vite/bin/vite.js','--mode','browser-test','--host','127.0.0.1','--port',port],{env,stdio:'ignore'});
  for(let i=0;;i++){try{if((await fetch(`http://127.0.0.1:${port}/`)).ok)break;}catch{}if(i>100)throw Error('Vite startup failed');await delay(100);}
  run('open',`http://127.0.0.1:${port}/`);run('wait','.xterm-helper-textarea');
  run('click','.xterm-helper-textarea');run('type','.xterm-helper-textarea','export CONN_COLLAB_PROCESS=retained_42');run('press','Enter');await delay(350);
  run('wait','button.dot:not(.hidden)');run('click','button.dot:not(.hidden)');run('wait','.center');run('click','.sharing-row button');run('click','.sharing footer .ghost');run('wait','.sharing-trigger');
  socket=net.createConnection(join(dir,'conn.sock'));await new Promise((r,j)=>{socket.once('connect',r);socket.once('error',j);});
  socket.on('data',chunk=>{buffer+=chunk;let at;while((at=buffer.indexOf('\n'))>=0){const message=JSON.parse(buffer.slice(0,at));buffer=buffer.slice(at+1);const p=pending.get(message.id);if(p){clearTimeout(p.timer);pending.delete(message.id);message.error?p.reject(Object.assign(Error(message.error.message),message.error)):p.resolve(message.result);}}});
  const hello=await call('hello',{kind:'agent',agentId:'collaboration-test'});assert.equal(hello.admission,'pending');
  run('wait','.decision-card');run('screenshot',resolve(output,'harness-private-admission.png'));run('click','.decision-card .primary');await delay(300);run('screenshot',resolve(output,'harness-after-allow.png'));
  const hidden=await call('list_tabs');assert.equal(hidden.tabs,0);
  run('click','.sharing-trigger');run('click','.sharing-row button');run('click','.sharing input[type=checkbox]');run('click','.sharing footer .btn:not(.ghost)');
  await delay(200);assert.equal((await call('list_tabs')).tabs,1);
  const control=call('request_control',{reason:'Verify the existing shell survives sharing.'});
  // Give the socket event time to enter the browser event queue before blocking on CLI.
  await delay(250);run('wait','.decision-card');run('click','.decision-card .primary');await control;
  await call('type',{text:'printf "%s\\n" "$CONN_COLLAB_PROCESS"'});
  const execution=await call('send_key',{key:'ENTER',intent:'Read the marker already set in this shell.'});
  if(execution.status === 'pending') { run('wait','.decision-card .btn.ok');run('click','.decision-card .btn.ok'); }
  else assert.equal(execution.status,'executed');
  let snapshot;
  for(let i=0;i<20;i++){await delay(100);snapshot=await call('snapshot');if(snapshot.screen.some(row=>row.trim()==='retained_42'))break;}
  assert.ok(snapshot.screen.some(row=>row.trim()==='retained_42'), "standalone command output proves process state survived");
  await call('release_control');
  run('screenshot',resolve(output,'harness-shared-execution.png'));
  run('wait','button.dot:not(.hidden)');run('click','button.dot:not(.hidden)');run('wait','.center');run('click','.sharing-row button');run('click','.sharing footer .ghost');
  await delay(150);assert.equal((await call('list_tabs')).tabs,0);
  console.log('PASS real Harness: private admission, selected sharing, owner control grant, execution, same PTY marker, stop sharing');
  console.log(`Isolated evidence: ${dir}`);
} finally {
  for(const p of pending.values())clearTimeout(p.timer);socket?.destroy();
  try{run('close');}catch{}vite?.kill();server.kill();
}
