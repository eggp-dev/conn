<script lang="ts">
 import { t } from '../lib/i18n.svelte';
 import { cur } from '../lib/store.svelte';
 import { cmd } from '../lib/bridge';
 const proposal=$derived(cur().proposal);
</script>
{#if proposal}
<section class="proposal conn-popover" data-request-key={`${cur().id}:proposal:${proposal.id}`} tabindex="-1" aria-label={t('ext.proposal')}>
 <header><b>{proposal.agentId}</b><span class="muted">{t('ext.proposal')}</span></header>
 {#if proposal.intent}<p>{proposal.intent}</p>{/if}
 <code>{proposal.text}</code>
 {#if proposal.ready}<footer><button class="btn" onclick={()=>cmd('accept_proposal',{proposalId:proposal!.id})}>{t('ghost.run')} <kbd>⏎</kbd></button><button class="btn ghost" onclick={()=>cmd('reject_proposal',{proposalId:proposal!.id})}>{t('ghost.reject')} <kbd>esc</kbd></button></footer>{/if}
</section>
{/if}
<style>
.proposal{position:relative;margin:6px 16px;padding:12px 14px;font-size:12px;display:grid;gap:8px;}header{display:flex;gap:10px;}p{margin:0;}code{font-size:13px;white-space:pre-wrap;overflow-wrap:anywhere;}footer{display:flex;gap:8px;justify-content:flex-end;}
</style>
