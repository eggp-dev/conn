<script lang="ts">
  import {st, tab} from '../lib/store.svelte';
  import {pendingKind, sessionLabel} from '../lib/collaboration/activity';
  import {t} from '../lib/i18n.svelte';
  import {agentColor} from '../lib/themes';
  let {onselect}: {onselect:(id:string)=>void}=$props();
  const waiting=$derived(st.order.filter(id=>id!==st.active && pendingKind(tab(id)!)));
  const controlled=$derived(st.order.filter(id=>id!==st.active && tab(id)?.shared && tab(id)?.processAlive && tab(id)?.controller.type==='agent' && !waiting.includes(id)));
  const elsewhere=$derived(st.activity.connections.filter(a=>a.session && a.session!==st.active && tab(a.session)?.shared && tab(a.session)?.processAlive && !waiting.includes(a.session) && !controlled.includes(a.session)));
</script>
{#if waiting.length || controlled.length || elsewhere.length}
  <nav class="collaboration-activity" aria-label={t('activity.title')}>
    {#each waiting as id (id)}
      <button class="waiting" onclick={()=>onselect(id)}><span class="dot"></span><span>{sessionLabel(st.order,st.tabs,id)}</span><b>{t(`activity.${pendingKind(tab(id)!)}`)}</b><span aria-hidden="true">→</span></button>
    {/each}
    {#each controlled as id (id)}
      <button style:--c={agentColor(tab(id)!.controller.agentId)} onclick={()=>onselect(id)} title={t('activity.scope')}><span class="dot"></span><b>{tab(id)!.controller.agentId}</b><span>{sessionLabel(st.order,st.tabs,id)}</span><span>{t('activity.controlling')}</span><span aria-hidden="true">→</span></button>
    {/each}
    {#each elsewhere as a (a.connId)}
      <button style:--c={agentColor(a.agentId)} onclick={()=>onselect(a.session!)} title={`${t('activity.binding',{agent:a.agentId})} · #${a.connId}`}><span class="dot"></span><b>{a.agentId}</b><span>{sessionLabel(st.order,st.tabs,a.session!)}</span><span>{t('activity.recent')}</span><span aria-hidden="true">→</span></button>
    {/each}
  </nav>
{/if}
<style>
 .collaboration-activity{display:flex;gap:6px;flex-wrap:wrap;padding:6px 16px;max-height:112px;overflow:auto;}
 button{--c:var(--muted);display:flex;align-items:center;gap:8px;max-width:100%;padding:8px 12px;border:1px solid var(--line);border-radius:10px;background:var(--surface);color:var(--fg);font:inherit;font-size:12px;cursor:pointer;text-align:left;}
 button:hover,button:focus-visible{border-color:var(--c);outline:2px solid color-mix(in srgb,var(--c) 25%,transparent);outline-offset:2px;}
 button.waiting{--c:var(--warn);border-color:color-mix(in srgb,var(--warn) 40%,var(--line));}
 span:not(.dot),b{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-width:0;}b{font-weight:500;color:var(--c);}.dot{width:6px;height:6px;flex:none;border-radius:50%;background:var(--c);}
</style>
