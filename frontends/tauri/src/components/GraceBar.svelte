<script lang="ts">
  import { t, tabName } from "../lib/i18n.svelte";
  import { fly } from "svelte/transition";
  import { st, cur } from "../lib/store.svelte";
  import { cmd } from "../lib/bridge";
  let now = $state(performance.now());
  $effect(() => { let r = 0; const tick = () => { now = performance.now(); r = requestAnimationFrame(tick); }; r = requestAnimationFrame(tick); return () => cancelAnimationFrame(r); });
  const g = $derived(cur().grace!);
  const left = $derived(Math.max(0, g.ms - (now - g.start)));
</script>

{#if cur().grace}
  <section class="grace" aria-label={t("grace.scheduled")} transition:fly={{ y: 10, duration: 160 }}>
    <div class="heading"><span class="muted">{t("grace.scheduled")}</span><b class="remaining">{(left / 1000).toFixed(1)}s</b></div>
    {#if g.intent}<div class="intent">{g.intent}</div>{/if}
    <code class="command">{g.cmd}</code>
    <div class="track" aria-hidden="true"><div class="fill" style:width="{g.ms > 0 ? (left / g.ms) * 100 : 0}%"></div></div>
    <div class="footer">
      <span class="hint muted">{t("grace.hint")}</span>
      <div class="actions">
        <button class="btn" onclick={() => cmd("execute_now", { execId: g.execId })}>{t("grace.now")} <kbd>⏎</kbd></button>
        <button class="btn ghost" onclick={() => cmd("cancel_exec", { execId: g.execId })}>{t("grace.cancel")} <kbd>esc</kbd></button>
      </div>
    </div>
  </section>
{/if}

<style>
  .grace { position: relative; margin: 6px 16px; box-sizing:border-box; z-index: 12; width: calc(100% - 32px); max-height: calc(100% - 100px); overflow-y: auto; background: var(--surface); border: 1px solid color-mix(in srgb, var(--warn) 40%, var(--line)); border-radius: 12px; padding: 14px 16px; box-shadow: var(--shadow); display: grid; grid-template-columns: minmax(0, 1fr); gap: 10px; font-size: 13px; }
  .heading { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
  .remaining { flex-shrink: 0; font-variant-numeric: tabular-nums; color: var(--warn); }
  .intent { min-width: 0; white-space: pre-wrap; overflow-wrap: anywhere; line-height: 1.55; }
  .command { display: block; min-width: 0; white-space: pre-wrap; overflow-wrap: anywhere; line-height: 1.5; background: var(--bg); border: 1px solid var(--line); border-radius: 6px; padding: 8px 10px; }
  .track { height: 4px; background: var(--surface2); border-radius: 2px; overflow: hidden; }
  .fill { height: 100%; background: var(--warn); transition: width .08s linear; }
  .footer { display: flex; flex-wrap: wrap; align-items: center; gap: 10px 16px; }
  .hint { flex: 1 1 220px; min-width: 0; line-height: 1.5; font-size: 12px; overflow-wrap: anywhere; }
  .actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 6px; margin-left: auto; }
  .actions .btn { white-space: nowrap; min-height: 32px; }
  .actions kbd { margin-left: 4px; }
</style>
