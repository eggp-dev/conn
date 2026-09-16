import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compareVersions, selectRelease, checkRelease, RELEASES } from '../../src/lib/updates.ts';
const platform = { version: '0.4.0', os: 'macos', arch: 'aarch64' };
const release = (tag = 'v0.5.0', preview = true, suffix = 'aarch64-apple-darwin-desktop.dmg') => ({ tag_name: tag, draft: false, prerelease: preview, body: 'Changes', assets: [{ name: `conn-${tag}-${suffix}`, state: 'uploaded', browser_download_url: `${RELEASES}download/${tag}/conn-${tag}-${suffix}` }] });
test('semantic versions, prerelease ordering and no downgrade', () => {
 assert.ok(compareVersions('0.10.0', '0.9.0') > 0);
 assert.ok(compareVersions('1.0.0-rc.10', '1.0.0-rc.2') > 0);
 assert.ok(compareVersions('1.0.0', '1.0.0-rc.10') > 0);
 assert.equal(compareVersions('v1.0.0+build', '1.0.0'), 0);
 assert.equal(selectRelease([release('v0.4.0'), release('v0.3.0')], platform, true), null);
});
test('preview channel, drafts and malformed tags', () => {
 const data = [release(), release('v0.4.1', false), { ...release('v9.0.0'), draft: true }, release('nonsense')];
 assert.equal(selectRelease(data, platform, false)?.tag, 'v0.4.1');
 assert.equal(selectRelease(data, platform, true)?.tag, 'v0.5.0');
 assert.throws(() => selectRelease({}, platform, true));
});
test('exact platform assets and safe fallback', () => {
 for (const [os, arch, suffix] of [['macos','aarch64','aarch64-apple-darwin-desktop.dmg'],['windows','x86_64','x86_64-pc-windows-msvc-setup.exe'],['linux','x86_64','x86_64-unknown-linux-gnu-desktop.AppImage']]) {
  assert.ok(selectRelease([release('v0.5.0', true, suffix)], { ...platform, os, arch }, true)?.download?.endsWith(suffix));
 }
 assert.equal(selectRelease([release()], {...platform, arch:'unknown'}, true)?.download, undefined);
 // Paused Intel builds must never be offered the Apple Silicon installer,
 // even if an old Intel asset is present in the response.
 for (const suffix of ['aarch64-apple-darwin-desktop.dmg', 'x86_64-apple-darwin-desktop.dmg']) {
  const intel = selectRelease([release('v0.5.0', true, suffix)], {...platform, arch:'x86_64'}, true);
  assert.equal(intel?.download, undefined);
  assert.equal(intel?.url, `${RELEASES}tag/v0.5.0`);
 }
 const malicious = release(); malicious.assets[0].browser_download_url = 'https://evil.test/install';
 assert.equal(selectRelease([malicious], platform, true)?.download, undefined);
});
test('fetch handles success, rate limits and invalid response', async () => {
 const original = globalThis.fetch;
 try {
  globalThis.fetch = async () => new Response(JSON.stringify([release()]));
  assert.equal((await checkRelease(platform,true,new AbortController().signal))?.tag, 'v0.5.0');
  globalThis.fetch = async () => new Response('',{status:429});
  await assert.rejects(checkRelease(platform,true,new AbortController().signal), /rate/);
  globalThis.fetch = async () => new Response('{}');
  await assert.rejects(checkRelease(platform,true,new AbortController().signal), /Invalid release/);
 } finally { globalThis.fetch=original; }
});
