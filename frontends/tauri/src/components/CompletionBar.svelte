<script lang="ts">
 import { onDestroy, tick, untrack } from 'svelte';
 import { cmd } from '../lib/bridge';
 import { st, cur } from '../lib/store.svelte';
 import { extensions } from '../lib/extensions.svelte';
 import { t } from '../lib/i18n.svelte';
 type Job = {id:string;session:string;status:'pending'|'ready'|'failed'|'cancelled'|'accepted';text?:string;error?:string};
 type CompletionStatus = {surfaceAvailable:boolean;completionPromptReady:boolean;shared:boolean;controller:{type:string}};
 let {height=$bindable(0)}:{height?:number}=$props();
 let job=$state<Job|null>(null);
 let error=$state('');
 let working=$state(false);
 let requestSession='';
 let automatic=false;
 let version=0;
 let polling:ReturnType<typeof setTimeout>|undefined;
 let debounce:ReturnType<typeof setTimeout>|undefined;
 const seenInputs=new Map<string,number>();
 function readyForAutomatic(session:string, revision:number) {
  const tb=st.tabs[session];
  return st.active===session && tb?.humanInputRevision===revision && tb.shared && tb.attended
   && tb.controller.type==='human' && extensions.catalog?.settings.completionEnabled
   && extensions.catalog.settings.providerEnabled && extensions.catalog.keyStatus==='stored'
   && document.visibilityState==='visible' && document.hasFocus()
   && !st.paletteOpen && !st.settingsOpen && !st.centerOpen && !st.menuOpen && !st.sharingOpen && !st.timelineOpen
   && !tb.approval && !tb.proposal && !tb.grace && !tb.ctlReq;
 }
 async function close() {
  version++;clearTimeout(polling);st.completionOpen=false;job=null;error='';working=false;
  if(requestSession) await cmd('completion_cancel',{session:requestSession}).catch(()=>{});
 }
 function fail() { if(automatic) void close();else {error=t('ext.completionFailed');working=false;} }
 async function poll(expected:number) {
  if(expected!==version || !job) return;
  try {
   const result=await cmd<Job>('completion_status',{session:requestSession,id:job.id});
   if(expected!==version) return;
   job=result;
   if(result.status==='pending') polling=setTimeout(()=>poll(expected),400);
   else if(result.status==='failed' || result.status==='cancelled') fail();
   else working=false;
  } catch { if(expected===version) fail(); }
 }
 async function request(explicit=true, session=st.active, inputRevision=cur().humanInputRevision) {
  if(!explicit && !readyForAutomatic(session,inputRevision)) return;
  await close();
  requestSession=session;automatic=!explicit;
  const expected=++version;
  try {
   // Quietly decline automatic suggestions outside a verified shell prompt.
   if(!explicit) {
    const status=await cmd<CompletionStatus>('status',{session});
    if(expected!==version || !readyForAutomatic(session,inputRevision) || !status.completionPromptReady) return;
   }
   working=true;error='';st.completionOpen=true;
   // Apply the dock layout before requesting the current backend terminal grid.
   await tick();
   if(expected!==version || st.active!==session) return;
   const result=await cmd<Job>('completion_request',{session,explicit});
   if(expected!==version) return;
   job=result;st.completionOpen=true;
   void poll(expected);
  } catch { if(expected===version) fail(); }
 }
 async function accept() {
  if(!job || job.status!=='ready') return;
  working=true;
  try {await cmd('completion_accept',{session:requestSession,id:job.id});st.completionOpen=false;job=null;}
  catch {error=t('ext.completionStale');}
  finally {working=false;}
 }
 $effect(()=>{const trigger=st.completionRequest;if(trigger) untrack(()=>void request());});
 $effect(()=>{
  const session=st.active, revision=cur().humanInputRevision;
  const previous=seenInputs.get(session);seenInputs.set(session,revision);clearTimeout(debounce);
  // A counter, not the keystrokes, schedules this check. Initial/config/output changes do not.
  if(previous!==undefined && revision && previous!==revision) debounce=setTimeout(()=>void request(false,session,revision),800);
 });
 $effect(()=>{const session=st.active,shared=cur().shared;if(session!==requestSession || !shared) untrack(()=>void close());});
 $effect(()=>{if(!st.completionOpen && (job || working)) untrack(()=>void close());});
 onDestroy(()=>{clearTimeout(debounce);void close();});
</script>
{#if st.completionOpen}
<section class="completion conn-popover" bind:clientHeight={height} aria-label={t('ext.completion')}>
 <div class="copy">{#if error}<p role="status">{error}</p>{:else if working && !job?.text}<p role="status">{t('ext.thinking')}</p>{:else}<code>{job?.text}</code><span class="muted">{t('ext.acceptHint')}</span>{/if}</div>
 {#if job?.status==='ready'}<button class="btn" disabled={working} onclick={accept}>{t('ext.accept')}</button>{/if}
 <button class="btn ghost" onclick={close} aria-label={t('close')}>×</button>
</section>
{/if}
<style>
 .completion {position:relative;pointer-events:auto;margin:6px 16px;display:flex;align-items:center;gap:12px;padding:12px 14px;min-width:0;height:82px;box-sizing:border-box;overflow:auto;font-size:12px;}
 .copy{flex:1;min-width:0;display:grid;gap:6px;}code{font-size:13px;white-space:pre-wrap;overflow-wrap:anywhere;}p{margin:0;line-height:1.5;}.btn{flex-shrink:0;}.muted{font-size:11px;}
</style>
