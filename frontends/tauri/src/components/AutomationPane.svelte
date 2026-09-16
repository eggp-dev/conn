<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '../lib/transport';
  import { t } from '../lib/i18n.svelte';
  import { toast } from '../lib/store.svelte';
  type Config = { enabled: boolean; profiles: string[]; linuxExecutables: string[] };
  type Info = { config: Config; nativeSupported: boolean; linuxSupported: boolean; requiresReenable: boolean };
  let info = $state<Info | null>(null);
  let profiles = $state<{id: string; name: string}[]>([]);
  let busy = $state(false);
  let error = $state('');
  let alive = false;
  let revision = 0;
  async function load() {
    if (busy) return;
    const current = ++revision;
    try {
      const [next, catalog] = await Promise.all([invoke<Info>('automation_settings'), invoke<{config: {profiles: typeof profiles}}>('profiles_catalog')]);
      if (!alive || current !== revision || busy) return;
      info = next; profiles = catalog.config.profiles; error = '';
    } catch (e) { if (alive && current === revision && !busy) error = String(e); }
  }
  onMount(() => { alive = true; load(); const timer = setInterval(load, 3000); return () => { alive = false; revision++; clearInterval(timer); }; });
  async function save(config: Config) {
    if (busy) return;
    const current = ++revision;
    busy = true;
    try {
      const next = await invoke<Info>('automation_save', {config});
      if (alive && current === revision) { info = next; error = ''; toast(t('automation.saved'), 'ok'); }
    }
    catch (e) { if (alive && current === revision) toast(String(e), 'danger'); }
    finally { if (alive && current === revision) busy = false; }
  }
  function toggle(event: Event) {
    if (!info) return;
    const input = event.currentTarget as HTMLInputElement;
    const enabled = input.checked;
    input.checked = info.config.enabled;
    void save({...info.config, enabled});
  }
  function profile(id: string, event: Event) {
    if (!info) return;
    const input = event.currentTarget as HTMLInputElement;
    const enabled = input.checked;
    input.checked = info.config.profiles.includes(id);
    save({...info.config, profiles: enabled ? [...info.config.profiles, id] : info.config.profiles.filter(p => p !== id)});
  }
  async function revoke() {
    if (busy) return;
    const current = ++revision;
    busy = true;
    try {
      await invoke('automation_revoke');
      const next = await invoke<Info>('automation_settings');
      if (alive && current === revision) { info = next; error = ''; toast(t('automation.revoked'), 'ok'); }
    } catch (e) { if (alive && current === revision) toast(String(e), 'danger'); }
    finally { if (alive && current === revision) busy = false; }
  }
</script>

<div class="automation">
  <h2>{t('automation.title')}</h2>
  <p>{t('automation.description')}</p>
  {#if error}<p role="alert">{error}</p>{/if}
  {#if info}
    {#if !info.nativeSupported}<p class="notice">{t('automation.macos')}</p>{/if}
    {#if info.requiresReenable}<p class="notice" role="status">{t('automation.reenable')}</p>{/if}
    <label class="toggle"><input type="checkbox" checked={info.config.enabled} disabled={busy || !info.nativeSupported} onchange={toggle} />{t('automation.enable')}</label>
    <p class="hint">{t('automation.permission')}</p>
    {#if info.linuxSupported}
      <label for="linux-callers">{t('automation.callers')}</label>
      <textarea id="linux-callers" rows="3" disabled={busy} value={info.config.linuxExecutables.join('\n')} onchange={e => info && save({...info.config, linuxExecutables: e.currentTarget.value.split('\n').map(p => p.trim()).filter(Boolean)})}></textarea>
      <p class="hint">{t('automation.callersHint')}</p>
    {/if}
    <h3>{t('automation.profiles')}</h3>
    {#each profiles as item}
      <label class="profile"><input type="checkbox" checked={info.config.profiles.includes(item.id)} disabled={busy || !info.nativeSupported} onchange={e => profile(item.id, e)} /><span>{item.name}<small>{item.id}</small></span></label>
    {/each}
    <p class="hint">{t('automation.scope')}</p>
    <button class="revoke" disabled={busy} onclick={revoke}>{t('automation.revoke')}</button>
  {/if}
</div>

<style>
  .automation { display: grid; gap: 14px; min-width: 0; }
  h2,h3,p { margin: 0; } h2 { font-size: 16px; } h3 { margin-top: 8px; font-size: 12px; color: var(--muted); }
  p { line-height: 1.6; overflow-wrap: anywhere; }
  .hint { font-size: 12px; color: var(--muted); }
  .notice { padding: 12px; border: 1px solid var(--line); border-radius: 10px; }
  label { display: flex; gap: 10px; align-items: center; }
  .profile { padding: 9px 0; } small { display: block; color: var(--muted); margin-top: 3px; }
  input { accent-color: var(--agent); } .revoke { justify-self: start; padding: 8px 12px; border: 1px solid var(--line); border-radius: 8px; background: var(--surface); color: var(--fg); cursor: pointer; }
  .revoke:disabled { cursor: default; opacity: .5; }
  .revoke:focus-visible, input:focus-visible { outline: 2px solid var(--agent); outline-offset: 3px; }
  textarea { width: 100%; box-sizing: border-box; resize: vertical; padding: 10px; border: 1px solid var(--line); border-radius: 8px; background: var(--surface); color: var(--fg); font: inherit; }
  textarea:focus-visible { outline: 2px solid var(--agent); outline-offset: 3px; }
  .profile span { min-width: 0; overflow-wrap: anywhere; }
</style>
