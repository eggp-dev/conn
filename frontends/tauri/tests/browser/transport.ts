type Handler = (event: {payload:any})=>void;
const listeners = new Map<string,Set<Handler>>();
type OutputPacket = {session:string;data:string;outputSeq:number;generation:number};
const history = new Map<string,OutputPacket[]>();
const historyBytes = new Map<string,number>();
const pending = new Map<number,{resolve:(v:any)=>void;reject:(e:Error)=>void;timer:ReturnType<typeof setTimeout>}>();
let seq=0;
let connection:Promise<WebSocket>|undefined;
async function connected():Promise<WebSocket> {
  return connection ??= (async()=>{
    const res=await fetch("/__conn/connection");
    if(!res.ok) throw new Error("Start the Conn browser harness with npm run test:browser.");
    const config=await res.json();
    return new Promise<WebSocket>((resolve,reject)=>{
      const ws=new WebSocket(`${config.url}?token=${encodeURIComponent(config.token)}`);
      ws.onopen=()=>{for(const h of listeners.get("ss:connection")??[])h({payload:{connected:true}});resolve(ws);};
      ws.onerror=()=>reject(new Error("Cannot connect to the Conn test harness."));
      ws.onclose=()=>{
        for(const h of listeners.get("ss:connection")??[])h({payload:{connected:false}});
        const error=new Error("Conn test harness disconnected. Reload after restarting it.");
        reject(error);
        for(const p of pending.values()){clearTimeout(p.timer);p.reject(error);} pending.clear();
      };
      ws.onmessage=({data})=>{
        const v=JSON.parse(data);
        if(v.id!==undefined){const p=pending.get(v.id);if(!p)return;clearTimeout(p.timer);pending.delete(v.id);v.error ? p.reject(new Error(v.error)) : p.resolve(v.result);return;}
        if(v.event==="ss:output"){
          const id=v.payload.session;const chunks=history.get(id)??[];chunks.push(v.payload);history.set(id,chunks);
          let count=(historyBytes.get(id)??0)+v.payload.data.length;
          while(count>512*1024 && chunks.length>1)count-=chunks.shift()!.data.length;
          historyBytes.set(id,count);
        }
        for(const h of listeners.get(v.event)??[])h({payload:v.payload});
      };
    });
  })().catch(e=>{connection=undefined;throw e;});
}
export async function invoke<T>(name:string,args:Record<string,unknown>={}):Promise<T>{
  const ws=await connected();
  if(ws.readyState!==WebSocket.OPEN) throw new Error("Conn test harness disconnected. Reload after restarting it.");
  const id=++seq;
  return new Promise((resolve,reject)=>{
    const timer=setTimeout(()=>{pending.delete(id);reject(new Error(`Conn command timed out: ${name}`));},15000);
    pending.set(id,{resolve,reject,timer});
    try{ws.send(JSON.stringify({id,name,args}));}catch(e){clearTimeout(timer);pending.delete(id);reject(e);}
  });
}
export async function listen<T>(name:string,handler:(event:{payload:T})=>void):Promise<()=>void>{
  let set=listeners.get(name);if(!set)listeners.set(name,set=new Set());set.add(handler);
  // Transport replay only. All events, status and decisions originate from Rust.
  if(name==="ss:output")for(const chunks of history.values())for(const packet of chunks)handler({payload:packet as T});
  return ()=>{set!.delete(handler);};
}
