import { test } from 'node:test';
import assert from 'node:assert/strict';
import { groupConnections } from '../../src/lib/connections.ts';

test('duplicate agent names show connection counts and keep idle sockets inspectable', () => {
  const groups = groupConnections([
    {connId:3,agentId:'copilot',idleSecs:720,sessions:['t1']},
    {connId:2,agentId:'copilot',idleSecs:0,sessions:['t1','t2']},
    {connId:1,agentId:'claude',idleSecs:3,sessions:['t2']},
  ]);
  assert.deepEqual(groups.map(g=>[g.agentId,g.connections.length]), [['claude',1],['copilot',2]]);
  assert.equal(groups[1].connections[1].idleSecs,720);
  assert.deepEqual(groups[1].connections[0].sessions,['t1','t2']);
  assert.deepEqual(groupConnections([]),[]);
});
