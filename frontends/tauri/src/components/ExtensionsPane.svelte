<script lang="ts">
 import { onMount } from 'svelte';
 import { cmd } from '../lib/bridge';
 import { extensions, refreshExtensions, selectTheme } from '../lib/extensions.svelte';
 import { t } from '../lib/i18n.svelte';
 import { toast } from '../lib/store.svelte';
 let loading = $state(true);
 let busy = $state(false);
 let error = $state('');
 let key = $state('');
 let model = $state('');
 let themeFile:HTMLInputElement;
 const catalog = $derived(extensions.catalog);
 onMount(() => { void (async () => { try { model = (await refreshExtensions()).settings.model; } catch { error = t('ext.loadFailed'); } finally { loading = false; } })(); return () => { key = ''; }; });
 async function configure(id:string, config:Record<string,unknown>) {
  busy=true;error='';
  try { await cmd('extension_configure',{id,config}); await refreshExtensions(); }
  catch { error=t('ext.saveFailed'); }
  finally {busy=false;}
 }
 async function installTheme(file:File|undefined) {
  if(!file)return;busy=true;error='';
  try {if(file.size>65536)throw new Error('Too large');const manifest=JSON.parse(await file.text());await cmd('extension_install_theme',{manifest});await refreshExtensions();}
  catch {error=t('ext.themeFailed');}
  finally {busy=false;if(themeFile)themeFile.value='';}
 }
 async function saveKey() {
  busy=true;error='';
  // This is write-only: no read API and no saved key returned into the webview.
  const value=key;key='';
  try {await cmd('extension_set_key',{provider:'openai',key:value});await refreshExtensions();toast(t('ext.keySaved'),'ok');}
  catch {error=t('ext.keyFailed');}
  finally {busy=false;}
 }
 async function deleteKey() {
  busy=true;error='';
  try {await cmd('extension_delete_key',{provider:'openai'});await refreshExtensions();}
  catch {error=t('ext.keyFailed');}
  finally {busy=false;}
 }
</script>
{#if loading}<p class="muted">{t('ext.loading')}</p>
{:else if catalog}
<section class="extension">
 <header><div><h3>{t('ext.theme')}</h3><p class="muted">{t('ext.themeHint')}</p></div></header>
 <label class="field"><span>{t('ext.theme')}</span><select disabled={busy} value={catalog.settings.theme} onchange={async e => {try {await selectTheme(e.currentTarget.value.replace(/^conn\.theme\./,''));} catch {error=t('ext.saveFailed');}}}>{#each catalog.themes as theme}<option value={theme.id}>{theme.name}</option>{/each}</select></label>
 <input bind:this={themeFile} type="file" accept=".json,application/json" hidden onchange={e=>installTheme(e.currentTarget.files?.[0])}/><button class="btn ghost" disabled={busy} onclick={()=>themeFile.click()}>{t("ext.installTheme")}</button>
</section>
<section class="extension">
 <header><div><h3>OpenAI</h3><p class="muted">{t('ext.providerHint')}</p></div><label class="toggle"><span>{t('ext.enabled')}</span><input type="checkbox" disabled={busy || catalog.keyStatus==='unavailable'} checked={catalog.settings.providerEnabled} onchange={e => configure('conn.provider.openai',{enabled:e.currentTarget.checked,model})} /></label></header>
 <form onsubmit={e => {e.preventDefault();configure('conn.provider.openai',{enabled:catalog!.settings.providerEnabled,model});}}>
  <label class="field"><span>{t('ext.model')}</span><input bind:value={model} autocomplete="off" spellcheck="false" placeholder="gpt-4.1-mini" required maxlength="100" /></label>
  <button class="btn ghost" disabled={busy || !model.trim()} type="submit">{t('ext.saveModel')}</button>
 </form>
 <div class="key-status"><span>{t(`ext.key.${catalog.keyStatus}`)}</span>{#if catalog.keyStatus==='stored'}<button class="btn ghost" disabled={busy} onclick={deleteKey}>{t('ext.removeKey')}</button>{/if}</div>
 {#if catalog.keyStatus!=='unavailable'}
 <form class="key-form" onsubmit={e => {e.preventDefault();saveKey();}}>
  <label class="field"><span>{t('ext.keyLabel')}</span><input type="password" bind:value={key} autocomplete="new-password" spellcheck="false" placeholder={t('ext.keyPlaceholder')} required /></label>
  <button class="btn" disabled={busy || !key.trim()} type="submit">{t('ext.saveKey')}</button>
 </form>
 {/if}
</section>
<section class="extension">
 <header><div><h3>{t('ext.completion')}</h3><p class="muted">{t('ext.completionHint')}</p></div><label class="toggle"><span>{t('ext.enabled')}</span><input type="checkbox" disabled={busy} checked={catalog.settings.completionEnabled} onchange={e => configure('conn.completion',{enabled:e.currentTarget.checked})} /></label></header>
 <p class="muted">{t('ext.completionHow')}</p>
</section>
<details><summary>{t('ext.permissions')}</summary>{#each catalog.extensions as extension}<div class="permissions"><b>{extension.name}</b><span class="muted">{extension.capabilities.join(' · ') || t('ext.noPermissions')}</span></div>{/each}</details>
{/if}
{#if error}<p role="alert" class="error">{error}</p>{/if}
<style>
 .extension {padding:16px 0;border-bottom:1px solid var(--line);} .extension:first-child{padding-top:0;} header {display:flex;justify-content:space-between;gap:18px;align-items:flex-start;} h3 {font-size:13px;margin:0 0 5px;} p {margin:4px 0;font-size:12px;line-height:1.5;} .field{display:grid;gap:6px;font-size:12px;margin:12px 0;} input,select{width:100%;min-width:0;padding:8px 10px;border:1px solid var(--line);border-radius:8px;background:var(--bg);color:var(--fg);font:inherit;} .toggle{display:flex;align-items:center;gap:8px;font-size:12px;white-space:nowrap;} .toggle input{width:auto;} .key-form{display:flex;gap:10px;align-items:end;}.key-form .field{flex:1;margin:0;} .key-form .btn{flex-shrink:0;} .key-status{display:flex;align-items:center;justify-content:space-between;gap:8px;font-size:12px;margin:12px 0;} details{margin-top:18px;font-size:12px;} summary{cursor:pointer;color:var(--muted);} .permissions{display:grid;gap:5px;margin:12px 0;} .error{color:var(--danger);font-size:12px;} @media(max-width:620px){.key-form{display:grid;}}
</style>
