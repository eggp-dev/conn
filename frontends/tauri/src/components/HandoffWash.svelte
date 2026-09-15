<script lang="ts">
  // Compositor-only: a static radial gradient disc, animated with transform + opacity.
  import { st, cur } from "../lib/store.svelte";
  const w = $derived(cur().wash);
</script>

{#if w && st.effects}
  {#key w.key}
    <div class="wash" class:out={w.out} style:--x="{w.x}px" style:--y="{w.y}px" style:--c={w.color}></div>
  {/key}
{/if}

<style>
  .wash {
    position: absolute; left: var(--x); top: var(--y); width: 240vmax; height: 240vmax; margin: -120vmax 0 0 -120vmax;
    border-radius: 50%; pointer-events: none; z-index: 6;
    background: radial-gradient(circle, color-mix(in srgb, var(--c) 34%, transparent) 0%, color-mix(in srgb, var(--c) 14%, transparent) 22%, transparent 48%);
    transform: scale(0); opacity: 0; will-change: transform, opacity;
    animation: wash-in 1.1s cubic-bezier(.16, 1, .3, 1) forwards;
  }
  .wash.out { animation: wash-out .55s cubic-bezier(.4, 0, .6, 1) forwards; }
  @keyframes wash-in { 0% { transform: scale(0); opacity: .9; } 55% { opacity: .6; } 100% { transform: scale(1); opacity: 0; } }
  @keyframes wash-out { 0% { transform: scale(.5); opacity: .45; } 100% { transform: scale(0); opacity: 0; } }
  @media (prefers-reduced-motion: reduce) { .wash { display: none; } }
</style>
