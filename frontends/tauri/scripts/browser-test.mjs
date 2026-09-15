import { mkdtemp, access } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawn } from "node:child_process";
const state=process.env.CONN_TEST_STATE ?? await mkdtemp(join(tmpdir(),"conn-browser-test-"));
const children=[];
const run=(command,args,options={})=>{const child=spawn(command,args,{stdio:"inherit",...options});children.push(child);return child;};
const build=run("cargo",["build","-p","conn-browser-harness","--locked"],{cwd:resolve("../.."),env:{...process.env,CARGO_INCREMENTAL:"0",CARGO_PROFILE_DEV_DEBUG:"0"}});
await new Promise((resolve,reject)=>{build.on("error",reject);build.on("exit",code=>code===0?resolve():reject(new Error(`Harness build failed: ${code}`)));});
const shellEnv={...process.env};
delete shellEnv.npm_config_prefix;
const server=run(resolve(`../../target/debug/conn-browser-harness${process.platform==="win32"?".exe":""}`),[state],{env:shellEnv});
for(let i=0;;i++){try{await access(join(state,"connection.json"));break;}catch{if(i>100||server.exitCode!==null)throw new Error("Harness did not start");await new Promise(r=>setTimeout(r,100));}}
const vite=run(process.execPath,["node_modules/vite/bin/vite.js","--mode","browser-test","--host","127.0.0.1","--port","1421"],{env:{...process.env,CONN_TEST_STATE:state}});
console.log(`Test state: ${state}\nBrowser: http://127.0.0.1:1421/`);
let stopping=false;
function stop(){if(stopping)return;stopping=true;for(const c of children)c.kill("SIGTERM");}
process.on("SIGINT",stop);process.on("SIGTERM",stop);
vite.on("exit",stop);server.on("exit",stop);
