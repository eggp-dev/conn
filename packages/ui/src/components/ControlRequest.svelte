<script lang="ts">
  import { useConnApp } from '../runtime/context';
  const app = useConnApp();
  const { t, cur, st, controlAction, resolveControl } = app;

  import RequestDetails from './RequestDetails.svelte';
  import { failureKey } from '../lib/collaboration/api';
  const r = $derived(cur().ctlReq);
  const action = $derived(controlAction({ session: st.active, requestId: r?.id ?? 'none' }));
  async function decide(d: 'grant' | 'copilot' | 'deny') {
    if (!r) return;
    const target = { session: st.active, requestId: r.id };
    await resolveControl(target, d);
  }
</script>
{#if r}
  <div class="control-body" aria-busy={action.busy}>
    <p class="scope">{t('ctl.scope')}</p>
    {#if r.reason}<p class="reason">{r.reason}</p>{/if}
    <RequestDetails request={r.originalRequest} />
    {#if action.error}<p class="error" role="alert">{t(failureKey(action.error))}</p>{/if}
    <div class="actions">
      <button class="btn primary" disabled={action.busy} onclick={() => decide('grant')}>{t('ctl.grant')}</button>
      <button class="btn ghost" disabled={action.busy} onclick={() => decide('deny')}>{t('ctl.deny')}</button>
    </div>
    <details class="options"><summary>{t('ctl.options')}</summary>
      <p class="scope">{t('ctl.copilot_hint')}</p>
      <button class="btn" disabled={action.busy} onclick={() => decide('copilot')}>{t('ctl.copilot')}</button>
    </details>
  </div>
{/if}
<style>
  .control-body { min-width:0; display:grid; gap:12px; }
  p { margin:0; white-space:pre-wrap; overflow-wrap:anywhere; line-height:1.55; }
  .scope { color:var(--muted); font-size:12px; }
  .reason { font-size:13px; }
  .actions { display:flex; gap:8px; flex-wrap:wrap; }
  .actions .btn { padding:9px 14px; }
  .options { font-size:11px; color:var(--muted); }
  .options summary { cursor:pointer; width:fit-content; }
  .options p { margin:10px 0; }
  .error { color:var(--danger); }
</style>
