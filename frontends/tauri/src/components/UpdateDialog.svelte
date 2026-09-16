<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '../lib/transport';
  import { t } from '../lib/i18n.svelte';
  import { checkRelease, RELEASES, type Platform, type Release } from '../lib/updates';
  let { onclose }: { onclose: () => void } = $props();
  let dialog: HTMLDialogElement;
  let platform = $state<Platform>();
  let release = $state<Release | null>(null);
  let phase = $state<'loading' | 'ready' | 'error'>('loading');
  let error = $state('network');
  let opening = $state(false);
  let openFailed = $state(false);
  let previews = $state(true);
  let request: AbortController | undefined;
  try { previews = localStorage.getItem('conn:update-previews') !== 'false'; } catch {}
  async function check() {
    request?.abort();
    const current = new AbortController(); request = current;
    phase = 'loading'; release = null; openFailed = false;
    const timer = setTimeout(() => current.abort(), 15000);
    try {
      const info = await invoke<Platform>('update_info');
      if (current !== request) return;
      if (current.signal.aborted) throw new Error("timeout");
      platform = info;
      const result = await checkRelease(info, previews, current.signal);
      if (current !== request) return;
      release = result; phase = 'ready';
    } catch (e) {
      if (current !== request) return;
      error = e instanceof Error && e.message === 'rate' ? 'rate' : 'network'; phase = 'error';
    } finally { clearTimeout(timer); }
  }
  async function open(url: string) {
    opening = true; openFailed = false;
    try { await invoke('open_release', { url }); } catch { openFailed = true; }
    finally { opening = false; }
  }
  onMount(() => { dialog.showModal(); void check(); return () => { request?.abort(); request = undefined; }; });
</script>

<dialog bind:this={dialog} aria-labelledby="update-title" onkeydown={(e) => e.stopPropagation()} onclose={onclose} oncancel={onclose}>
  <header><h2 id="update-title">{t('update.title')}</h2><button aria-label={t('close')} onclick={onclose}>×</button></header>
  <p class="muted">{t('update.current', { version: platform?.version ?? '—' })}</p>
  <label><input type="checkbox" bind:checked={previews} onchange={() => { try { localStorage.setItem('conn:update-previews', String(previews)); } catch {} void check(); }} />{t('update.previews')}</label>
  <section aria-live="polite" aria-busy={phase === 'loading'}>
    {#if phase === 'loading'}<h3>{t('update.loading')}</h3>
    {:else if phase === 'error'}<h3>{t('update.failed')}</h3><p>{t(`update.${error}`)}</p>
    {:else if release}<h3>{t('update.available', { version: release.tag })}{release.preview ? ` · ${t('update.preview')}` : ''}</h3>
      <details><summary>{t('update.notes')}</summary><pre>{release.notes || t('update.noNotes')}</pre></details>
      <p class="muted">{t('update.install')}</p>
      {#if release.download}<button disabled={opening} onclick={() => open(release!.url)}>{t('update.releases')}</button>{/if}
    {:else}<h3>{t('update.latest')}</h3><p>{t('update.channel')}</p>{/if}
  </section>
  {#if openFailed}<p role="alert">{t('update.openFailed')} <code>{release?.download ?? release?.url ?? RELEASES}</code></p>{/if}
  <footer>
    <button onclick={check} disabled={phase === 'loading'}>{t('update.retry')}</button>
    <button class="primary" disabled={opening} onclick={() => open(release?.download ?? release?.url ?? RELEASES)}>{t(release?.download ? 'update.download' : 'update.releases')}</button>
  </footer>
</dialog>

<style>
  dialog { width: min(480px, calc(100vw - 48px)); max-height: calc(100dvh - 64px); overflow: auto; padding: 24px; border: 1px solid var(--line); border-radius: 16px; background: var(--surface); color: var(--fg); box-shadow: var(--shadow); }
  dialog::backdrop { background: #0008; }
  header, footer { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { font-size: 18px; margin: 0; } h3 { font-size: 15px; line-height: 1.5; }
  p, label, summary { font-size: 13px; line-height: 1.65; } .muted { color: var(--muted); }
  label { display: flex; align-items: center; gap: 8px; } input { accent-color: var(--agent); }
  section { margin: 20px 0; min-height: 85px; } summary { cursor: pointer; }
  pre { white-space: pre-wrap; overflow-wrap: anywhere; max-height: 260px; overflow: auto; padding: 12px; background: var(--bg); font-size: 12px; line-height: 1.6; }
  code { display: block; overflow-wrap: anywhere; user-select: text; }
  button { border: 1px solid var(--line); border-radius: 7px; padding: 8px 12px; color: var(--fg); background: var(--surface2); cursor: pointer; }
  button:disabled { opacity: .5; cursor: default; } button:focus-visible, summary:focus-visible { outline: 2px solid var(--agent); outline-offset: 3px; }
  .primary { background: var(--agent); color: #101018; } footer { flex-wrap: wrap; }
</style>
