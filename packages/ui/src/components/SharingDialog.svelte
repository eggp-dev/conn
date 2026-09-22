<script lang="ts">
  import { useConnApp } from '../runtime/context';
  const app = useConnApp();
  const { sharingSnapshot, setSharing, cur, st, toast, t } = app;

  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { failureKey } from '../lib/collaboration/api';
  import type { Participant } from "../lib/collaboration/contracts";
  let { onclose }: { onclose: () => void } = $props();
  const session = st.active;
  let dialog:HTMLDivElement;
  let participants = $state<Participant[]>([]);
  let selected = $state<number[]>([]);
  let loading = $state(true);
  let busy = $state(false);
  let error = $state("");
  let loadError = $state("");
  let revision = $state<number | null>(null);
  let stale = $state(false);
  let reloadSelection = $state<() => Promise<void>>(async () => {});
  onMount(() => {
    dialog?.focus();
    let disposed = false;
    let refreshing = false;
    let initialized = false;
    async function refresh(reset = false) {
      if (refreshing || busy) return;
      refreshing = true;
      try {
        const snapshot = await sharingSnapshot(session);
        const next = snapshot.participants;
        if (disposed) return;
        // Keep the owner's edits; never select new agents automatically. The
        // saved selection loads once, even when the first attempt failed.
        selected = initialized && !reset
          ? selected.filter(id => next.some(p => p.connId === id))
          : next.filter(p => p.selected).map(p => p.connId);
        initialized = true;
        if (revision === null || reset) revision = snapshot.revision;
        stale = snapshot.revision !== revision;
        if (reset) error = "";
        participants = next;
        loadError = "";
      } catch {
        if (!disposed) loadError = t("sharing.loadFailed");
      } finally {
        refreshing = false;
        if (!disposed) loading = false;
      }
    }
    reloadSelection = () => refresh(true);
    void refresh();
    const timer = setInterval(refresh, 1000);
    return () => { disposed = true; clearInterval(timer); };
  });
  async function apply(shared: boolean) {
    if (busy || revision === null || stale) return;
    if (shared && selected.length === 0) return;
    busy = true; error = "";
    try {
      const status = await setSharing(session, shared, shared ? selected : [], revision);
      if (st.tabs[session]) Object.assign(st.tabs[session], { shared: status.shared, inputPending: status.inputPending, externalInputAvailable: status.externalInputAvailable });
      toast(t(status.inputPending ? "sharing.pendingInput" : shared ? "sharing.started" : "sharing.stopped"), status.inputPending ? "" : "ok");
      onclose();
    } catch (cause) { error = t(failureKey(cause)); if (String(cause).includes('sharing_changed')) stale = true; }
    finally { busy = false; }
  }
  function key(e: KeyboardEvent) {
    if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); onclose(); }
    if (e.key === "Tab") {
      const controls = [...(e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled)')];
      const next = e.shiftKey ? controls[controls.length-1] : controls[0];
      if ((!e.shiftKey && document.activeElement === controls[controls.length-1]) || (e.shiftKey && document.activeElement === controls[0])) { e.preventDefault(); next?.focus(); }
    }
  }
</script>
<div class="scrim" transition:fade={{duration:120}} onclick={onclose} role="presentation"></div>
<div bind:this={dialog} class="sharing palette-surface" role="dialog" aria-modal="true" aria-labelledby="sharing-title" tabindex="-1" onkeydown={key}>
  <header><h2 id="sharing-title">{t("sharing.title")}</h2><button class="btn ghost" onclick={onclose}>{t("close")} <kbd>esc</kbd></button></header>
  <div class="body">
    <p>{t("sharing.confirm")}</p>
    <p class="muted">{t("sharing.authority")}</p>
    {#if cur().inputPending}<p class="muted">{t("sharing.pendingInput")}</p>{/if}
    <fieldset><legend>{t("sharing.participants")}</legend>
      {#if loading}<p class="muted">{t("sharing.loading")}</p>
      {:else if !participants.length}<p class="muted">{t(st.admissions.length ? "sharing.awaiting" : "sharing.empty")}</p>{#if st.admissions.length}<button class="btn" onclick={onclose}>{t("sharing.reviewConnections")}</button>{/if}
      {:else}{#each participants as p (p.connId)}
        <label><input type="checkbox" bind:group={selected} value={p.connId} disabled={busy} /><span>{p.agentId}</span><small class="muted">#{p.connId}</small></label>
      {/each}{/if}
    </fieldset>
    {#if loadError}<p role="alert" class="error">{loadError}</p>{/if}
    {#if error}<p role="alert" class="error">{error}</p>{/if}
    {#if stale}<p role="status" class="muted">{t("sharing.changed")}</p><button class="btn" disabled={busy} onclick={reloadSelection}>{t("sharing.reload")}</button>{/if}
  </div>
  <footer>
    {#if cur().shared}<button class="btn ghost" disabled={busy || stale || revision === null} onclick={() => apply(false)}>{t("sharing.stop")}</button>{/if}
    <span></span><button class="btn" disabled={busy || loading || stale || revision === null || selected.length === 0} onclick={() => apply(true)}>{t(cur().shared ? "sharing.apply" : "sharing.start")}</button>
  </footer>
</div>
<style>
 .scrim { position:absolute; inset:0; z-index:42; background:rgba(0,0,0,.16); }
 .sharing { position:absolute; z-index:43; width:min(460px,calc(100vw - 32px)); right:16px; top:56px; max-height:calc(100vh - 80px); overflow:auto; }
 header, footer { display:flex; align-items:center; gap:12px; padding:14px 18px; } header { border-bottom:1px solid var(--line); } h2 { font-size:14px; margin:0; flex:1; } .body { padding:4px 18px 10px; font-size:13px; line-height:1.5; } footer { border-top:1px solid var(--line); } footer span {flex:1;}
 fieldset {border:0;padding:8px 0 0;margin:0;} legend {font-weight:600;font-size:12px;} label {display:flex;align-items:center;gap:10px;padding:8px 0;} label span {flex:1;} .error {color:var(--danger);} .muted {font-size:12px;}
</style>
