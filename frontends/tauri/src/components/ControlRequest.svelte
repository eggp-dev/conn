<script lang="ts">
  import RequestDetails from "./RequestDetails.svelte";
  import { t } from "../lib/i18n.svelte";
  import { fly } from "svelte/transition";
  import { cur } from "../lib/store.svelte";
  import { cmd, changeMode } from "../lib/bridge";
  import { agentColor } from "../lib/themes";
  const r = $derived(cur().ctlReq!);
  async function decide(d: "grant" | "copilot" | "deny") {
    if (d === "copilot" && !await changeMode("copilot")) return;
    await cmd("decide_control", { requestId: r.id, grant: d !== "deny" });
  }
</script>

{#if cur().ctlReq}
  <div class="req" style:--c={agentColor(r.agentId)} transition:fly={{ y: -10, duration: 180 }}>
    <div class="heading"><span class="who">{r.agentId}</span><span class="muted">{t("ctl.asks")}</span></div>
    {#if r.reason}<span class="reason">{r.reason}</span>{/if}
    <RequestDetails request={r.originalRequest} />
    <span class="acts">
      <button class="btn primary" onclick={() => decide("grant")}>{t("ctl.grant")}</button>
      <button class="btn" onclick={() => decide("copilot")}>{t("ctl.copilot")}</button>
      <button class="btn ghost" onclick={() => decide("deny")}>{t("ctl.deny")}</button>
    </span>
  </div>
{/if}

<style>
  .req { position: relative; margin: 6px 16px; box-sizing:border-box; z-index: 19; display: grid; grid-template-columns: minmax(0, 1fr); align-items: center; gap: 8px 16px; width: calc(100% - 32px); padding: 14px 16px; border-radius: 12px; background: var(--surface); border: 1px solid color-mix(in srgb, var(--c) 50%, var(--line)); box-shadow: var(--shadow); font-size: 13px; }
  .heading { display: flex; flex-wrap: wrap; gap: 4px 8px; min-width: 0; }
  .who { color: var(--c); font-weight: 700; overflow-wrap: anywhere; }
  .reason { grid-column: 1; min-width: 0; line-height: 1.55; white-space: pre-wrap; overflow-wrap: anywhere; max-height: 30vh; overflow-y: auto; }
  .acts { grid-column: 1; display: flex; justify-content: flex-end; flex-wrap: wrap; gap: 6px; }
  .acts .btn { padding: 7px 10px; white-space: nowrap; flex-shrink: 0; }
  @media (max-width: 680px) {
    .req { grid-template-columns: minmax(0, 1fr); }
    .acts { grid-column: 1; grid-row: auto; justify-content: flex-end; }
  }
</style>
