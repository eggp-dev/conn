<script lang="ts">
  import { shortcutLabel } from "../lib/shortcuts";
  import { t } from "../lib/i18n.svelte";
  import { dockReveal, dockDismiss } from "../lib/motion";
  import { cur, toast } from "../lib/store.svelte";
  import { cmd } from "../lib/bridge";
  import { agentColor } from "../lib/themes";
  const a = $derived(cur().lastAgent!);
  export async function handBack() {
    try { await cmd("hand_back"); } catch (e) { toast(String(e), "warn"); }
  }
</script>

{#if cur().handback && cur().lastAgent}
  <button class="chip" style:--c={agentColor(a.agentId)} in:dockReveal out:dockDismiss onclick={handBack}>
    <kbd>{shortcutLabel("⌘⏎")}</kbd><span>{@html t("handback", { agent: a.agentId })}</span>
    {#if a.lastCmd}<code class="muted">{a.lastCmd}</code>{/if}
    <span class="x" role="button" tabindex="-1" onclick={(e) => { e.stopPropagation(); cur().handback = false; }}>×</span>
  </button>
{/if}

<style>
  .chip { position: relative; width: fit-content; margin: 6px 16px; pointer-events: auto; display: flex; align-items: center; gap: 10px; max-width: 70vw; padding: 7px 10px 7px 10px; border-radius: 999px; background: color-mix(in srgb, var(--surface) 90%, transparent); backdrop-filter: blur(12px); border: 1px solid color-mix(in srgb, var(--c) 50%, var(--line)); box-shadow: 0 8px 30px color-mix(in srgb, var(--c) 25%, transparent); cursor: pointer; font-size: 12.5px; }
  .chip b { color: var(--c); }
  code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 34vw; font-size: 11.5px; }
  .x { color: var(--muted); padding: 0 2px; }
  .x:hover { color: var(--fg); }
</style>
