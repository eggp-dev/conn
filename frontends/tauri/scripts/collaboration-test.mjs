import { collaborationBrowser } from './collaboration-browser.mjs';
// Runs the production App against an isolated contract fixture. Uses an isolated Playwright browser.
import { spawn } from 'node:child_process';
import { strict as assert } from 'node:assert';
import { resolve } from 'node:path';
import { mkdirSync } from 'node:fs';
const port=process.env.CONN_UI_TEST_PORT ?? '1435';
const base=`http://127.0.0.1:${port}/tests/collaboration/`;
const output=resolve('../../artifacts/collaboration-refactor');mkdirSync(output,{recursive:true});
const {run, value, open: openBrowser} = await collaborationBrowser();
let server;
async function openFixture(query) { await openBrowser(base+query); }
try {
  try { await fetch(base); } catch { server=spawn(process.execPath,['node_modules/vite/bin/vite.js','--host','127.0.0.1','--port',port],{stdio:'ignore'}); }
  for(let i=0;i<100;i++){try{if((await fetch(base)).ok)break;}catch{}if(i===99)throw Error('Vite did not start');await new Promise(r=>setTimeout(r,100));}
  await openFixture('?fail-admission');await run('wait','.decision-card');
  assert.equal(await value('document.querySelectorAll(".decision-card").length'),1);
  await run('click','.decision-card .primary');
  assert.match(await value('document.querySelector("[role=alert]").textContent'),/try again/);
  assert.equal(await value('document.querySelector(".decision-card .primary").disabled'),false);
  await run('screenshot',resolve(output,'admission-retry.png'));
  await run('click','.decision-card .primary');
  await run('gone','.decision-card');
  assert.equal(await value('document.querySelectorAll(".decision-card").length'),0);
  console.log('PASS private admission visible, failure retained, retry resolves');
  await openFixture('?fail-sharing');await run('wait','.decision-card');await run('click','.decision-card .primary');
  await run('click','.sharing-trigger');await run('click','.sharing-row button');
  await run('wait','.sharing');await run('click','.sharing input[type=checkbox]');await run('click','.sharing footer .btn:not(.ghost)');
  const error=await value('document.querySelector(".sharing [role=alert]").textContent');
  await new Promise(r=>setTimeout(r,2800));
  assert.equal(await value('document.querySelector(".sharing [role=alert]").textContent'),error);
  assert.match(error,/input|typing|command/i);
  await run('screenshot',resolve(output,'sharing-failure.png'));
  console.log('PASS sharing error survives participant polling');
  await openFixture('?fail-stale');await run('wait','.decision-card');await run('click','.decision-card .primary');
  await run('click','.sharing-trigger');await run('click','.sharing-row button');
  await run('wait','.sharing');await run('click','.sharing input[type=checkbox]');await run('click','.sharing footer .btn:not(.ghost)');
  assert.match(await value('document.querySelector(".sharing [role=alert]").textContent'),/another window/);
  assert.equal(await value('document.querySelector(".sharing footer .btn:not(.ghost)").disabled'),true);
  await run('click','.sharing .body > .btn');
  assert.equal(await value('document.querySelector(".sharing input[type=checkbox]").checked'),false);
  await run('click','.sharing input[type=checkbox]');await run('click','.sharing footer .btn:not(.ghost)');
  await run('gone','.sharing');
  assert.equal(await value('document.querySelectorAll(".sharing").length'),0);
  console.log('PASS stale sharing requires reloading the current selection');
  for (const state of ['empty', 'preparing', 'settings']) {
    await openFixture('?'+state);await run('wait','.decision-card');
    assert.equal(await value('document.querySelector(".decision-card .primary").closest("[inert]") === null'),true);
    if(state === 'settings') assert.ok((await value('document.querySelector(".sheet").textContent')).includes('Agents'));
    await run('click','.decision-card .primary');
    await run('gone','.decision-card');
  assert.equal(await value('document.querySelectorAll(".decision-card").length'),0);
  }
  console.log('PASS admission in empty, preparing, and settings views');
  await openFixture('?lang=ko&delayed');await run('set','media','dark','reduced-motion');await run('set','viewport','390','700');
  await run('eval','window.dispatchEvent(new Event("fixture:request"))');await run('wait','.decision-card');
  assert.equal(await value('document.documentElement.scrollWidth <= innerWidth'),true);
  assert.equal(await value('document.querySelector(".decision-card").getBoundingClientRect().right <= innerWidth'),true);
  await run('screenshot',resolve(output,'admission-ko-narrow.png'));
  assert.equal(await value('matchMedia("(prefers-reduced-motion: reduce)").matches'),true);
  assert.equal(await value('document.querySelector(".decision-card").getAnimations().length'),0);
  await run('eval','document.querySelector(".decision-card .primary").focus()');await run('press','Enter');
  await run('gone','.decision-card');
  assert.equal(await value('document.querySelectorAll(".decision-card").length'),0);
  console.log('PASS Korean narrow layout, reduced motion, and keyboard approval');
} finally {try{await run('close');}catch{}server?.kill();}
