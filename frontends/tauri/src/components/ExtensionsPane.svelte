<script lang="ts">
 import { onMount } from 'svelte';
 import { cmd } from '../lib/bridge';
 import { extensions, refreshExtensions, selectTheme } from '../lib/extensions.svelte';
 import { t } from '../lib/i18n.svelte';
 let loading = $state(true);
 let busy = $state(false);
 let error = $state('');
 let themeFile:HTMLInputElement;
 const catalog = $derived(extensions.catalog);
 onMount(() => { void (async () => { try { await refreshExtensions(); } catch { error = t('ext.loadFailed'); } finally { loading = false; } })(); });
 async function installTheme(file:File|undefined) {
  if(!file)return;busy=true;error='';
  try {if(file.size>65536)throw new Error('Too large');const manifest=JSON.parse(await file.text());await cmd('extension_install_theme',{manifest});await refreshExtensions();}
  catch {error=t('ext.themeFailed');}
  finally {busy=false;if(themeFile)themeFile.value='';}
 }
</script>
{#if loading}<p class="muted">{t('ext.loading')}</p>
{:else if catalog}
<section class="extension">
 <header><div><h3>{t('ext.theme')}</h3><p class="muted">{t('ext.themeHint')}</p></div></header>
 <label class="field"><span>{t('ext.theme')}</span><select disabled={busy} value={catalog.settings.theme} onchange={async e => {try {await selectTheme(e.currentTarget.value.replace(/^conn\.theme\./,''));} catch {error=t('ext.saveFailed');}}}>{#each catalog.themes as theme}<option value={theme.id}>{theme.name}</option>{/each}</select></label>
 <input bind:this={themeFile} type="file" accept=".json,application/json" hidden onchange={e=>installTheme(e.currentTarget.files?.[0])}/><button class="btn ghost" disabled={busy} onclick={()=>themeFile.click()}>{t("ext.installTheme")}</button>
</section>
<details><summary>{t('ext.permissions')}</summary>{#each catalog.extensions as extension}<div class="permissions"><b>{extension.name}</b><span class="muted">{extension.capabilities.join(' · ') || t('ext.noPermissions')}</span></div>{/each}</details>
{/if}
{#if error}<p role="alert" class="error">{error}</p>{/if}
<style>
 .extension {padding:16px 0;border-bottom:1px solid var(--line);} .extension:first-child{padding-top:0;} header {display:flex;justify-content:space-between;gap:18px;align-items:flex-start;} h3 {font-size:13px;margin:0 0 5px;} p {margin:4px 0;font-size:12px;line-height:1.5;} .field{display:grid;gap:6px;font-size:12px;margin:12px 0;} input,select{width:100%;min-width:0;padding:8px 10px;border:1px solid var(--line);border-radius:8px;background:var(--bg);color:var(--fg);font:inherit;} details{margin-top:18px;font-size:12px;} summary{cursor:pointer;color:var(--muted);} .permissions{display:grid;gap:5px;margin:12px 0;} .error{color:var(--danger);font-size:12px;}
</style>
