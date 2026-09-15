// Build the CLI sidecar for this host before launching or packaging the desktop.
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { mkdirSync, copyFileSync } from "node:fs";
import { join } from "node:path";
const frontend=fileURLToPath(new URL("../",import.meta.url));
const root=fileURLToPath(new URL("../../../",import.meta.url));
function run(command,args,options={}) {
  const result=spawnSync(command,args,{cwd:root,stdio:"inherit",...options});
  if(result.error)throw result.error;
  if(result.status!==0)process.exit(result.status ?? 1);
  return result;
}
const args=process.argv.slice(2);
if(args[0]==="dev" || args[0]==="build") {
  const rust=run("rustc",["-vV"],{encoding:"utf8",stdio:["ignore","pipe","inherit"]}).stdout;
  const host=rust.match(/^host: (.+)$/m)?.[1];
  if(!host)throw new Error("Could not detect the Rust host target");
  const targetIndex=args.indexOf("--target");
  const target=(targetIndex>=0 ? args[targetIndex+1] : args.find(a=>a.startsWith("--target="))?.slice(9)) ?? host;
  if(target==="universal-apple-darwin")throw new Error("Build each Apple target separately; universal sidecar merging is not configured");
  const debug=args[0]==="dev" || args.includes("--debug");
  const profile=debug ? "debug" : "release";
  run("cargo",["build","--locked",...(debug ? [] : ["--release"]),"-p","conn","--target",target]);
  const metadata=JSON.parse(run("cargo",["metadata","--no-deps","--format-version","1"],{encoding:"utf8",stdio:["ignore","pipe","inherit"]}).stdout);
  const extension=target.includes("windows") ? ".exe" : "";
  const binaries=join(frontend,"src-tauri","binaries");mkdirSync(binaries,{recursive:true});
  copyFileSync(join(metadata.target_directory,target,profile,`conn${extension}`),join(binaries,`conn-${target}${extension}`));
}
run(process.execPath,[join(frontend,"node_modules","@tauri-apps","cli","tauri.js"),...args],{cwd:frontend});
