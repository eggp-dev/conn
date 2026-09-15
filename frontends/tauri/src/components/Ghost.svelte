<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import { fade } from "svelte/transition";
  import { st, cur } from "../lib/store.svelte";
  import { agentColor } from "../lib/themes";
  const p = $derived(cur().proposal!);
  const c = $derived(cur().cursor);
  let innerWidth = $state(1000);
  let innerHeight = $state(700);
  let hintW = $state(0);
  let hintH = $state(0);
  // Below the line when there is room above the timeline strip; otherwise above it.
  const STRIP = 30;
  const below = $derived(c.y + c.h + 6 + hintH + STRIP <= innerHeight);
  const top = $derived(below ? c.y + c.h + 6 : Math.max(8, c.y - hintH - 6));
  const left = $derived(Math.max(16, Math.min(c.x, innerWidth - hintW - 16)));
</script>

<svelte:window bind:innerWidth bind:innerHeight />

{#if cur().proposal}
  <div class="ghost" class:ready={p.ready} style:left="{c.x}px" style:top="{c.y}px" style:height="{c.h}px" style:--c={agentColor(p.agentId)} transition:fade={{ duration: 100 }}>
    <span class="text" style:font-size="{st.fontSize}px">{p.text}</span>
    {#if !p.ready}<span class="caret"></span>{/if}
  </div>
  {#if p.ready}
    <div class="hint" class:above={!below} style:left="{left}px" style:top="{top}px" style:--c={agentColor(p.agentId)} bind:clientWidth={hintW} bind:clientHeight={hintH} role="status">
      {#if p.intent}<b class="i" title={p.intent}>{p.intent}</b><span class="sep">·</span>{/if}
      <kbd>⏎</kbd> {t("ghost.run")} <kbd>esc</kbd> {t("ghost.reject")} <em>· {p.agentId}</em>
    </div>
  {/if}
{/if}

<style>
  .ghost { position: absolute; z-index: 8; pointer-events: none; display: flex; align-items: center; }
  .text { font-family: var(--font-mono); font-style: italic; color: color-mix(in srgb, var(--c) 75%, var(--muted)); white-space: pre; letter-spacing: 0; }
  .ready .text { font-style: normal; color: var(--c); text-shadow: 0 0 12px color-mix(in srgb, var(--c) 60%, transparent); }
  .caret { width: 2px; height: 1em; background: var(--c); margin-left: 1px; animation: blink .8s steps(1) infinite; }
  @keyframes blink { 50% { opacity: 0; } }
  .hint { position: absolute; z-index: 9; max-width: calc(100vw - 32px); white-space: nowrap; font-size: 11px; color: var(--muted); display: flex; gap: 6px; align-items: center; background: color-mix(in srgb, var(--surface) 88%, transparent); backdrop-filter: blur(8px); padding: 3px 8px; border-radius: 6px; border: 1px solid color-mix(in srgb, var(--c) 40%, var(--line)); pointer-events: none; }
  .hint em { font-style: normal; }
  .hint .i { color: var(--fg); font-weight: 600; overflow: hidden; text-overflow: ellipsis; min-width: 0; flex: 0 1 auto; }
  .hint .sep { color: var(--muted); }
</style>
