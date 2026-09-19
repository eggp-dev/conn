<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import { fly } from "svelte/transition";
  import { st, toast } from "../lib/store.svelte";
  import { invoke } from "../lib/transport";
  import { agentColor } from "../lib/themes";
  let busy = $state(false);
  // One at a time, oldest first: each answer is a separate, deliberate choice.
  const r = $derived(st.admissions[0]);
  async function decide(allow: boolean) {
    if (!r || busy) return;
    busy = true;
    const { connId } = r;
    try { await invoke("decide_admission", { connId, allow }); }
    catch { toast(t("admission.gone"), "warn"); }
    finally { st.admissions = st.admissions.filter(a => a.connId !== connId); busy = false; }
  }
</script>

{#if r}
  {#key r.connId}
  <div class="req" role="alertdialog" aria-label={t("admission.title")} style:--c={agentColor(r.agentId)} transition:fly={{ y: -10, duration: 180 }}>
    <div class="heading"><span class="who">{r.agentId}</span><span class="muted">{t("admission.asks")}</span>{#if st.admissions.length > 1}<span class="more">{t("admission.more", { n: st.admissions.length - 1 })}</span>{/if}</div>
    <span class="reason">{t("admission.desc")}</span>
    <span class="acts">
      <button class="btn primary" disabled={busy} onclick={() => decide(true)}>{t("admission.allow")}</button>
      <button class="btn ghost" disabled={busy} onclick={() => decide(false)}>{t("admission.deny")}</button>
    </span>
  </div>
  {/key}
{/if}

<style>
  .req { position: relative; margin: 6px 16px; box-sizing:border-box; z-index: 19; display: grid; grid-template-columns: minmax(0, 1fr); align-items: center; gap: 8px 16px; width: calc(100% - 32px); padding: 14px 16px; border-radius: 12px; background: var(--surface); border: 1px solid color-mix(in srgb, var(--c) 50%, var(--line)); box-shadow: var(--shadow); font-size: 13px; }
  .heading { display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 8px; min-width: 0; }
  .who { color: var(--c); font-weight: 700; overflow-wrap: anywhere; }
  .more { margin-left: auto; color: var(--muted); font-size: 12px; }
  .reason { min-width: 0; line-height: 1.55; color: var(--muted); }
  .acts { display: flex; justify-content: flex-end; flex-wrap: wrap; gap: 6px; }
  .acts .btn { padding: 7px 10px; white-space: nowrap; flex-shrink: 0; }
</style>
