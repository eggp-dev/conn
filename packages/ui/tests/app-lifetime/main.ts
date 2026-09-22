// State/lifetime boundary fixture: two actual ConnApp roots, no replacement product UI.
// This test deliberately supplies minimal host responses; it does not claim PTY/MCP coverage.
import { mount, unmount } from 'svelte';
import { ConnApp, translate, type ConnPorts, type ConnTransport } from '../../src/index';
import '../../src/styles/app.css';

type Label = 'one' | 'two';
type Handler = (event: {payload: any}) => void;
type Fixture = ReturnType<typeof fixture>;
function fixture(label: Label, lang = 'en') {
  const listeners = new Map<string, Set<Handler>>();
  const calls: string[] = [];
  const links: string[] = [];
  const saved = new Map<string, string>([['ss:lang', lang]]);
  let pending = true;
  let revision = 0;
  let disposed = false;
  let fail: 'retryable' | 'outcome_unknown' | null = label === 'two' ? 'retryable' : null;
  let release: (() => void) | undefined;
  const held = new Promise<void>(resolve => { release = resolve; });
  const emit = (name: string, payload: unknown) => { for (const handler of listeners.get(name) ?? []) handler({payload}); };
  const transport: ConnTransport = {
    async invoke<T>(name: string): Promise<T> {
      calls.push(name);
      let result: unknown;
      switch (name) {
        case 'admission_snapshot': result = {revision, pending: pending ? [{connId:7, agentId:`fixture-${label}`}] : []}; break;
        case 'activity_snapshot': result = {revision, connections:[]}; break;
        case 'profiles_catalog': result = {config:{version:1, revision:0, defaultProfile:'fixture', profiles:[]}, availability:{}}; break;
        case 'extensions_status': result = {apiVersion:1, extensions:[], settings:{theme:'conn.theme.midnight'}, themes:[]}; break;
        case 'start': result = {socket:`fixture-${label}`, session:null, sessions:[], externalPending:false}; break;
        case 'decide_admission':
          if (label === 'one') await held;
          if (fail) {
            const code=fail; fail=null;
            if(code==='outcome_unknown') throw Object.assign(new Error('Connection lost; the command may have completed.'), {code});
            throw new Error('Synthetic retryable decision failure');
          }
          pending = false; revision++;
          emit('ss:admission',{connId:7,agentId:`fixture-${label}`,revision,state:'granted'});
          result = {decided:true}; break;
        case 'agent_integrations': result = {clients:[],canConfigure:false,backupPath:''}; break;
        case 'agent_clients': case 'agents': case 'audit_tail': result = []; break;
        case 'admission_policy': result = {ask:true}; break;
        case 'cli_status': result = {onPath:null,canConfigure:false,canInstall:false}; break;
        case 'diagnostics': result = null; break;
        case 'ui_ready': case 'log': result = null; break;
        default: throw new Error(`Unhandled fixture owner command: ${name}`);
      }
      return result as T;
    },
    async listen<T>(name: string, handler: (event:{payload:T}) => void) {
      const group=listeners.get(name) ?? new Set<Handler>(); group.add(handler);listeners.set(name,group);
      if (name === 'ss:connection') queueMicrotask(()=>handler({payload:{connected:true,epoch:1,runtimeId:label} as T}));
      return ()=>{group.delete(handler);};
    },
    dispose() { disposed=true; listeners.clear(); },
  };
  const ports: ConnPorts = {
    transport,
    storage:{getItem:key=>saved.get(key)??null,setItem:(key,value)=>{saved.set(key,value);},removeItem:key=>{saved.delete(key);}},
    attention:{isFocused:async()=>true,request:async()=>{}},
    updates:{supported:false,invoke:async()=>({phase:'unsupported',current:'fixture',notes:'',preview:false,supported:false,downloaded:0})},
    links:{open:async url=>{links.push(url);}},
    clipboard:{writeText:async()=>{}},
    platform:'web',
  };
  return { ports, emit, release:()=>release?.(), uncertain:()=>{fail='outcome_unknown';}, inspect:()=>({calls:[...calls],links:[...links],saved:Object.fromEntries(saved),disposed,listeners:[...listeners.values()].reduce((sum,group)=>sum+group.size,0)}) };
}
const instances = new Map<Label, {component:ReturnType<typeof mount>, fixture:Fixture}>();
const retired: Fixture[]=[];
function mountSlot(label: Label, lang = 'en') {
  if(instances.has(label))throw new Error('Unmount a slot before remounting it');
  const host=fixture(label,lang);
  const component=mount(ConnApp,{target:document.getElementById(label)!,props:{ports:host.ports}});
  instances.set(label,{component,fixture:host});
}
mountSlot('one');mountSlot('two');
(window as any).appLifetime = {
  inspect:(label:Label)=>instances.get(label)!.fixture.inspect(),
  uncertain:(label:Label)=>instances.get(label)!.fixture.uncertain(),
  translate,
  unmount:async(label:Label)=>{const old=instances.get(label)!;instances.delete(label);retired.push(old.fixture);await unmount(old.component);},
  mount:mountSlot,
  releaseRetired:()=>{for(const old of retired){old.release();old.emit('ss:admission',{connId:7,agentId:'late-event',revision:99,state:'pending'});}},
  retired:()=>retired.map(old=>old.inspect()),
};
