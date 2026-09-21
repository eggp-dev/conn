// Compare the installed production xterm parser against conn-core, without mocks.
import {createRequire} from 'node:module';
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
import {strict as assert} from 'node:assert';
globalThis.self=globalThis;
const {Terminal}=createRequire(import.meta.url)('@xterm/xterm');
const {Unicode11Addon}=createRequire(import.meta.url)('@xterm/addon-unicode11');
const core=spawn('cargo',['run','--quiet','--locked','-p','conn-core','--example','screen_probe'],{cwd:'../..',stdio:['pipe','pipe','inherit']});
const lines=createInterface({input:core.stdout})[Symbol.asyncIterator]();
const call=async op=>{core.stdin.write(JSON.stringify(op)+'\n');const row=await lines.next();assert.ok(!row.done,'core probe exited');return JSON.parse(row.value);};
let assertions=0;
async function compare(name,rows,cols,ops){
 const term=new Terminal({rows,cols,scrollback:5000,allowProposedApi:true});
 term.loadAddon(new Unicode11Addon());term.unicode.activeVersion='11';
 try{
  await call({reset:true,rows,cols});
  for(const [index,op] of ops.entries()){
   if(op.text!==undefined)await new Promise(resolve=>term.write(op.text,resolve));else term.resize(op.cols,op.rows);
   const result=await call(op),b=term.buffer.active;
   const expected=Array.from({length:term.rows},(_,i)=>(b.getLine(b.baseY+i)?.translateToString(true)??'').trimEnd());
   assert.deepEqual(result.screen,expected,`${name} step ${index}: ${JSON.stringify(op)}`);
   assert.deepEqual(result.cursor,{row:b.cursorY,col:b.cursorX},`${name} cursor step ${index}`);assertions++;
  }
 }finally{term.dispose()}
}
try{
 for(const cols of [8,13,40,80]){
  const narrow=Math.max(3,Math.floor(cols/2));
  for(const text of ['abcdefghij'.repeat(13),'한글/가나다/경로/'.repeat(9),'wide:界🙂e\u0301/'.repeat(11)]){
   await compare(`wrap-${cols}-${text.slice(0,4)}`,12,cols,[{text:text+'\r\n$ '},{rows:12,cols:narrow},{rows:12,cols},{rows:6,cols:narrow},{rows:16,cols},{text:'done\r\n$ '}]);
  }
 }
 await compare('scrollback',5,20,[{text:Array.from({length:30},(_,i)=>`row-${i}-`+'abcd'.repeat(9)).join('\r\n')+'\r\n$ '},{rows:3,cols:11},{rows:10,cols:30},{rows:15,cols:80},{text:'\x1b[3J'},{rows:20,cols:80}]);
 await compare('cursor-line',8,20,[{text:'completed line before input\r\n$ '+('unsubmitted-'.repeat(4))},{rows:8,cols:12},{rows:10,cols:30}]);
 await compare('alternate',6,20,[{text:'normal long line to retain\r\n$ \x1b[?1049h\x1b[Heditor\r\nlast line\x1b[6;1Hbottom'},{rows:4,cols:14},{rows:8,cols:30},{text:'\x1b[?1049l'}]);
 console.log(`PASS ${assertions} core/xterm grid and cursor comparisons`);
}finally{core.stdin.end();core.kill();}
