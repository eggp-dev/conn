<script lang="ts">
  import { useConnApp } from '../runtime/context';
  const app = useConnApp();
  const { st, invoke, t, requestAction } = app;

  import {sessionLabel} from '../lib/collaboration/activity';
  import DecisionCard from './DecisionCard.svelte';
  let {onchoose,onnew,focused}: {onchoose:(id:string)=>Promise<void>;onnew:()=>Promise<void>;focused?:number}=$props();
  const r=$derived(st.activity.connections.find(a=>a.connId===focused && a.preparation) ?? st.activity.connections.find(a=>a.preparation));
  const action=$derived(requestAction(`prepare:${r?.connId}:${r?.preparation?.id}`));
  const busy=$derived(action.busy),error=$derived(action.error);
  const choices=$derived(st.order.filter(id=>st.tabs[id]?.processAlive));
  let selected=$state('');
  $effect(()=>{if(!choices.includes(selected))selected=choices[0]??'';});
  const run=(work:()=>Promise<unknown>)=>action.run(work);
  async function later(){const request=r;if(!request?.preparation)return;await invoke('dismiss_preparation',{connId:request.connId,requestId:request.preparation.id});}
</script>
{#if r?.preparation}
  <div class="terminal-preparation" data-request={r.preparation.id}>
    <DecisionCard requestKey={`preparation:${r.connId}:${r.preparation.id}`} agent={r.agentId} label={t('prepare.title')} {busy} {error}>
      <p class="reason">{t('prepare.description')}</p>
      {#if r.preparation.reason}<details><summary>{t('prepare.reason')}</summary><p class="reason">{r.preparation.reason}</p></details>{/if}
      {#if choices.length}<label>{t('prepare.terminal')} <select bind:value={selected} disabled={busy}>{#each choices as id}<option value={id}>{sessionLabel(st.order,st.tabs,id)}</option>{/each}</select></label>{/if}
      {#snippet actions()}
        <button class="btn ghost" disabled={busy} onclick={()=>run(later)}>{t('prepare.later')}</button>
        <button class="btn" disabled={busy} onclick={()=>run(onnew)}>{t('prepare.new')}</button>
        {#if choices.length}<button class="btn primary" disabled={busy||!selected} onclick={()=>run(()=>onchoose(selected))}>{t('prepare.choose')}</button>{/if}
      {/snippet}
    </DecisionCard>
  </div>
{/if}
<style>label{display:flex;gap:12px;align-items:center;}select{min-width:0;max-width:100%;flex:1;background:var(--surface2);color:var(--fg);border:1px solid var(--line);border-radius:6px;padding:7px;}details{font-size:12px;color:var(--muted);}</style>
