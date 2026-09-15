<script lang="ts">
  import { shortcutLabel } from "../lib/shortcuts";
  import { t, tabName } from "../lib/i18n.svelte";
  // Shown right after the human leaves a tab where an agent held the conn:
  // leave it to the agent (policy `unattended` cap applies) or let it stay paused.
  import { fly } from "svelte/transition";
  import { st, tab, tabIndex, toast } from "../lib/store.svelte";
  import { cmd } from "../lib/bridge";
  import { agentColor } from "../lib/themes";
  const c = $derived(st.leaveChip!);
  export async function entrust() {
    if (!st.leaveChip) return;
    const { session, agentId } = st.leaveChip;
    try {
      await cmd("entrust", { session });
      const tb = tab(session);
      if (tb) { tb.entrustedTo = agentId; tb.paused = false; }
      toast(t("entrust.done", { agent: agentId, where: tabName(tabIndex(session)) }), "ok");
    } catch (e) { toast(String(e), "warn"); }
    st.leaveChip = null;
  }
</script>

{#if st.leaveChip}
  <button class="chip" style:--c={agentColor(c.agentId)} transition:fly={{ y: 8, duration: 180 }} onmousedown={(e) => e.preventDefault()} onclick={entrust}>
    <span class="sw"></span>
    <span>{@html t("leave.stuck", { agent: c.agentId, where: tabName(tabIndex(c.session)) })}</span>
    <kbd>{shortcutLabel("⌘⏎")}</kbd>
    <span class="x" role="button" tabindex="-1" onclick={(e) => { e.stopPropagation(); st.leaveChip = null; }}>×</span>
  </button>
{/if}

<style>
  .chip { position: absolute; left: 50%; transform: translateX(-50%); bottom: 34px; z-index: 13; display: flex; align-items: center; gap: 10px; padding: 8px 12px; border-radius: 999px; background: color-mix(in srgb, var(--surface) 92%, transparent); backdrop-filter: blur(12px); border: 1px solid color-mix(in srgb, var(--c) 50%, var(--line)); box-shadow: 0 8px 30px color-mix(in srgb, var(--c) 25%, transparent); cursor: pointer; font-size: 12.5px; color: var(--fg); }
  .sw { width: 8px; height: 8px; border-radius: 50%; background: var(--c); }
  .chip b { color: var(--c); }
  .x { color: var(--muted); padding: 0 2px; }
</style>
