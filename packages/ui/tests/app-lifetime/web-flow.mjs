import assert from 'node:assert/strict';
import { join } from 'node:path';

function ownerResult(name, terminal) {
  switch (name) {
    case 'admission_snapshot': return {revision:0,pending:[]};
    case 'activity_snapshot': return {revision:0,connections:[]};
    case 'profiles_catalog': return {config:{version:1,revision:0,defaultProfile:'fixture',profiles:[]},availability:{}};
    case 'extensions_status': return {apiVersion:1,extensions:[],settings:{theme:'conn.theme.midnight'},themes:[]};
    case 'start': return {socket:'web-fixture',session:terminal?'fixture-term':null,sessions:terminal?['fixture-term']:[],externalPending:false};
    case 'status': return {shared:false,externalOrigin:false,surfaceAvailable:true,inputPending:false,externalStarting:false,externalInputAvailable:false,processAlive:true,attended:true,profileId:'fixture',profileName:'Fixture',reviewRequired:false,mode:'autopilot',effectiveMode:'autopilot',controlGate:false,pacing:{minWriteIntervalMs:0,enterGraceMs:0,leaseTtlSecs:60,approvalTtlSecs:300},sessionAllows:[],affordanceMask:null,connectedAgents:[],controller:{type:'human'},attentionRequest:null,openedBy:null,lastAgent:null,pending:[],proposal:null,controlRequests:[]};
    case 'ui_ready': case 'log': return null;
    default: throw new Error(`Unhandled web fixture command: ${name}`);
  }
}

/** Actual WebRoot/WebTransport/ConnApp, with HTTP/WS responses at the host boundary. */
export async function testWebBootstrap(browser, baseUrl, output) {
  const page=await browser.newPage({locale:'ko-KR',viewport:{width:1000,height:750},reducedMotion:'reduce'});
  const errors=[],hellos=[];
  page.on('pageerror',error=>errors.push(error.message));
  let authenticated=false, socket, epoch=0, terminal=false, failInput=false, failAttach=false, heldInput;
  const input=[];
  await page.route('**/api/info',route=>route.fulfill({json:{authenticated,protocol:1,buildVersion:'fixture'}}));
  await page.route('**/api/bootstrap',route=>{
    authenticated=route.request().postDataJSON().token==='fixture-code';
    return route.fulfill({status:authenticated?200:401,json:{}});
  });
  await page.routeWebSocket('**/api/ws',route=>{
    socket=route;
    let streamSeq=0;
    const output=(size,reset=false)=>route.send(JSON.stringify({event:'ss:output',epoch,payload:{epoch,session:'fixture-term',generation:1,outputSeq:streamSeq+1,streamSeq:++streamSeq,reset,size,data:reset?Buffer.from('fixture> ').toString('base64'):''}}));
    route.onMessage(raw=>{
      const message=JSON.parse(String(raw));
      if(message.type==='hello') {
        hellos.push(message);
        route.send(JSON.stringify({type:'hello',protocol:1,runtimeId:'web-fixture',buildVersion:'fixture',epoch:++epoch,viewId:'web'}));
      } else {
        assert.equal(message.epoch,epoch);assert.ok(message.operationId>0);
        if(message.name==='input') {
          input.push(message.args.data);
          if(failInput) { failInput=false;heldInput=()=>route.send(JSON.stringify({id:message.id,error:'owner_busy',code:'owner_busy'}));return; }
          route.send(JSON.stringify({id:message.id,result:null}));return;
        }
        if(message.name==='attach_output') {
          if(failAttach) { failAttach=false;route.send(JSON.stringify({id:message.id,error:'checkpoint temporarily unavailable',code:'command_failed'}));return; }
          output({rows:24,cols:80},true);
          route.send(JSON.stringify({id:message.id,result:null}));return;
        }
        if(message.name==='resize') {
          output({rows:message.args.rows,cols:message.args.cols});
          route.send(JSON.stringify({id:message.id,result:null}));return;
        }
        route.send(JSON.stringify({id:message.id,result:ownerResult(message.name,terminal)}));
      }
    });
  });
  try {
    await page.goto(`${baseUrl}tests/app-lifetime/web.html`);
    await page.getByRole('button',{name:'Connect',exact:true}).waitFor();
    await page.waitForFunction(()=>!document.querySelector('.host-connection button').disabled);
    assert.equal(await page.evaluate(()=>navigator.language),'ko-KR');
    assert.equal(await page.getByRole('alert').count(),0,'a fresh code-required visit is not a failed connection');
    await page.screenshot({path:join(output,'web-first-visit.png')});
    await page.getByLabel('Connection code',{exact:true}).fill('wrong-code');
    await page.getByRole('button',{name:'Connect',exact:true}).click();
    await page.getByRole('alert').filter({hasText:'Check the connection code.'}).waitFor();
    await page.getByLabel('Connection code',{exact:true}).fill('fixture-code');
    await page.getByRole('button',{name:'Connect',exact:true}).click();
    await page.locator('.app[data-runtime-online="true"]').waitFor();
    await page.locator('.app-menu .trigger').click();
    await page.getByRole('menuitem',{name:/^Settings/}).waitFor();
    assert.equal(hellos.length,1);

    authenticated=false;
    await socket.close({code:1001,reason:'fixture authentication expired'});
    await page.getByRole('alert').filter({hasText:'Reconnect using the server connection code.'}).waitFor();
    assert.equal(await page.locator('.app').count(),0);
    await page.getByLabel('Connection code',{exact:true}).fill('fixture-code');
    await page.getByRole('button',{name:'Connect',exact:true}).click();
    await page.locator('.app[data-runtime-online="true"]').waitFor();
    socket.send(JSON.stringify({type:'attachment_fenced'}));
    await page.getByRole('heading',{name:'Open in another view',exact:true}).waitFor();
    assert.equal(await page.locator('input[type="password"]').count(),0);
    await page.waitForTimeout(700);
    assert.equal(hellos.length,2,'a fenced view cannot automatically take ownership back');
    await page.getByRole('button',{name:'Continue here',exact:true}).click();
    await page.locator('.app[data-runtime-online="true"]').waitFor();
    assert.deepEqual(hellos.map(hello=>hello.takeover),[false,false,true]);
    await page.evaluate(()=>localStorage.setItem('ss:lang','ko'));
    authenticated=false;
    await page.reload();
    await page.getByRole('button',{name:'연결',exact:true}).waitFor();
    await page.waitForFunction(()=>!document.querySelector('.host-connection button').disabled);
    assert.equal(await page.getByRole('alert').count(),0);
    await page.getByLabel('연결 코드',{exact:true}).fill('fixture-code');
    await page.getByRole('button',{name:'연결',exact:true}).click();
    await page.locator('.app[data-runtime-online="true"]').waitFor();
    await page.locator('.app-menu .trigger').click();
    await page.getByRole('menuitem',{name:/^설정/}).waitFor();

    // Use the actual Term input path. Hold the first reply so B/C queue behind A.
    terminal=true;failInput=true;
    await page.evaluate(()=>localStorage.setItem('ss:lang','en'));
    await page.reload();
    await page.locator('.host[data-terminal-ready="true"]').waitFor();
    await page.locator('.xterm-helper-textarea').focus();
    await page.keyboard.type('ABC');
    await page.waitForTimeout(50);
    assert.ok(heldInput,'genuine terminal input reached the owner transport');
    heldInput();
    await page.getByRole('alert').filter({hasText:'Reconnect, then check the terminal and request state before trying again.'}).waitFor();
    assert.equal(await page.locator('.app').count(),0);
    assert.equal(await page.locator('input[type="password"]').count(),0,'uncertain input reconnects with existing authentication');
    const countBeforeReconnect=hellos.length;
    await page.waitForTimeout(700);
    assert.equal(hellos.length,countBeforeReconnect,'ordered failures do not reconnect automatically');
    assert.deepEqual(input,['A'],'queued input is fenced before reaching the host');
    await page.getByRole('button',{name:'Reconnect',exact:true}).click();
    await page.locator('.host[data-terminal-ready="true"]').waitFor();
    assert.equal(hellos.at(-1).takeover,false,'ordinary retry uses the cookie and does not request a takeover');
    assert.deepEqual(input,['A'],'reconnection never replays input');
    await page.locator('.xterm-helper-textarea').focus();
    await page.keyboard.type('Z');
    await page.waitForTimeout(50);
    assert.deepEqual(input,['A','Z'],'only fresh user input is accepted after the new checkpoint');

    // A failed checkpoint must leave the same explicit escape route, not a locked terminal.
    failAttach=true;
    await socket.close({code:1001,reason:'fixture checkpoint reconnect'});
    await page.getByRole('alert').filter({hasText:'Reconnect, then check the terminal and request state before trying again.'}).waitFor();
    const countAfterFailedCheckpoint=hellos.length;
    await page.waitForTimeout(700);
    assert.equal(hellos.length,countAfterFailedCheckpoint);
    await page.getByRole('button',{name:'Reconnect',exact:true}).click();
    await page.locator('.host[data-terminal-ready="true"]').waitFor();
    assert.deepEqual(input,['A','Z']);

    // Connection notification can arrive before rejection of its pending input.
    failInput=true;heldInput=undefined;
    await page.locator('.xterm-helper-textarea').focus();
    await page.keyboard.type('LMN');
    await page.waitForTimeout(50);
    assert.ok(heldInput);
    const countBeforeLostResponse=hellos.length;
    await socket.close({code:1001,reason:'fixture input response lost'});
    await page.getByRole('alert').filter({hasText:'Reconnect, then check the terminal and request state before trying again.'}).waitFor();
    await page.waitForTimeout(700);
    assert.equal(hellos.length,countBeforeLostResponse,'lost input outcome still requires explicit reconnect after the close event');
    assert.deepEqual(input,['A','Z','L']);
    await page.getByRole('button',{name:'Reconnect',exact:true}).click();
    await page.locator('.host[data-terminal-ready="true"]').waitFor();
    assert.deepEqual(input,['A','Z','L'],'neither the uncertain input nor its queued suffix is replayed');
    assert.deepEqual(errors,[]);
  } catch(error) {
    await page.screenshot({path:join(output,'web-flow-failure.png')}).catch(()=>{});
    throw error;
  } finally { await page.close(); }
}
