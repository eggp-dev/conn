export const RELEASES = 'https://github.com/eggplantiny/conn/releases/';
export type Platform = { version: string; os: string; arch: string };
export type Release = { tag: string; notes: string; preview: boolean; url: string; download?: string };
function version(value: string) {
  const m = /^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/.exec(value);
  return m ? { core: m.slice(1, 4).map(Number), pre: m[4]?.split('.') } : null;
}
export function compareVersions(a: string, b: string): number {
  const x = version(a), y = version(b);
  if (!x || !y) throw new Error('Invalid version');
  for (let i = 0; i < 3; i++) if (x.core[i] !== y.core[i]) return x.core[i] > y.core[i] ? 1 : -1;
  if (!x.pre || !y.pre) return x.pre ? -1 : y.pre ? 1 : 0;
  for (let i = 0; i < Math.max(x.pre.length, y.pre.length); i++) {
    const l = x.pre[i], r = y.pre[i];
    if (l === r) continue;
    if (l === undefined) return -1;
    if (r === undefined) return 1;
    const ln = /^\d+$/.test(l), rn = /^\d+$/.test(r);
    if (ln && rn) return BigInt(l) > BigInt(r) ? 1 : -1;
    if (ln !== rn) return ln ? -1 : 1;
    return l > r ? 1 : -1;
  }
  return 0;
}
export function selectRelease(data: unknown, platform: Platform, previews: boolean): Release | null {
  if (!Array.isArray(data)) throw new Error('Invalid release response');
  const target = ({ macos: { aarch64: 'aarch64-apple-darwin-desktop.dmg' }, windows: { x86_64: 'x86_64-pc-windows-msvc-setup.exe' }, linux: { x86_64: 'x86_64-unknown-linux-gnu-desktop.AppImage' } } as Record<string, Record<string, string>>)[platform.os]?.[platform.arch];
  const releases = data.filter(r => r && r.draft === false && typeof r.prerelease === 'boolean' && (previews || !r.prerelease) && typeof r.tag_name === 'string' && version(r.tag_name));
  releases.sort((a, b) => compareVersions(b.tag_name, a.tag_name));
  const r = releases.find(r => compareVersions(r.tag_name, platform.version) > 0);
  if (!r) return null;
  const name = target ? `conn-${r.tag_name}-${target}` : undefined;
  const expected = name ? `${RELEASES}download/${r.tag_name}/${name}` : undefined;
  const asset = Array.isArray(r.assets) && r.assets.find((a: any) => a.name === name && a.browser_download_url === expected && a.state === 'uploaded');
  return { tag: r.tag_name, notes: typeof r.body === 'string' ? r.body : '', preview: r.prerelease, url: `${RELEASES}tag/${r.tag_name}`, download: asset ? expected : undefined };
}
export async function checkRelease(platform: Platform, previews: boolean, signal: AbortSignal): Promise<Release | null> {
  // Public, credential-free lookup. Listing includes previews, unlike /latest.
  const response = await fetch('https://api.github.com/repos/eggplantiny/conn/releases?per_page=100', { signal, credentials: 'omit', headers: { Accept: 'application/vnd.github+json' } });
  if (!response.ok) throw new Error(response.status === 403 || response.status === 429 ? 'rate' : 'network');
  return selectRelease(await response.json(), platform, previews);
}
