import assert from 'node:assert/strict';
import test from 'node:test';
import { createRequire } from 'node:module';
import { canPublishSurface, surfaceChannel, viewportFrame, type SurfaceFrame } from '../../src/lib/surface.ts';
if (!('self' in globalThis)) Object.defineProperty(globalThis,'self',{value:globalThis,configurable:true});
const require=createRequire(import.meta.url);
const {Terminal}=require('@xterm/xterm') as typeof import('@xterm/xterm');
const stamp={surfaceId:'display',generation:1,revision:1,outputSeq:1};
const write=(t:InstanceType<typeof Terminal>,text:string)=>new Promise<void>(resolve=>t.write(text,resolve));

test('real xterm viewport preserves displayed Unicode and masks concealed cells',async()=>{
 const term=new Terminal({cols:40,rows:4,allowProposedApi:true});
 try {
  await write(term,'ID: demo-user\r\nPassword: ******\r\n\x1b[8mconcealed-비밀\x1b[0m visible e\u0301🙂');
  const frame=viewportFrame(term,stamp);
  assert.equal(frame.screen[0],'ID: demo-user');assert.equal(frame.screen[1],'Password: ******');
  assert.ok(!JSON.stringify(frame).includes('concealed'));assert.ok(!JSON.stringify(frame).includes('비밀'));
  assert.ok(frame.screen[2].endsWith('visible e\u0301🙂'));
 } finally {term.dispose();}
});
test('screen clearing and alternate buffer do not include normal-screen history',async()=>{
 const term=new Terminal({cols:30,rows:3,scrollback:20,allowProposedApi:true});
 try {
  await write(term,'old private screen\r\nsecond\r\nthird\r\nfourth\r\nfifth');
  assert.ok(!viewportFrame(term,stamp).screen.join('\n').includes('old private'));
  await write(term,'\x1b[?1049h\x1b[Heditor');
  const editor=viewportFrame(term,stamp);assert.equal(editor.alternateScreen,true);assert.ok(editor.screen[0].startsWith('editor'));assert.ok(!editor.screen.join().includes('fourth'));
  await write(term,'\x1b[?1049l\x1b[2J\x1b[Hclean');
  assert.equal(viewportFrame(term,stamp).screen.join('\n').trim(),'clean');
 } finally {term.dispose();}
});
test('only active attended unobscured shared surfaces can publish',()=>{
 const allowed={active:true,attended:true,shared:true,focused:true,documentVisible:true,obscured:false,ready:true};
 assert.equal(canPublishSurface(allowed),true);
 for(const key of ['active','attended','shared','focused','documentVisible','ready'] as const) assert.equal(canPublishSurface({...allowed,[key]:false}),false,key);
 assert.equal(canPublishSurface({...allowed,obscured:true}),false);
});
test('queued stale publication cannot overtake invalidation or disposal',async()=>{
 const calls:string[]=[];
 const channel=surfaceChannel(async command=>{calls.push(command);},'session','surface');
 const frame={...stamp,rows:1,cols:2,screen:['ok'],cursor:null,alternateScreen:false,visible:true} satisfies SurfaceFrame;
 const stale=channel.publish(frame);const invalidate=channel.invalidate();await Promise.all([stale,invalidate]);
 assert.deepEqual(calls,['invalidate_surface']);
 await channel.publish({...frame,generation:2});
 await channel.dispose();await channel.publish({...frame,generation:3});
 assert.deepEqual(calls,['invalidate_surface','publish_surface','invalidate_surface']);
});
