import {test} from 'node:test';
import assert from 'node:assert/strict';
import {connMarkState, graceProgress, type MarkInput} from '../../src/lib/connMark.ts';
const base: MarkInput = {processAlive:true,policyBlockedUntil:0,controller:{type:'human'},ctlReq:null,approval:null,attention:null,proposal:null,grace:null};
test('terminal availability and human decisions outrank agent ownership',()=>{
 const agent={...base,controller:{type:'agent' as const}};
 assert.equal(connMarkState(base,true,0),'human');
 assert.equal(connMarkState(agent,true,0),'agent');
 assert.equal(connMarkState({...agent,ctlReq:{}},true,0),'request');
 assert.equal(connMarkState({...agent,proposal:{ready:true}},true,0),'request');
 assert.equal(connMarkState({...agent,approval:{}},true,0),'request');
 assert.equal(connMarkState({...agent,processAlive:false,approval:{}},true,0),'offline');
 assert.equal(connMarkState(agent,false,0),'offline');
});
test('block feedback expires and grace ownership remains visible',()=>{
 const agent={...base,controller:{type:'agent' as const},policyBlockedUntil:2400};
 assert.equal(connMarkState(agent,true,500),'blocked');
 assert.equal(connMarkState(agent,true,2400),'agent');
 assert.equal(connMarkState({...base,grace:{start:0,ms:2000}},true,500),'grace');
});
test('grace uses the real timestamp and duration with no fake progress on cancellation',()=>{
 const grace={start:1000,ms:5000};
 assert.equal(graceProgress(grace,0),0);
 assert.equal(graceProgress(grace,3500),.5);
 assert.equal(graceProgress(grace,9000),1);
 assert.equal(graceProgress(null,3500),0);
 assert.equal(graceProgress({start:0,ms:0},0),1);
 assert.equal(connMarkState({...base,grace},true,3000),'grace');
});
