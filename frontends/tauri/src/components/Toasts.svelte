<script lang="ts">
  import { fly } from "svelte/transition";
  import { st } from "../lib/store.svelte";
</script>

<div class="toasts">
  {#each st.toasts as t (t.id)}
    <div class="toast {t.cls}" transition:fly={{ y: 8, duration: 160 }}>{t.text}</div>
  {/each}
</div>

<style>
  .toasts { position: absolute; right: 16px; bottom: 26px; display: grid; gap: 6px; z-index: 14; pointer-events: none; }
  .toast { background: color-mix(in srgb, var(--surface) 92%, transparent); backdrop-filter: blur(10px); border: 1px solid var(--line); border-left: 3px solid var(--agent); border-radius: 8px; padding: 6px 10px; font-size: 12px; max-width: 46vw; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .toast.warn { border-left-color: var(--warn); } .toast.ok { border-left-color: var(--ok); } .toast.danger { border-left-color: var(--danger); }
</style>
