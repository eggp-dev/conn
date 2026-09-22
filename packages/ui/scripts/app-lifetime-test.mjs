// Two real ConnApp mounts with isolated host ports. No live user runtime is touched.
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { chromium } from 'playwright';
import { testWebBootstrap } from '../tests/app-lifetime/web-flow.mjs';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const output=process.env.CONN_APP_LIFETIME_OUTPUT??mkdtempSync(join(tmpdir(),'conn-app-lifetime-'));
const server=await createServer({configFile:false,root,plugins:[svelte()],resolve:{alias:[{find:/^@tauri-apps\/api\/(core|event|window|webviewWindow)$/,replacement:join(root,'tests/app-lifetime/native-host.ts')}]},server:{host:'127.0.0.1',port:Number(process.env.CONN_APP_LIFETIME_PORT??1491),strictPort:true},logLevel:'error'});
let browser, page;
try {
 await server.listen();
 browser=await chromium.launch();
 page=await browser.newPage({viewport:{width:1200,height:850},reducedMotion:'reduce'});
 const errors=[];page.on('pageerror',error=>errors.push(error.message));
 await page.goto(`${server.resolvedUrls.local[0]}tests/app-lifetime/`);
 await page.addStyleTag({content:'html,body{overflow:auto!important}#one,#two{position:relative;display:block}'});
 const one=page.locator('#one'),two=page.locator('#two');
 await one.locator('.app[data-runtime-online="true"]').waitFor();
 await two.locator('.app[data-runtime-online="true"]').waitFor();
 const first=one.locator('.decision-card .primary'),second=two.locator('.decision-card .primary');
 await first.waitFor();await second.waitFor();
 assert.equal(await first.innerText(),'Allow shared screen access');assert.equal(await second.innerText(),'Allow shared screen access');
 await first.click();
 await one.locator('.decision-card[aria-busy="true"]').waitFor();
 assert.equal(await first.isDisabled(),true);assert.equal(await second.isEnabled(),true,'same request ID in another mount stays actionable');
 await second.click();await two.locator('.decision-card [role="alert"]').waitFor();
 assert.equal(await one.locator('.decision-card [role="alert"]').count(),0,'request errors belong to their own app');
 assert.equal(await first.isDisabled(),true);assert.equal(await second.isEnabled(),true);
 await second.click();await second.waitFor({state:'detached'});
 assert.equal(await first.isDisabled(),true,'other app resolution cannot finish a pending decision');

 await one.locator('.app-menu .trigger').click();
 await one.getByRole('menuitem',{name:/^Settings/}).click();
 await one.locator('.category-list').getByRole('button',{name:'Appearance',exact:true}).click();
 await one.getByRole('button',{name:'한국어',exact:true}).click();
 assert.equal(await one.locator('.decision-card .primary').innerText(),'공유 화면 접근 허용');
 assert.equal(await two.locator('.sheet').count(),0,'opening settings affects only the target root');
 assert.equal(await page.evaluate(()=>window.appLifetime.inspect('one').saved['ss:lang']),'ko');
 assert.equal(await page.evaluate(()=>window.appLifetime.inspect('two').saved['ss:lang']),'en');
 await one.locator('.sheet header .btn.ghost').click();
 await two.locator('.app-menu .trigger').click();
 await two.getByRole('menuitem',{name:/^Check for Updates/}).click();
 await two.getByRole('button',{name:'View releases',exact:true}).click();
 assert.equal((await page.evaluate(()=>window.appLifetime.inspect('two').links)).length,1,'release link goes to its own host port');
 assert.equal((await page.evaluate(()=>window.appLifetime.inspect('one').links)).length,0);
 assert.equal((await page.evaluate(()=>window.appLifetime.inspect('two').calls)).includes('open_release'),false,'web host link never invokes server-side opening');
 await two.locator('dialog header button').click();
 await page.screenshot({path:join(output,'isolated-apps.png'),fullPage:true});

 await page.evaluate(()=>window.appLifetime.unmount('one'));
 await one.locator('.app').waitFor({state:'detached'});
 const before=await page.evaluate(()=>({old:window.appLifetime.retired()[0],live:window.appLifetime.inspect('two')}));
 assert.equal(before.old.disposed,true);assert.equal(before.old.listeners,0);
 await page.waitForTimeout(5600);
 const after=await page.evaluate(()=>({old:window.appLifetime.retired()[0],live:window.appLifetime.inspect('two')}));
 assert.deepEqual(after.old.calls,before.old.calls,'unmounted app stops all polling');
 assert.ok(after.live.calls.length>before.live.calls.length,'surviving app keeps its own polling');
 await page.evaluate(()=>{window.appLifetime.mount('one','en');window.appLifetime.releaseRetired();});
 await one.locator('.app[data-runtime-online="true"]').waitFor();
 const fresh=one.locator('.decision-card .primary');await fresh.waitFor();
 assert.equal(await fresh.innerText(),'Allow shared screen access');assert.equal(await fresh.isEnabled(),true,'remount gets a fresh request map even before old pending work finishes');
 assert.equal(await page.getByText('late-event',{exact:true}).count(),0,'late events from a retired host never reach a new mount');
 assert.deepEqual(errors,[]);
 await page.evaluate(async()=>{await window.appLifetime.unmount('one');await window.appLifetime.unmount('two');});
 const retired=await page.evaluate(()=>window.appLifetime.retired());
 assert.ok(retired.every(app=>app.disposed&&app.listeners===0));
 assert.equal(await page.evaluate(()=>window.appLifetime.translate('ko','host.connect')),'Conn에 연결');
 assert.equal(await page.evaluate(()=>window.appLifetime.translate('en','tab.n',{n:3})),'tab 3');
 assert.equal(await page.evaluate(()=>window.appLifetime.translate('en','missing.key')),'missing.key');
 await page.evaluate(()=>{window.appLifetime.mount('two','en');window.appLifetime.uncertain('two');});
 await two.locator('.app[data-runtime-online="true"]').waitFor();
 await two.locator('.decision-card .primary').click();
 await two.locator('.decision-card [role="alert"]').filter({hasText:'Check the terminal and current request state before trying again.'}).waitFor();
 await two.locator('.app-menu .trigger').click();
 await two.getByRole('menuitem',{name:/^Settings/}).click();
 await two.locator('.category-list').getByRole('button',{name:'Appearance',exact:true}).click();
 await two.getByRole('button',{name:'한국어',exact:true}).click();
 await two.locator('.sheet header .btn.ghost').click();
 await two.locator('.decision-card [role="alert"]').filter({hasText:'다시 시도하기 전에 터미널과 현재 요청 상태를 확인하세요.'}).waitFor();
 await page.screenshot({path:join(output,'uncertain-request-ko.png'),fullPage:true});
 await page.evaluate(()=>window.appLifetime.unmount('two'));
 await page.goto(`${server.resolvedUrls.local[0]}tests/app-lifetime/native.html`);
 await page.getByRole('alert').filter({hasText:'Synthetic owner attachment failure'}).waitFor();
 assert.equal(await page.locator('input[type="password"]').count(),0,'native connection never asks for a server code');
 assert.equal((await page.evaluate(()=>window.nativeLifetime.inspect())).attachments,1);
 await page.getByRole('button',{name:'Reconnect',exact:true}).click();
 await page.locator('.app[data-runtime-online="true"]').waitFor();
 assert.equal((await page.evaluate(()=>window.nativeLifetime.inspect())).attachments,2);
 await page.evaluate(()=>window.nativeLifetime.fenceNext());
 await page.getByRole('alert').filter({hasText:'This window’s connection changed.'}).waitFor();
 await page.waitForFunction(()=>window.nativeLifetime.inspect().listeners===0);
 await page.waitForTimeout(1200);
 assert.equal((await page.evaluate(()=>window.nativeLifetime.inspect())).attachments,2,'a fenced native view never takes ownership automatically');
 await page.screenshot({path:join(output,'native-reconnect.png'),fullPage:true});
 await page.getByRole('button',{name:'Reconnect',exact:true}).click();
 await page.locator('.app[data-runtime-online="true"]').waitFor();
 assert.equal((await page.evaluate(()=>window.nativeLifetime.inspect())).attachments,3);
 await page.evaluate(()=>window.nativeLifetime.unmount());
 assert.equal((await page.evaluate(()=>window.nativeLifetime.inspect())).listeners,0);
 assert.deepEqual(errors,[]);
 await testWebBootstrap(browser,server.resolvedUrls.local[0],output);
 const checks = [
  'two actual ConnApp roots', 'same-ID busy and retry isolation', 'language and settings isolation',
  'links use own host port', 'unmount polling/listener cleanup', 'late response/event fencing', 'fresh state on remount',
  'pure translations share EN/KO dictionary', 'uncertain decision tells user to check terminal/state in EN and KO',
  'native attachment failure shown in shared UI', 'native fenced view stops without automatic takeover',
  'native explicit retry uses fresh attachment and metadata', 'initial code-required visit has no error',
  'English default matches bootstrap/app despite Korean browser locale', 'saved Korean preference matches bootstrap/app',
  'web reauthentication after connection shows error', 'web fenced view requires explicit takeover',
  'ordered input failure fences queued suffix and requires explicit cookie reconnect',
  'fresh checkpoint accepts new input without replaying old input',
  'failed reconnect checkpoint presents explicit recovery instead of a locked terminal',
  'socket close preceding lost-input rejection still requires explicit recovery',
 ];
 writeFileSync(join(output,'result.json'),JSON.stringify({status:'passed',scope:'Actual product UI with minimal host/IPC fixtures; no native OS, PTY, or MCP execution.',checks,errors},null,2));
 console.log(`PASS app mount/language/request/host-link/disposal isolation. Evidence: ${output}`);
} catch (error) {
 await page?.screenshot({path:join(output,'failure.png'),fullPage:true}).catch(()=>{});
 console.error(`App lifetime evidence: ${output}`);
 throw error;
} finally { await browser?.close();await server.close(); }
