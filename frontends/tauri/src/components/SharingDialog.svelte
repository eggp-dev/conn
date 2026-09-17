<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { cmd } from "../lib/bridge";
  import { cur, st, toast } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";
  let { onclose }: { onclose: () => void } = $props();
  const session = st.active;
  let dialog:HTMLDivElement;
  let participants = $state<{connId:number;agentId:string;selected?:boolean}[]>([]);
  let selected = $state<number[]>([]);
  let loading = $state(true);
  let busy = $state(false);
  let error = $state("");
  onMount(async () => {
    dialog?.focus();
    try {
      participants = await cmd("sharing_participants", { session });
      selected = participants.filter(p => p.selected).map(p => p.connId);
    } catch { error = t("sharing.loadFailed"); }
    finally { loading = false; }
  });
  async function apply(shared: boolean) {
    busy = true; error = "";
    try {
      const status = await cmd<{inputPending?:boolean}>("set_sharing", { session, shared, connectionIds: shared ? selected : [] });
      if (st.tabs[session]) st.tabs[session].inputPending = status.inputPending === true;
      toast(t(status.inputPending ? "sharing.pendingInput" : shared ? "sharing.started" : "sharing.stopped"), status.inputPending ? "" : "ok");
      onclose();
    } catch { error = t("sharing.failed"); }
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
      {:else if !participants.length}<p class="muted">{t("sharing.empty")}</p>
      {:else}{#each participants as p (p.connId)}
        <label><input type="checkbox" bind:group={selected} value={p.connId} /><span>{p.agentId}</span><small class="muted">#{p.connId}</small></label>
      {/each}{/if}
    </fieldset>
    {#if error}<p role="alert" class="error">{error}</p>{/if}
  </div>
  <footer>
    {#if cur().shared}<button class="btn ghost" disabled={busy} onclick={() => apply(false)}>{t("sharing.stop")}</button>{/if}
    <span></span><button class="btn" disabled={busy || loading} onclick={() => apply(true)}>{t(cur().shared ? "sharing.apply" : "sharing.start")}</button>
  </footer>
</div>
<style>
 .scrim { position:absolute; inset:0; z-index:42; background:rgba(0,0,0,.16); }
 .sharing { position:absolute; z-index:43; width:min(460px,calc(100vw - 32px)); right:16px; top:56px; max-height:calc(100vh - 80px); overflow:auto; }
 header, footer { display:flex; align-items:center; gap:12px; padding:14px 18px; } header { border-bottom:1px solid var(--line); } h2 { font-size:14px; margin:0; flex:1; } .body { padding:4px 18px 10px; font-size:13px; line-height:1.5; } footer { border-top:1px solid var(--line); } footer span {flex:1;}
 fieldset {border:0;padding:8px 0 0;margin:0;} legend {font-weight:600;font-size:12px;} label {display:flex;align-items:center;gap:10px;padding:8px 0;} label span {flex:1;} .error {color:var(--danger);} .muted {font-size:12px;}
</style>
