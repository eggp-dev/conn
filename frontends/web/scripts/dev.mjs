import { spawn, spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const port = process.env.CONN_WEB_PORT ?? '1429';
const backend = process.env.CONN_WEB_BACKEND_PORT ?? '1430';
const origin = `http://127.0.0.1:${port}`;
const state = process.env.CONN_WEB_STATE ?? join(process.env.XDG_CONFIG_HOME ?? join(homedir(), '.config'), 'conn-web-dev');
for (const p of [port, backend]) {
  try { await fetch(`http://127.0.0.1:${p}/api/info`, { signal: AbortSignal.timeout(500) }); throw new Error(`Port ${p} is already serving. Reuse that host or select another port.`); }
  catch (error) { if (!['TimeoutError', 'TypeError'].includes(error.name)) throw error; }
}
const build = spawnSync('cargo', ['build', '--locked', '-p', 'conn-web', '-p', 'conn'], { cwd: root, stdio: 'inherit' });
if (build.status !== 0) process.exit(build.status ?? 1);
const executable = resolve(root, 'target/debug/conn-web' + (process.platform === 'win32' ? '.exe' : ''));
const args = ['serve', '--state-dir', state, '--port', backend, '--dev-origin', origin];
if (process.env.CONN_WEB_SETUP_HOME) args.push('--setup-home', process.env.CONN_WEB_SETUP_HOME);
const hostEnv = { ...process.env };
// npm's launcher setting is not a shell configuration; it breaks nvm startup.
delete hostEnv.npm_config_prefix;
const server = spawn(executable, args, { cwd: root, env: hostEnv, stdio: 'inherit' });
let vite, stopping = false;
const stop = () => { if (stopping) return; stopping = true; vite?.kill(); server.kill(); };
process.on('SIGINT', stop); process.on('SIGTERM', stop);
server.on('exit', code => { stop(); process.exitCode = code ?? 1; });
try {
  const connection = join(state, 'connection.json');
  for (let attempt = 0; ; attempt++) {
    try { if (existsSync(connection) && (await fetch(`http://127.0.0.1:${backend}/api/info`)).ok) break; } catch {}
    if (attempt >= 100 || server.exitCode !== null) throw new Error('Conn web host did not start.');
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  const metadata = JSON.parse(readFileSync(connection, 'utf8'));
  if (metadata.protocol !== 1) throw new Error('Conn host protocol mismatch.');
  vite = spawn(process.execPath, [fileURLToPath(new URL('../node_modules/vite/bin/vite.js', import.meta.url)), '--host', '127.0.0.1', '--port', port], {
    cwd: resolve(root, 'frontends/web'), env: { ...process.env, CONN_WEB_BACKEND: `http://127.0.0.1:${backend}` }, stdio: 'inherit',
  });
  vite.on('exit', code => { stop(); process.exitCode = code ?? 1; });
  console.log(`Open ${origin}. Connection code: bootstrapToken in ${connection}`);
} catch (error) { console.error(error.message); stop(); process.exitCode = 1; }
