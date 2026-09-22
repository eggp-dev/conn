<script lang="ts">
  import { useConnApp } from '../runtime/context';
  const app = useConnApp();
  const { invoke, t, updater, initUpdates, updateAction, updatePreference, updateCapability } = app;

  import { onMount } from 'svelte';
  import { RELEASES } from '../lib/updates';
  let { onclose }: { onclose: () => void } = $props();
  let dialog: HTMLDialogElement;
  let confirm = $state(false);
  let openFailed = $state(false);
  const s = $derived(updater.status);
  const progress = $derived(s.total ? Math.min(100, Math.round(s.downloaded / s.total * 100)) : undefined);
  async function open() { try { await app.openExternal(s.version ? `${RELEASES}tag/v${s.version}` : RELEASES); } catch { openFailed = true; } }
  onMount(() => { dialog.showModal(); void initUpdates().then(() => { if (['idle','current','error'].includes(s.phase)) void updateAction('check'); }); });
</script>

<dialog bind:this={dialog} class="palette-surface" aria-labelledby="update-title" onkeydown={(e) => e.stopPropagation()} onclose={onclose} oncancel={onclose}>
  <header><h2 id="update-title">{t('update.title')}</h2><button aria-label={t('close')} onclick={onclose}>×</button></header>
  <p class="muted">{t('update.current', { version: s.current || '—' })}</p>
  <section aria-live="polite" aria-busy={updater.busy}>
    {#if confirm}
      <h3>{t('update.restartTitle')}</h3><p>{t('update.restartWarning')}</p>
    {:else if s.phase === 'unsupported'}<h3>{t('update.hostManaged')}</h3><p class="muted">{updateCapability.reason || t('update.hostManagedHint')}</p>
    {:else if s.phase === 'checking'}<h3>{t('update.loading')}</h3>
    {:else if s.phase === 'error'}<h3>{t('update.failed')}</h3><p>{t('update.unchanged')}</p><details><summary>{t('s.diag.details')}</summary><p class="error">{s.error}</p></details>
    {:else if s.phase === 'downloading'}<h3>{t('update.downloading', { progress: progress === undefined ? '…' : `${progress}%` })}</h3><progress max="100" value={progress} aria-label={t('update.download')} />
    {:else if s.phase === 'ready'}<h3>{t('update.ready', { version: s.version ?? '' })}</h3><p class="muted">{t('update.verified')}</p>
    {:else if s.phase === 'installing'}<h3>{t('update.installing')}</h3>
    {:else if s.version}<h3>{t('update.available', { version: s.version })}{s.preview ? ` · ${t('update.preview')}` : ''}</h3>
      {#if !s.supported}<p class="muted">{t('update.manual')}</p>{/if}
    {:else}<h3>{t('update.latest')}</h3><p class="muted">{t('update.channel')}</p>{/if}
    {#if s.notes && !confirm}<details><summary>{t('update.notes')}</summary><pre>{s.notes}</pre></details>{/if}
  </section>
  {#if !confirm && updateCapability.supported}
    <details class="preferences"><summary>{t('update.preferences')}</summary>
      <label><input type="checkbox" disabled={updater.busy} checked={updater.automatic} onchange={e => updatePreference('automatic', e.currentTarget.checked)} />{t(s.supported ? 'update.automatic' : 'update.autoCheck')}</label>
      <label><input type="checkbox" disabled={updater.busy} checked={updater.previews} onchange={e => { updatePreference('previews', e.currentTarget.checked); void updateAction('check'); }} />{t('update.previews')}</label>
    </details>
  {/if}
  {#if openFailed}<p role="alert">{t('update.openFailed')}</p>{/if}
  <footer>
    {#if confirm}
      <button onclick={() => confirm = false} disabled={updater.busy}>{t('cancel')}</button>
      <button class="primary" disabled={updater.busy} onclick={() => updateAction('install', true)}>{t('update.restart')}</button>
    {:else}
      <button onclick={() => updateAction('check')} disabled={!updateCapability.supported || updater.busy || s.phase === 'ready'}>{t('update.retry')}</button>
      {#if s.phase === 'ready'}<button class="primary" onclick={() => confirm = true}>{t('update.restart')}</button>
      {:else if s.supported && s.version && s.phase !== 'error'}<button class="primary" disabled={updater.busy} onclick={() => updateAction('download')}>{t('update.download')}</button>
      {:else}<button class="primary" onclick={open}>{t('update.releases')}</button>{/if}
    {/if}
  </footer>
</dialog>

<style>
  dialog { width: min(480px, calc(100vw - 48px)); max-height: calc(100dvh - 64px); overflow: auto; padding: 24px; border: 1px solid var(--line); border-radius: 16px; background: var(--surface); color: var(--fg); box-shadow: var(--shadow); }
  dialog::backdrop { background: #0008; }
  header, footer { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { font-size: 18px; margin: 0; } h3 { font-size: 15px; line-height: 1.5; }
  p, label, summary { font-size: 13px; line-height: 1.65; } .muted { color: var(--muted); }
  label { display: flex; align-items: center; gap: 8px; margin: 10px 0; } input, progress { accent-color: var(--agent); } progress { width: 100%; }
  section { margin: 20px 0; min-height: 85px; } summary { cursor: pointer; }
  pre { white-space: pre-wrap; overflow-wrap: anywhere; max-height: 260px; overflow: auto; padding: 12px; background: var(--bg); font-size: 12px; line-height: 1.6; }
  .error { overflow-wrap: anywhere; } .preferences { margin-bottom: 20px; color: var(--muted); }
  button { border: 1px solid var(--line); border-radius: 7px; padding: 8px 12px; color: var(--fg); background: var(--surface2); cursor: pointer; }
  button:disabled { opacity: .5; cursor: default; } button:focus-visible, summary:focus-visible { outline: 2px solid var(--agent); outline-offset: 3px; }
  .primary { background: var(--agent); color: #101018; } footer { flex-wrap: wrap; }
</style>
