// Browser-only test fixture. This entry is never imported by the application build.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { mount } from "svelte";
import App from "../../src/App.svelte";
import "../../src/app.css";
import { st } from "../../src/lib/store.svelte";
import { setLang } from "../../src/lib/i18n.svelte";
import type { Profile, ProfileConfig } from "../../src/lib/profiles.svelte";
setLang("ko");
const base:Profile={id:"local-bash",name:"Bash",enabled:true,backend:"local",shell:"posix",program:"/bin/bash",args:["-l"],cwd:null,env:{},target:null,port:null};
let config:ProfileConfig={version:1,revision:0,defaultProfile:base.id,profiles:[base,{...base,id:"ssh-dev",name:"Development server",backend:"ssh",program:"",shell:"custom",args:[],target:"dev@example.invalid"},{...base,id:"wsl-ubuntu",name:"WSL Ubuntu",backend:"wsl",target:"Ubuntu"}]};
const sessions:Record<string,Profile>={t1:structuredClone(base)};
let active="t1", sequence=1;
const catalog=()=>({config:structuredClone(config),availability:Object.fromEntries(config.profiles.map(p=>[p.id,{available:p.enabled && p.backend!=="wsl",message:p.backend==="wsl" ? "WSL profiles can be launched on Windows only" : p.enabled ? "Ready" : "Profile is disabled"}]))});
mockWindows("main");
mockIPC((command,args:any)=>{
  if (command === "dispatch") { command = args.name; args = args.args; }
  if(command==="profiles_catalog") return catalog();
  if(command==="profiles_discover") return [{...base,id:"local-fish",name:"fish",program:"/usr/bin/fish",shell:"fish"}];
  if(command==="profiles_save") {
    const next=args.config as ProfileConfig;
    if(next.revision!==config.revision) throw new Error("Profiles changed in another window or CLI. Reload before saving.");
    if(!next.profiles.find(p=>p.id===next.defaultProfile)?.enabled)throw new Error("The default profile must be enabled");
    config={...structuredClone(next),revision:next.revision+1};return catalog();
  }
  if(command==="profiles_test") {
    const p=args.profile as Profile;
    if(p.target==="unreachable.invalid")throw new Error("Connection test timed out");
    return {available:p.backend!=="wsl",message:p.backend==="wsl" ? "WSL profiles can be launched on Windows only" : "Test fixture: target connection verified"};
  }
  if(command==="start")return {socket:"/test/conn.sock",shell:"/bin/bash",session:active,sessions:[active]};
  if(command==="open_tab") {
    const p=config.profiles.find(p=>p.id===(args.profileId ?? config.defaultProfile));
    if(!p || !catalog().availability[p.id].available)throw new Error("Profile unavailable");
    const id=`t${++sequence}`;sessions[id]=structuredClone(p);active=id;return id;
  }
  if(command==="attend"){active=args.session;return true;}
  if(command==="close_tab"){delete sessions[args.session];return Object.keys(sessions)[0];}
  if(command==="status") {
    const p=sessions[args.session] ?? base;
    return {mode:"autopilot",effectiveMode:"autopilot",controlGate:false,sessionAllows:[],affordanceMask:null,connectedAgents:[],connectedFrontends:[],pacing:{minWriteIntervalMs:0,enterGraceMs:0,leaseTtlSecs:60,approvalTtlSecs:300},attended:args.session===active,processAlive:true,controller:{type:"human"},pending:[],controlRequests:[],profileId:p.id,profileName:p.name,reviewRequired:p.backend!=="local" || p.shell!=="posix"};
  }
  if(command==="audit_tail" || command==="agents")return [];
  if(command==="log" || command==="input" || command==="resize")return null;
  return null;
},{shouldMockEvents:true});
mount(App,{target:document.getElementById("app")!});
st.settingsTab="profiles";st.settingsOpen=true;
