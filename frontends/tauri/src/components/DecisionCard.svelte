<script lang="ts">
  import type { Snippet } from 'svelte';
  import { decisionReveal } from '../lib/motion';
  import { agentColor } from '../lib/themes';
  import { t } from '../lib/i18n.svelte';
  import { failureKey } from '../lib/collaboration/api';
  let { agent, label, busy = false, error = null, requestKey, children, actions }: { agent: string; label: string; busy?: boolean; error?: unknown; requestKey?:string; children?: Snippet; actions: Snippet } = $props();
</script>
<div class="decision-card" data-request-key={requestKey} tabindex="-1" role="alertdialog" aria-label={label} aria-busy={busy} style:--c={agentColor(agent)} transition:decisionReveal>
  <div class="heading"><strong>{agent}</strong><span class="muted">{label}</span></div>
  {@render children?.()}
  {#if error}<p class="error" role="alert">{t(failureKey(error))}</p>{/if}
  <div class="acts">{@render actions()}</div>
</div>
<style>
  .decision-card { position:relative; margin:6px 16px; box-sizing:border-box; display:grid; gap:8px 16px; min-width:0; padding:14px 16px; border-radius:12px; background:var(--surface); border:1px solid color-mix(in srgb,var(--c) 50%,var(--line)); box-shadow:var(--shadow); font-size:13px; }
  .heading { display:flex; flex-wrap:wrap; align-items:baseline; gap:4px 8px; min-width:0; } strong { color:var(--c); overflow-wrap:anywhere; }
  .acts { display:flex; justify-content:flex-end; flex-wrap:wrap; gap:6px; } .acts :global(.btn) { padding:7px 10px; white-space:nowrap; }
  .error { color:var(--danger); margin:0; } .decision-card :global(.reason) { margin:0; min-width:0; line-height:1.55; white-space:pre-wrap; overflow-wrap:anywhere; max-height:30vh; overflow-y:auto; }
</style>
