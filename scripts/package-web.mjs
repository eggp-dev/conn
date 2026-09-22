// Build a local, self-contained Conn web host. Nothing is uploaded or published.
import {spawn,execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {cpSync,readFileSync,writeFileSync,mkdirSync,mkdtempSync,readdirSync,statSync,existsSync} from 'node:fs';
import {dirname,resolve,relative,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {tmpdir} from 'node:os';
import assert from 'node:assert/strict';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const npm=process.platform==='win32'?'npm.cmd':'npm';
const executable=process.platform==='win32'?'.exe':'';
const debug=process.argv.includes('--debug');
const profile=debug?'debug':'release';
const version=JSON.parse(readFileSync(join(root,'frontends/web/package.json'),'utf8')).version;
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
function run(program,args){execFileSync(program,args,{cwd:root,stdio:'inherit'});}
function sourceIdentity(){
 const paths=execFileSync('git',['ls-files','--cached','--others','--exclude-standard','-z'],{cwd:root,encoding:'utf8'}).split('\0').filter(p=>p&&existsSync(join(root,p))&&statSync(join(root,p)).isFile()&&(/^(Cargo\.(toml|lock)|package(-lock)?\.json)$/.test(p)||/^(crates|vendor|packages\/ui|frontends\/web|scripts\/package-web)/.test(p))).sort();
 const hash=createHash('sha256');for(const path of paths)hash.update(path+'\0').update(readFileSync(join(root,path))).update('\0');return hash.digest('hex');
}
const source=sourceIdentity();
run(npm,['run','build','-w','@conn/web']);
run('cargo',['build','--locked',...(debug?[]:['--release']),'-p','conn-web','-p','conn']);
assert.equal(sourceIdentity(),source,'Sources changed during packaging; rerun to build a coherent UI/server/MCP bundle.');
const base=join(root,'release-artifacts');mkdirSync(base,{recursive:true});
const bundle=mkdtempSync(join(base,`conn-web-${version}-${process.platform}-${process.arch}${debug?'-debug':''}-`));
for(const name of ['conn-web','conn'])cpSync(join(root,'target',profile,name+executable),join(bundle,name+executable));
cpSync(join(root,'frontends/web/dist'),join(bundle,'ui'),{recursive:true});
const contract=JSON.parse(execFileSync(join(bundle,'conn-web'+executable),['contract'],{encoding:'utf8'}));
for(const name of ['conn-web','conn'])assert.match(execFileSync(join(bundle,name+executable),['--version'],{encoding:'utf8'}),new RegExp(`^${name} ${version.replaceAll('.','\\.')}\\s*$`));
const manifest={package:'conn-web',version,protocol:contract.protocol,platform:process.platform,architecture:process.arch,profile,sourceCommit:execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),sourceDirty:execFileSync('git',['status','--porcelain'],{cwd:root,encoding:'utf8'}).trim().length>0,sourceDigest:source,builtAt:new Date().toISOString(),runtime:{server:'conn-web'+executable,agent:'conn'+executable,ui:'ui'},scope:'Loopback local owner; standalone web runtime. No remote hosting or desktop session mirroring.'};
writeFileSync(join(bundle,'manifest.json'),JSON.stringify(manifest,null,2)+'\n');
writeFileSync(join(bundle,'README.md'),`# Conn web ${version}\n\nThis ${process.platform}/${process.arch} bundle contains the server, the same-build MCP CLI, and the compiled common UI. Running it does not require Node, Vite, Rust, or a checkout. ${debug?'This is a debug verification bundle, not a release build.':''}\n\nFrom this directory:\n\n\`\`\`sh\n./conn-web${executable} serve --state-dir /path/to/your/conn-web-state\n\`\`\`\n\nThe default address is http://127.0.0.1:1423/. Open it and enter the \`bootstrapToken\` from your private state directory's \`connection.json\`. Keep that file private. The server binds only to loopback; this package does not enable remote access. The default UI directory is \`ui/\` beside the executable.\n\nUse the app's explicit agent setup flow, or configure a persistent MCP process:\n\n\`\`\`sh\n/path/to/this/bundle/conn${executable} --socket /path/to/your/conn-web-state/conn.sock mcp --agent-id your-agent\n\`\`\`\n\nReloading the page preserves the running shell while this server process stays alive. Another owner page requires an explicit handoff. Closing a shell or stopping the server ends its process. Restarting the server does not restore old PTYs, SSH sessions, or permissions. The shell uses the account running this server; a separate state directory is not a filesystem sandbox. Desktop settings and sockets are separate by default.\n\n\`manifest.json\` records the build and source identity. \`SHA256SUMS\` covers the packaged files. Packaging verifies binary versions and static HTTP routes; native macOS UI/IME and your real remote environment require separate acceptance.\n`);
function files(dir){return readdirSync(dir,{withFileTypes:true}).flatMap(entry=>entry.isDirectory()?files(join(dir,entry.name)):[join(dir,entry.name)]);}
writeFileSync(join(bundle,'SHA256SUMS'),files(bundle).sort().map(path=>`${digest(readFileSync(path))}  ${relative(bundle,path).replaceAll('\\','/')}`).join('\n')+'\n');
const state=mkdtempSync(join(tmpdir(),'conn-web-package-check-'));
const server=spawn(join(bundle,'conn-web'+executable),['serve','--port','0','--state-dir',state],{cwd:bundle,stdio:['ignore','ignore','pipe']});let errors='';server.stderr.on('data',bytes=>{errors+=bytes.toString();});
try{
 let metadata;for(let i=0;i<100;i++){if(existsSync(join(state,'connection.json'))){metadata=JSON.parse(readFileSync(join(state,'connection.json'),'utf8'));break;}if(server.exitCode!==null)throw Error(errors||'Packaged server exited');await new Promise(resolve=>setTimeout(resolve,50));}
 assert.ok(metadata,'Packaged server did not become ready');assert.equal(metadata.buildVersion,version);assert.equal(metadata.protocol,contract.protocol);
 const html=await fetch(metadata.url);assert.equal(html.status,200);const content=await html.text();assert.match(content,/type="module"/);
 const assets=[...content.matchAll(/(?:src|href)="([^"#]+\.(?:js|css))"/g)].map(match=>match[1]);assert.ok(assets.length,'No built assets referenced');
 for(const asset of assets){const response=await fetch(new URL(asset,metadata.url));assert.equal(response.status,200,asset);assert.ok((await response.arrayBuffer()).byteLength,asset);}
 const info=await(await fetch(new URL('/api/info',metadata.url))).json();assert.equal(info.buildVersion,version);assert.equal(info.authenticated,false);
 writeFileSync(join(base,`${relative(base,bundle)}-verification.json`),JSON.stringify({bundle,version,profile,staticRoutes:'passed',assets:assets.length,cliVersion:'passed',serverVersion:'passed',protocol:contract.protocol,requiresBuildToolsAtRuntime:false},null,2)+'\n');
 console.log(`Packaged and verified: ${bundle}`);
}finally{server.kill('SIGTERM');await Promise.race([new Promise(resolve=>server.once('exit',resolve)),new Promise(resolve=>setTimeout(()=>{if(server.exitCode===null)server.kill('SIGKILL');resolve();},3000))]);}
