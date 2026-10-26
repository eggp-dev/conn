<script lang="ts">
  import { onMount } from 'svelte';
  import { useConnApp } from '../runtime/context';
  import { applySessionSnapshot, type SessionSnapshot } from '../lib/collaboration/session';
  const {cur, st, t, cmd} = useConnApp();
  let selected = $state<number | null>(null);
  let acknowledged = $state(false);
  let busy = $state(false);
  let error = $state('');
  let now = $state(performance.now());
  const connection = $derived(cur().agentConnections.find(c => c.connId === selected));
  const active = $derived(cur().environmentReview.filter(g => g.until > now));
  onMount(() => { const timer = setInterval(() => now = performance.now(), 250); return () => clearInterval(timer); });
  function close() { selected = null; acknowledged = false; error = ''; }
  async function change(connId: number, start: boolean) {
    if (busy || (start && (!acknowledged || !connection))) return;
    busy = true; error = '';
    const session = st.active;
    try {
      await cmd(start ? 'start_environment_review' : 'stop_environment_review', {session, connId});
      const snapshot = await cmd<SessionSnapshot>('status', {session});
      const tab = st.tabs[session];
      if (tab) applySessionSnapshot(tab, snapshot, tab.title);
      close();
    } catch { error = t('environment.failed'); }
    finally { busy = false; }
  }
  function remaining(until: number) {
    const secs = Math.max(0, Math.ceil((until - now) / 1000));
    return `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, '0')}`;
  }
</script>

{#if active.length || (cur().reviewRequired && cur().agentConnections.length)}
  <section class="environment-review" aria-label={t('environment.title')}>
    {#each active as grant (grant.grantId)}
      <div class="active">
        <strong>{t('environment.active', {time: remaining(grant.until)})}</strong>
        <span>{t('environment.target', {session: `${cur().title} (${cur().id})`, agent: grant.agentId, conn: grant.connId})}</span>
        <button class="btn" disabled={busy} onclick={() => change(grant.connId, false)}>{t('environment.stop')}</button>
      </div>
    {/each}
    {#if cur().reviewRequired}
      <details>
        <summary>{t('environment.title')}</summary>
        {#each cur().agentConnections.filter(c => !active.some(g => g.connId === c.connId)) as c (c.connId)}
          <button class="btn" disabled={busy} onclick={() => {selected = c.connId; acknowledged = false;}}>{t('environment.target', {session: `${cur().title} (${cur().id})`, agent: c.agentId, conn: c.connId})}</button>
        {/each}
        {#if connection}
          <div class="confirmation">
            <strong>{t('environment.target', {session: `${cur().title} (${cur().id})`, agent: connection.agentId, conn: connection.connId})}</strong>
            <p>{t('environment.warning')}</p>
            <p>{t('environment.limit')}</p>
            <label><input type="checkbox" bind:checked={acknowledged} disabled={busy} /> {t('environment.ack')}</label>
            <div><button class="btn" disabled={busy || !acknowledged} onclick={() => change(connection!.connId, true)}>{t('environment.start')}</button>
            <button class="btn" disabled={busy} onclick={close}>{t('environment.close')}</button></div>
          </div>
        {/if}
      </details>
    {/if}
    {#if error}<p role="alert">{error}</p>{/if}
  </section>
{/if}

<style>
  .environment-review {border:1px solid var(--warn); border-radius:8px; padding:8px 12px; font-size:12px; max-height:35vh; overflow:auto;}
  .active {display:flex; align-items:center; flex-wrap:wrap; gap:8px;}
  .active strong {color:var(--warn);}
  .active span, .confirmation strong {overflow-wrap:anywhere;}
  summary {cursor:pointer; padding:4px 0;}
  .confirmation {display:grid; gap:8px; padding:8px 0;}
  p {margin:0; line-height:1.5;}
  label {display:flex; align-items:center; gap:6px;}
  button {white-space:normal; text-align:left;}
</style>
