<script lang="ts">
  import { useConnApp } from '../runtime/context';
  const app = useConnApp();
  const { t, st, decideAdmission, requestAction } = app;

  import DecisionCard from './DecisionCard.svelte';
  let { focused }: {focused?:number}=$props();
  const r = $derived(st.admissions.find(a=>a.connId===focused) ?? st.admissions[0]);
  const action = $derived(requestAction(`admission:${r?.connId ?? "none"}`));
  async function decide(allow: boolean) {
    const request = r;
    if (!request) return;
    if (await action.run(() => decideAdmission(request.connId, allow))) {
      st.admissions = st.admissions.filter(a => a.connId !== request.connId);
    }
  }
</script>
{#if r}
  {#key r.connId}
    <DecisionCard placement="connection" requestKey={`connection:${r.connId}`} agent={r.agentId} label={t('admission.asks')} busy={action.busy} error={action.error}>
      <p class="reason">{t('admission.desc')}</p>
      {#if st.admissions.length > 1}<span class="muted">{t('admission.more', { n: st.admissions.length - 1 })}</span>{/if}
      {#snippet actions()}
        <button class="btn primary" disabled={action.busy} onclick={() => decide(true)}>{t('admission.allow')}</button>
        <button class="btn ghost" disabled={action.busy} onclick={() => decide(false)}>{t('admission.deny')}</button>
      {/snippet}
    </DecisionCard>
  {/key}
{/if}
