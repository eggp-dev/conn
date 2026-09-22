<script lang="ts">
  import { useConnApp } from '../runtime/context';
  const app = useConnApp();
  const { t, cur, st, controlAction, resolveControl } = app;

  import RequestDetails from './RequestDetails.svelte';
  import DecisionCard from './DecisionCard.svelte';
  const r = $derived(cur().ctlReq);
  const action = $derived(controlAction({ session: st.active, requestId: r?.id ?? 'none' }));
  async function decide(d: 'grant' | 'copilot' | 'deny') {
    if (!r) return;
    const target = { session: st.active, requestId: r.id };
    await resolveControl(target, d);
  }
</script>
{#if r}
  {#key `${st.active}:${r.id}`}
    <DecisionCard requestKey={`${st.active}:control:${r.id}`} agent={r.agentId} label={t('ctl.asks')} busy={action.busy} error={action.error}>
      {#if r.reason}<p class="reason">{r.reason}</p>{/if}
      <RequestDetails request={r.originalRequest} />
      {#snippet actions()}
        <button class="btn primary" disabled={action.busy} onclick={() => decide('grant')}>{t('ctl.grant')}</button>
        <button class="btn" disabled={action.busy} onclick={() => decide('copilot')}>{t('ctl.copilot')}</button>
        <button class="btn ghost" disabled={action.busy} onclick={() => decide('deny')}>{t('ctl.deny')}</button>
      {/snippet}
    </DecisionCard>
  {/key}
{/if}
