import assert from 'node:assert/strict';
import { test } from 'node:test';
import { Admissions } from '../../src/lib/collaboration/admissions.ts';
import { applySessionEvent, applySessionSnapshot, type SessionSnapshot } from '../../src/lib/collaboration/session.ts';
import type { TabState } from '../../src/lib/store.svelte.ts';
import {pendingKind,requestStillPending,sessionLabel} from '../../src/lib/collaboration/activity.ts';
const pending = (connId: number, revision: number) => ({connId, agentId: 'same-name', revision, state: 'pending' as const});
test('late startup snapshot cannot resurrect a request resolved in another window', () => {
  const state = new Admissions();
  state.event(pending(1, 1)); state.event({...pending(1, 2), state: 'granted'});
  state.snapshot({revision: 1, pending: [pending(1, 1)]}); assert.deepEqual(state.requests, []);
});
test('out-of-order events for different connections remain independent', () => {
  const state = new Admissions(); state.event(pending(2, 2)); state.event(pending(1, 1));
  assert.deepEqual(state.requests.map(a => a.connId), [1, 2]);
});
test('snapshot recovers missed events without overwriting newer changes', () => {
  const state = new Admissions(); state.event(pending(1, 1)); state.event(pending(3, 5));
  state.snapshot({revision: 4, pending: [pending(2, 3)]}); assert.deepEqual(state.requests.map(a => a.connId), [2, 3]);
  state.event(pending(1, 2)); assert.deepEqual(state.requests.map(a => a.connId), [2, 3]);
});
test('closed IDs and same-name reconnects do not inherit admission', () => {
  const state = new Admissions(); state.event({...pending(1, 3), state: 'closed'}); state.event(pending(2, 4));
  state.event(pending(1, 1)); assert.deepEqual(state.requests.map(a => a.connId), [2]);
});
test('stale snapshots cannot erase a current request', () => {
  const state = new Admissions(); state.snapshot({revision: 5, pending: [pending(1, 5)]}); state.snapshot({revision: 4, pending: []});
  assert.deepEqual(state.requests.map(a => a.connId), [1]);
});

import { Reconciler } from '../../src/lib/collaboration/reconcile.ts';
test('a status read racing a live event is retried before applying', async () => {
  let resolve!: (value: string) => void; let reads = 0; const applied: string[] = [];
  const state = new Reconciler<string>(() => ++reads === 1 ? new Promise(r => resolve = r) : Promise.resolve('new'), (_id, value) => applied.push(value));
  const pending = state.sync('one'); state.changed('one'); resolve('old'); await pending;
  assert.deepEqual(applied, ['new']); assert.equal(reads, 2);
});
test('parallel polls coalesce and unrelated sessions do not invalidate each other', async () => {
  let resolve!: (value: string) => void; let reads = 0; const applied: string[] = [];
  const state = new Reconciler<string>(() => { reads++; return new Promise(r => resolve = r); }, (_id, value) => applied.push(value));
  const first=state.sync('one'), second=state.sync('one'); state.changed('two'); resolve('current'); await Promise.all([first,second]);
  assert.equal(reads,1); assert.deepEqual(applied,['current']);
});
test('failed reads preserve current state and a later retry can repair it', async () => {
  let fail=true; const applied: string[]=[];
  const state=new Reconciler<string>(async()=>{if(fail)throw Error('offline');return 'recovered';},(_id,value)=>applied.push(value));
  await assert.rejects(state.sync('one'));assert.deepEqual(applied,[]);fail=false;await state.sync('one');assert.deepEqual(applied,['recovered']);
});
test('unmounted views do not apply late status responses', async () => {
  let resolve!: (value: string) => void; const applied: string[]=[];
  const state=new Reconciler<string>(()=>new Promise(r=>resolve=r),(_id,value)=>applied.push(value));
  const pending=state.sync('one');state.dispose();resolve('late');await pending;assert.deepEqual(applied,[]);
});

const snapshot = (): SessionSnapshot => ({ shared: true, externalOrigin: false, surfaceAvailable: true, inputPending: false, externalInputAvailable: false, processAlive: true, attended: true, profileId: 'shell', profileName: 'Shell', reviewRequired: false, mode: 'autopilot', effectiveMode: 'autopilot', controlGate: true, pacing: {minWriteIntervalMs:0, enterGraceMs:5000, leaseTtlSecs:60, approvalTtlSecs:300}, sessionAllows: [], affordanceMask: null, connectedAgents: ['one'], controller: {type:'human'}, attentionRequest: null, openedBy: null, lastAgent: null, pending: [], proposal: null, controlRequests: [] });
function view() {
  const t = { id:'one', title:'Shell', timeline: {recording:true}, controller:{type:'human'} } as TabState;
  applySessionSnapshot(t,snapshot(),'Private'); return t;
}
test('notice identity survives reorder and cannot match a replacement request',()=>{
  const t=view();t.ctlReq={id:'new',agentId:'same-name'};
  assert.equal(requestStillPending(t,{session:t.id,kind:'control',requestId:'old'}),false);
  assert.equal(requestStillPending(t,{session:t.id,kind:'control',requestId:'new'}),true);
  assert.equal(sessionLabel(['other','one'],{one:t},'one'),'2 · Shell');
  assert.equal(pendingKind(t),'control');t.shared=false;assert.equal(pendingKind(t),null);
});
test('server lease clock and revoke reasons clear on a fresh grant or private state',()=>{
  const t=view();applySessionSnapshot(t,{...snapshot(),controller:{type:'agent',agentId:'one',expiresInSecs:12}},'Private');
  assert.ok(t.leaseExpiresAt!<=Date.now()+12000 && t.leaseExpiresAt!>=Date.now()+11000);
  applySessionEvent(t,{session:'one',event:'control_revoked',agentId:'one',reason:'expired'});
  assert.equal(t.leaseExpiresAt,undefined);assert.equal(t.controlReason,'expired');
  applySessionEvent(t,{session:'one',event:'control_granted',agentId:'one'});assert.equal(t.controlReason,undefined);
  applySessionSnapshot(t,{...snapshot(),shared:false},'Private');assert.equal(t.leaseExpiresAt,undefined);
});
test('event and snapshot use the same request projection without restarting its age', () => {
  const t=view();
  const request={id:'review-1',agentId:'one',cmd:'echo fixture',label:'review'};
  applySessionEvent(t,{session:'one',event:'approval_requested',request});
  t.approval!.at=1234;
  applySessionSnapshot(t,{...snapshot(),pending:[request]},'Private');
  assert.equal(t.approval!.at,1234);
  applySessionEvent(t,{session:'two',event:'approval_resolved',approvalId:'review-1',state:'denied',by:'human'});
  assert.ok(t.approval, 'another session cannot resolve this card');
  applySessionEvent(t,{session:'one',event:'approval_resolved',approvalId:'review-1',state:'denied',by:'human'});
  assert.equal(t.approval,null);
});
test('a missed grace event is recovered using the countdown clock and not reset by polling', () => {
  const t=view();const s={...snapshot(),scheduled:{execId:'exec-1',cmd:'echo fixture'},scheduledRemainingMs:1200};
  const before=performance.now();applySessionSnapshot(t,s,'Private');
  assert.ok(t.grace!.start>=before && t.grace!.start<=performance.now());assert.equal(t.grace!.ms,1200);
  const start=t.grace!.start;
  applySessionSnapshot(t,s,'Private');assert.equal(t.grace!.start,start);
  applySessionEvent(t,{session:'one',event:'exec_cancelled',execId:'older-exec',reason:'cancelled'});assert.ok(t.grace);
  applySessionSnapshot(t,{...snapshot(),scheduled:null},'Private');assert.equal(t.grace,null);
});
test('sharing revocation immediately clears collaboration and private snapshots cannot restore it', () => {
  const t=view();t.handback=true;
  applySessionEvent(t,{session:'one',event:'control_requested',request:{requestId:'control-1',agentId:'one'}});
  applySessionEvent(t,{session:'one',event:'sharing_changed',shared:false,generation:2});
  assert.equal(t.ctlReq,null);assert.equal(t.handback,false);assert.deepEqual(t.agents,[]);assert.equal(t.timeline.recording,false);
  applySessionSnapshot(t,{...snapshot(),shared:false,controlRequests:[{requestId:'stale',agentId:'one'}]},'Private');
  applySessionEvent(t,{session:'one',event:'control_requested',request:{requestId:'late',agentId:'one'}});
  assert.equal(t.ctlReq,null);assert.deepEqual(t.agents,[]);
});

test('approval choices follow the request even when the global shell status changes later', () => {
  for (const allowSession of [true, false]) {
    const t = view();
    const request = { id:'review-context', agentId:'agent', cmd:'rm fixture', label:'delete files', review: { reason: allowSession ? 'command_policy' as const : 'unverified_shell' as const, allowSession, remote:false } };
    applySessionEvent(t, {session:'one', event:'approval_requested', request});
    assert.equal(t.approval?.review?.allowSession, allowSession);
    applySessionSnapshot(t, {...snapshot(), reviewRequired:allowSession, pending:[request]}, 'Private');
    assert.equal(t.approval?.review?.allowSession, allowSession);
    assert.equal(t.approval?.label, 'delete files');
  }
});
