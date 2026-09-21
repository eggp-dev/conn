// Runs the production App against an isolated contract fixture. Requires agent-browser.
import { execFileSync, spawn } from 'node:child_process';
import { strict as assert } from 'node:assert';
import { resolve } from 'node:path';
import { mkdirSync } from 'node:fs';
const port=process.env.CONN_UI_TEST_PORT ?? '1435';
const base=`http://127.0.0.1:${port}/tests/collaboration/`;
const session=`conn-collaboration-${process.pid}`;
const output=resolve('../../artifacts/collaboration-refactor');mkdirSync(output,{recursive:true});
const run=(...args)=>execFileSync('agent-browser',['--session',session,...args],{encoding:'utf8',timeout:20000});
const value=source=>JSON.parse(run('eval',source));
let server;
let browserOpen=false;
function openFixture(query) {
  // Like per-test browser contexts: no storage, emulation or daemon lifetime leaks.
  if (browserOpen) run('close');
  run('open',base+query);browserOpen=true;
}
try {
  try { await fetch(base); } catch { server=spawn(process.execPath,['node_modules/vite/bin/vite.js','--host','127.0.0.1','--port',port],{stdio:'ignore'}); }
  for(let i=0;i<100;i++){try{if((await fetch(base)).ok)break;}catch{}if(i===99)throw Error('Vite did not start');await new Promise(r=>setTimeout(r,100));}
  openFixture('?fail-admission');run('wait','.decision-card');
  assert.equal(value('document.querySelectorAll(".decision-card").length'),1);
  run('click','.decision-card .primary');
  assert.match(value('document.querySelector("[role=alert]").textContent'),/try again/);
  assert.equal(value('document.querySelector(".decision-card .primary").disabled'),false);
  run('screenshot',resolve(output,'admission-retry.png'));
  run('click','.decision-card .primary');
  assert.equal(value('document.querySelectorAll(".decision-card").length'),0);
  console.log('PASS private admission visible, failure retained, retry resolves');
  openFixture('?fail-sharing');run('wait','.decision-card');run('click','.decision-card .primary');
  run('click','.sharing-trigger');run('click','.sharing-row button');
  run('wait','.sharing');run('click','.sharing input[type=checkbox]');run('click','.sharing footer .btn:not(.ghost)');
  const error=value('document.querySelector(".sharing [role=alert]").textContent');
  await new Promise(r=>setTimeout(r,2800));
  assert.equal(value('document.querySelector(".sharing [role=alert]").textContent'),error);
  assert.match(error,/input|typing|command/i);
  run('screenshot',resolve(output,'sharing-failure.png'));
  console.log('PASS sharing error survives participant polling');
  openFixture('?fail-stale');run('wait','.decision-card');run('click','.decision-card .primary');
  run('click','.sharing-trigger');run('click','.sharing-row button');
  run('wait','.sharing');run('click','.sharing input[type=checkbox]');run('click','.sharing footer .btn:not(.ghost)');
  assert.match(value('document.querySelector(".sharing [role=alert]").textContent'),/another window/);
  assert.equal(value('document.querySelector(".sharing footer .btn:not(.ghost)").disabled'),true);
  run('click','.sharing .body > .btn');
  assert.equal(value('document.querySelector(".sharing input[type=checkbox]").checked'),false);
  run('click','.sharing input[type=checkbox]');run('click','.sharing footer .btn:not(.ghost)');
  assert.equal(value('document.querySelectorAll(".sharing").length'),0);
  console.log('PASS stale sharing requires reloading the current selection');
  for (const state of ['empty', 'preparing', 'settings']) {
    openFixture('?'+state);run('wait','.decision-card');
    assert.equal(value('document.querySelector(".decision-card .primary").closest("[inert]") === null'),true);
    if(state === 'settings') assert.ok(value('document.querySelector(".sheet").textContent').includes('Agents'));
    run('click','.decision-card .primary');
    assert.equal(value('document.querySelectorAll(".decision-card").length'),0);
  }
  console.log('PASS admission in empty, preparing, and settings views');
  openFixture('?lang=ko&delayed');run('set','media','dark','reduced-motion');run('set','viewport','390','700');
  run('eval','window.dispatchEvent(new Event("fixture:request"))');run('wait','.decision-card');
  assert.equal(value('document.documentElement.scrollWidth <= innerWidth'),true);
  assert.equal(value('document.querySelector(".decision-card").getBoundingClientRect().right <= innerWidth'),true);
  run('screenshot',resolve(output,'admission-ko-narrow.png'));
  assert.equal(value('matchMedia("(prefers-reduced-motion: reduce)").matches'),true);
  assert.equal(value('document.querySelector(".decision-card").getAnimations().length'),0);
  run('eval','document.querySelector(".decision-card .primary").focus()');run('press','Enter');
  assert.equal(value('document.querySelectorAll(".decision-card").length'),0);
  console.log('PASS Korean narrow layout, reduced motion, and keyboard approval');
} finally {try{run('close');}catch{}server?.kill();}
