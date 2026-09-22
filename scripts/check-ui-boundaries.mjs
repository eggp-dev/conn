// Prevent a second product UI or a host-specific branch in the common package.
import { readdirSync, readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join, relative } from 'node:path';
import assert from 'node:assert/strict';
const root = fileURLToPath(new URL('../', import.meta.url));
const walk = directory => readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
  const path = join(directory, entry.name);
  return entry.isDirectory() ? walk(path) : /\.(ts|js|svelte)$/.test(path) ? [path] : [];
});
for (const path of walk(join(root, 'packages/ui/src'))) {
  const source = readFileSync(path, 'utf8'), name = relative(root, path);
  assert.ok(!/@tauri-apps|__TAURI|import\.meta\.env\.MODE|browser-test|CONN_TEST_/.test(source), `Host dependency in ${name}`);
  assert.ok(!/(?:from|import\s*\()\s*['"][^'"]*frontends\//.test(source), `Reverse frontend import in ${name}`);
}
for (const frontend of ['tauri', 'web']) {
  const files = walk(join(root, `frontends/${frontend}/src`));
  assert.ok(files.length <= 6, `${frontend} entry must remain a small host adapter`);
  assert.ok(files.some(path => /ConnApp/.test(readFileSync(path, 'utf8'))), `${frontend} must mount the common ConnApp`);
}
for (const legacy of ['crates/browser-harness', 'frontends/tauri/tests/browser', 'frontends/tauri/scripts/browser-test.mjs', 'frontends/tauri/src/components']) {
  assert.ok(!existsSync(join(root, legacy)), `Legacy alternate runtime remains: ${legacy}`);
}
console.log('PASS common UI imports, thin host entries and removal of the alternate browser runtime');
