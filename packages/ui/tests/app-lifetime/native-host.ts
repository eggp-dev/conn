// Minimal Tauri boundary fixture. The actual NativeRoot, nativePorts and ConnApp are used.
const handlers = new Map<string, Set<(event:unknown)=>void>>();
const requests: {name:string,args:Record<string,any>}[] = [];
let attachments = 0, fence = false;
export const inspect = () => ({attachments,requests:[...requests],listeners:[...handlers.values()].reduce((n,set)=>n+set.size,0)});
export const fenceNext = () => { fence=true; };
export async function invoke(name:string,args:Record<string,any>={}) {
  requests.push({name,args});
  if(name==='owner_attach') {
    if(++attachments===1) throw 'Synthetic owner attachment failure';
    return {protocol:1,runtimeId:'native-fixture',buildVersion:'fixture',epoch:attachments,viewId:'main'};
  }
  if(name==='app_update') return {phase:'unsupported',current:'fixture',notes:'',preview:false,supported:false,downloaded:0};
  if(name!=='dispatch') throw new Error(`Unhandled native fixture command: ${name}`);
  if(fence) { fence=false; throw 'attachment_fenced'; }
  if(args.meta?.epoch!==attachments || !Number.isSafeInteger(args.meta?.operationId)) throw new Error('Native owner metadata missing');
  switch(args.name) {
    case 'admission_snapshot': return {revision:0,pending:[]};
    case 'activity_snapshot': return {revision:0,connections:[]};
    case 'profiles_catalog': return {config:{version:1,revision:0,defaultProfile:'fixture',profiles:[]},availability:{}};
    case 'extensions_status': return {apiVersion:1,extensions:[],settings:{theme:'conn.theme.midnight'},themes:[]};
    case 'start': return {socket:'native-fixture',session:null,sessions:[],externalPending:false};
    case 'ui_ready': case 'log': return null;
    default: throw new Error(`Unhandled native owner command: ${args.name}`);
  }
}
export async function listen(name:string,handler:(event:unknown)=>void) {
  const group=handlers.get(name)??new Set(); group.add(handler);handlers.set(name,group);
  return ()=>group.delete(handler);
}
export const getCurrentWebviewWindow=()=>({label:'main'});
export const getCurrentWindow=()=>({isFocused:async()=>true,requestUserAttention:async()=>{}});
export const UserAttentionType={Informational:2};
