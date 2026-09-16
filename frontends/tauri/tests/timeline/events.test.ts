import { test } from 'node:test';
import assert from 'node:assert/strict';
import { newTimeline, recordTimeline, visibleTimeline, timelineSteps, importSavedActivity, commandDecisions, isExternalAutomation } from '../../src/lib/timeline.ts';
const request = (id = 'r1', actor = 'codex') => ({ event: 'control_requested', request: { requestId: id, agentId: actor, reason: 'Inspect a file' } });
const grant = (actor = 'codex', lease = 'l1') => ({ event: 'control_granted', agentId: actor, lease, reason: 'Inspect a file' });
const exec = (cmd = 'pwd', policy = 'allow') => ({ event: 'agent_exec', agentId: 'codex', cmd, policy });

test('a denied control request stays visible, with its reason, and is never a command', () => {
  const s = newTimeline();
  recordTimeline(s, request(), 1);
  recordTimeline(s, { event: 'control_request_resolved', requestId: 'r1', state: 'denied' }, 2);
  assert.equal(s.items.length, 1);
  assert.equal(s.items[0].status, 'denied');
  assert.equal(s.items[0].text, 'Inspect a file');
  assert.equal(visibleTimeline(s, 'commands').length, 0);
  assert.equal(visibleTimeline(s, 'collaboration').length, 1);
});
test('grant before resolution, grace, execution and return collapse into a command with a complete process', () => {
  const s = newTimeline();
  [request(), grant(), {event:'control_request_resolved',requestId:'r1',state:'granted'},
   {event:'exec_scheduled',execId:'e1',agentId:'codex',cmd:'pwd',graceMs:5000}, exec(),
   {event:'control_revoked',agentId:'codex',lease:'l1',reason:'released'}].forEach((ev, i) => recordTimeline(s, ev, i));
  const all = visibleTimeline(s, 'all');
  assert.equal(all.length, 1);
  assert.equal(all[0].status, 'executed');
  assert.deepEqual(timelineSteps(s, all[0]).map(s => s.type), ['control_requested','control_granted','scheduled','executed','control_returned']);
  assert.equal(visibleTimeline(s, 'collaboration')[0].status, 'returned');
});
test('approval refusal and its AgentExec notification are one denied command', () => {
  const s = newTimeline();
  recordTimeline(s, {event:'approval_requested',request:{id:'a1',agentId:'codex',cmd:'rm sample',label:'delete'}});
  recordTimeline(s, {event:'approval_resolved',approvalId:'a1',state:'denied',by:'human'});
  recordTimeline(s, exec('rm sample','confirm:denied'));
  assert.equal(s.items.length, 1);
  assert.equal(s.items[0].status, 'denied');
  assert.deepEqual(s.items[0].steps.map(s=>s.type), ['approval_requested','approval_denied','denied']);
});
test('accepted copilot proposals do not duplicate the following AgentExec', () => {
  const s = newTimeline();
  recordTimeline(s, {event:'proposal_changed',proposal:{proposalId:'p1',agentId:'codex',text:'pwd',state:'ready'}});
  recordTimeline(s, {event:'proposal_resolved',proposalId:'p1',state:'executed',cmd:'pwd'});
  recordTimeline(s, exec());
  assert.equal(s.items.length,1); assert.equal(s.items[0].status,'executed');
});
test('cancelled grace remains separate from a later identical command', () => {
  const s = newTimeline();
  recordTimeline(s, {event:'exec_scheduled',execId:'e1',agentId:'codex',cmd:'pwd',graceMs:5000});
  recordTimeline(s, {event:'exec_cancelled',execId:'e1',reason:'human_input'});
  recordTimeline(s, exec()); recordTimeline(s, exec());
  assert.deepEqual(s.items.map(s=>s.status),['cancelled','executed','executed']);
});
test('multiple pending requests resolve by ID and retain independent reasons', () => {
  const s = newTimeline();
  recordTimeline(s,request('r1','a'));recordTimeline(s,request('r2','b'));
  recordTimeline(s,{event:'control_request_resolved',requestId:'r1',state:'expired'});
  recordTimeline(s,{event:'control_request_resolved',requestId:'r2',state:'denied'});
  assert.deepEqual(s.items.map(it=>[it.actor,it.status]),[['a','expired'],['b','denied']]);
});
test('each command in a lease retains its own outcome while sharing control context', () => {
  const s = newTimeline(); recordTimeline(s,grant()); recordTimeline(s,exec()); recordTimeline(s,exec('ls'));
  recordTimeline(s,{event:'control_revoked',agentId:'codex',lease:'l1',reason:'released'});
  assert.equal(visibleTimeline(s,'all').length,2);
  assert.ok(visibleTimeline(s,'commands').every(it=>timelineSteps(s,it).some(s=>s.type==='control_returned')));
});
test('saved denied audit commands are not marked executed or joined to a new lease', () => {
  const s = newTimeline(); recordTimeline(s,grant());
  importSavedActivity(s,[{action:'exec',actor:'codex',cmd:'rm sample',policy:'confirm',approval:'denied',ts:'2026-01-01T00:00:00Z'}]);
  assert.equal(s.items[1].status,'denied'); assert.equal(s.items[1].saved,true); assert.equal(s.items[1].controlId,undefined);
});
test('retention bounds the list and preserves control details when the parent ages out', () => {
  const s = newTimeline(); recordTimeline(s,grant()); recordTimeline(s,exec());
  for(let i=0;i<399;i++) recordTimeline(s,{event:'human_exec',cmd:`echo ${i}`});
  assert.equal(s.items.length,400);
  assert.ok(s.items[0].steps.some(s=>s.type==='control_granted'));
  assert.equal(s.items[0].controlId,undefined);
});
test('independent tab states never merge request IDs', () => {
  const a=newTimeline(), b=newTimeline(); recordTimeline(a,request());recordTimeline(b,request());
  recordTimeline(a,{event:'control_request_resolved',requestId:'r1',state:'denied'});
  assert.equal(b.items[0].status,'pending');
});

test('past rejected control requests survive audit import without guessing a command or a tab', () => {
  const s=newTimeline();
  importSavedActivity(s,[{action:'control_request_resolved',actor:'codex',request:'ctl-3',state:'denied',ts:'2026-09-15T22:15:50+09:00'}]);
  assert.equal(visibleTimeline(s,'all')[0].status,'denied');
  assert.equal(visibleTimeline(s,'commands').length,0);
  assert.equal(s.items[0].saved,true);
});

test('denial and saved audit retain exact parameters without inventing execution', () => {
  const originalRequest = {method:'request_control',params:{reason:'inspect',command:"cat -- 'a b.md'\n",custom:{flag:true}}};
  const s = newTimeline();
  const ev = request();
  recordTimeline(s, {...ev,request:{...ev.request,originalRequest}}, 1);
  recordTimeline(s, {event:'control_request_resolved',requestId:'r1',state:'denied'}, 2);
  assert.deepEqual(s.items[0].originalRequest, originalRequest);
  assert.equal(visibleTimeline(s,'commands').length, 0);
  const restored = newTimeline();
  importSavedActivity(restored, [{action:'control_request_resolved',state:'denied',actor:'codex',reason:'inspect',originalRequest}]);
  assert.deepEqual(restored.items[0].originalRequest, originalRequest);
  const old = newTimeline();
  importSavedActivity(old,[{action:'control_request_resolved',state:'denied',actor:'codex',reason:'run pwd'}]);
  assert.equal(old.items[0].originalRequest, undefined);
});

test('planned request remains distinct from actual command, including after parent pruning', () => {
  const s = newTimeline();
  const originalRequest = {method:'request_control',params:{command:'pwd'}};
  recordTimeline(s,{...grant(),originalRequest},1);
  recordTimeline(s,exec('ls'),2);
  assert.equal(s.items[1].text,'ls');
  assert.equal(s.items[0].originalRequest?.params.command,'pwd');
  for(let i=0;i<399;i++) recordTimeline(s,{event:'human_exec',cmd:'true'},3+i);
  assert.equal(s.items[0].text,'ls');
  assert.deepEqual(s.items[0].originalRequest,originalRequest);
});

test('live and saved policy refusals retain the reason, distinct from human denial', async () => {
  const {isPolicyBlocked, policyBlockLabel} = await import('../../src/lib/timeline.ts');
  const s = newTimeline();
  recordTimeline(s, exec('rm f && pwd', 'deny:run dangerous commands alone'));
  assert.equal(isPolicyBlocked(s.items[0]),true);
  assert.equal(policyBlockLabel(s.items[0]),'run dangerous commands alone');
  importSavedActivity(s,[{actor:'codex',action:'exec',cmd:'rm f && pwd',policy:'deny',label:'run dangerous commands alone'}]);
  assert.equal(policyBlockLabel(s.items[1]),'run dangerous commands alone');
  recordTimeline(s,exec('rm f','confirm:denied'));
  assert.equal(isPolicyBlocked(s.items[2]),false);
});

test('control grants and elapsed grace never imply command approval, including after retention', () => {
  const s = newTimeline(); recordTimeline(s, grant());
  recordTimeline(s, {event:'exec_scheduled',execId:'e1',agentId:'codex',cmd:'pwd',graceMs:5000});
  recordTimeline(s, exec());
  assert.deepEqual(commandDecisions(visibleTimeline(s,'commands')[0]), []);
  for(let i=0;i<399;i++) recordTimeline(s,{event:'human_exec',cmd:'true'});
  assert.ok(s.items[0].steps.some(s=>s.type==='control_granted'));
  assert.deepEqual(commandDecisions(s.items[0]), []);
});

test('approval, proposal acceptance and grace co-sign are distinct command decisions', () => {
  const s = newTimeline();
  recordTimeline(s,{event:'approval_requested',request:{id:'a1',agentId:'codex',cmd:'rm sample',label:'delete'}});
  recordTimeline(s,{event:'approval_resolved',approvalId:'a1',state:'granted',by:'human'});
  recordTimeline(s,exec('rm sample','confirm:granted'));
  recordTimeline(s,{event:'proposal_changed',proposal:{proposalId:'p1',agentId:'codex',text:'ls',state:'ready'}});
  recordTimeline(s,{event:'proposal_resolved',proposalId:'p1',state:'executed'});
  recordTimeline(s,exec('ls'));
  recordTimeline(s,{event:'exec_scheduled',execId:'e1',agentId:'codex',cmd:'pwd',graceMs:5000});
  recordTimeline(s,{event:'exec_cosigned',execId:'e1',agentId:'codex'});
  recordTimeline(s,exec());
  assert.deepEqual(s.items.map(commandDecisions), [['approved'],['accepted'],['cosigned']]);
  assert.equal(s.items[2].steps.filter(s=>s.type==='exec_cosigned').length,1);
  recordTimeline(s,exec());
  assert.deepEqual(commandDecisions(s.items[3]), []);
});

test('saved commands preserve explicit human acts without guessing from policy or nearby events', () => {
  const s = newTimeline();
  importSavedActivity(s,[
    {action:'exec',actor:'codex',cmd:'a',policy:'confirm',approval:'granted'},
    {action:'exec',actor:'codex',cmd:'b',policy:'confirm',by:'human_commit'},
    {action:'exec',actor:'codex',cmd:'c',policy:'allow',by:'human_cosign'},
    {action:'exec_cosigned',actor:'human',cmd:'d'},
    {action:'exec',actor:'codex',cmd:'d',policy:'allow'},
  ]);
  assert.deepEqual(s.items.map(commandDecisions), [['approved'],['accepted'],['cosigned'],[]]);
});

test('external automation remains identifiable on linked commands and saved audit records', () => {
  const s = newTimeline();
  recordTimeline(s,{...grant(),originalRequest:{method:'request_control',params:{origin:'external_automation'}}});
  recordTimeline(s,exec());
  assert.equal(isExternalAutomation(s,s.items[1]),true);
  importSavedActivity(s,[{action:'exec',actor:'AppleScript · PAM',cmd:'ssh example.invalid',policy:'allow'}]);
  assert.equal(isExternalAutomation(s,s.items[2]),true);
  recordTimeline(s,{event:'human_exec',cmd:'pwd'});
  assert.equal(isExternalAutomation(s,s.items[3]),false);
});
