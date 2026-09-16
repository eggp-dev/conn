// Browser-only test fixture. This entry is never imported by the application build.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { mount } from "svelte";
import App from "../../src/App.svelte";
import "../../src/app.css";
import { st } from "../../src/lib/store.svelte";
import { setLang } from "../../src/lib/i18n.svelte";
import type { Profile, ProfileConfig } from "../../src/lib/profiles.svelte";
const options = new URLSearchParams(location.search);
const externalPrivate = options.has("private");
let externalInputAvailable = externalPrivate;
let externalStarting = externalPrivate && options.has("pending");
let rendererAttached = false;
let launchCancelled = false;
setLang(options.get("lang") === "en" ? "en" : "ko");
const base:Profile={id:"local-bash",name:"Bash",enabled:true,backend:"local",shell:"posix",program:"/bin/bash",args:["-l"],cwd:null,env:{},target:null,port:null};
let config:ProfileConfig={version:1,revision:0,defaultProfile:base.id,profiles:[base,{...base,id:"ssh-dev",name:"Development server",backend:"ssh",program:"",shell:"custom",args:[],target:"dev@example.invalid"},{...base,id:"wsl-ubuntu",name:"WSL Ubuntu",backend:"wsl",target:"Ubuntu"}]};
const sessions:Record<string,Profile>={t1:structuredClone(base)};
let active=options.has("existing") ? "t0" : "t1", sequence=1;
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
  if(command==="start" && options.has("pending") && !options.has("existing")) return {socket:"/test/conn.sock", shell:"",session:null,sessions:[],externalPending:true};
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
    if (externalPrivate && args.session === "t1") return {externalPrivate:true, externalStarting, externalInputAvailable:externalInputAvailable && !externalStarting,profileId:p.id,profileName:p.name,attended:true,processAlive:!externalStarting && !options.has("finished")};
    return {mode:"autopilot",effectiveMode:"autopilot",controlGate:false,sessionAllows:[],affordanceMask:null,connectedAgents:[],connectedFrontends:[],pacing:{minWriteIntervalMs:0,enterGraceMs:0,leaseTtlSecs:60,approvalTtlSecs:300},attended:args.session===active,processAlive:true,controller:{type:"human"},pending:[],controlRequests:[],profileId:p.id,profileName:p.name,reviewRequired:p.backend!=="local" || p.shell!=="posix"};
  }
  if(command==="ui_ready" && options.has("pending")) {
    setTimeout(() => { active = "t1"; void emit("ss:tab_opened", {session:"t1",externalPrivate:true,externalStarting:true,focus:true}); }, 100);
    return null;
  }
  if(command==="attach_output") {
    const session = args.session;
    if (session === "t1") rendererAttached = true;
    const publish = () => {
      if (session !== "t1") return;
      if (launchCancelled) return;
      if (options.has("abort")) { void emit("ss:tab_aborted", {session}); return; }
      if (externalStarting) {
        if (!rendererAttached) throw new Error("Child started before renderer attachment");
        externalStarting = false;
        void emit("ss:tab_opened", {session,externalPrivate:true,externalStarting:false,focus:true});
      }
      void emit("ss:output", {session,data:btoa("\x1b[5n\x1b[6n\x1b[c\x1b[>c\x1b]52;c;dGVzdC1jbGlwYm9hcmQ=\x07External terminal fixture\r\nReady for local input.\r\n")});
      // A wrongly routed command event must not enter a private title or timeline.
      void emit("ss:event", {session,event:"human_exec",cmd:"private-marker"});
    };
    setTimeout(publish, options.has("slowStart") ? Number(options.get("slowStart")) || 2000 : 0);
    return null;
  }
  if(command==="take" || command==="input") {
    externalInputAvailable = false;
    if (externalStarting) { launchCancelled = true; void emit("ss:tab_aborted", {session:args.session}); }
    return null;
  }
  if(command==="audit_tail" || command==="agents") {
    if (externalPrivate && active === "t1") throw new Error("Private fixture queried collaboration data");
    return [];
  }
  if(command==="log" || command==="input" || command==="resize")return null;
  return null;
},{shouldMockEvents:true});
mount(App,{target:document.getElementById("app")!});
st.settingsTab=externalPrivate ? "automation" : "profiles"; st.settingsOpen=!externalPrivate;
