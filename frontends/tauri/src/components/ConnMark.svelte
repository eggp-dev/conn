<script lang="ts">
  import type { ConnMarkState } from "../lib/connMark";
  let { state = "human", progress = 0, size = 24, color = "var(--agent)", motion = true }:
    { state?: ConnMarkState; progress?: number; size?: number; color?: string; motion?: boolean } = $props();
</script>

<svg class="conn-mark" class:still={!motion} data-state={state} viewBox="0 0 64 64" width={size} height={size} style:--identity={color} aria-hidden="true" focusable="false">
  <path class="rail" d="M44 16 A21 21 0 1 0 44 48" />
  <path class="clock" d="M40 21 A14 14 0 1 0 40 43" pathLength="100" stroke-dashoffset={100 * (1 - Math.min(1, Math.max(0, progress)))} />
  <rect class="cursor" x="42" y="26" width="8" height="12" rx="4" />
  {#if state === 'paused'}<g class="pause"><rect x="30" y="26" width="3" height="12" rx="1.5" /><rect x="36" y="26" width="3" height="12" rx="1.5" /></g>{/if}
</svg>

<style>
  .conn-mark { --ink: var(--fg); display: block; flex-shrink: 0; overflow: visible; }
  .rail { stroke: var(--ink); fill: none; stroke-width: 8; stroke-linecap: round; transition: stroke 450ms ease, opacity 450ms ease; }
  .cursor { fill: var(--ink); transform: translateX(0); transform-origin: 46px 32px; transition: transform 650ms cubic-bezier(.22,1,.36,1), fill 450ms ease; }
  .clock { fill: none; stroke: var(--warn); stroke-width: 2.5; stroke-linecap: round; stroke-dasharray: 100; opacity: 0; }
  .conn-mark[data-state="agent"] { --ink: var(--identity); }
  .conn-mark[data-state="request"], .conn-mark[data-state="grace"], .conn-mark[data-state="paused"] { --ink: var(--warn); }
  .conn-mark[data-state="blocked"] { --ink: var(--danger); }
  .conn-mark[data-state="offline"] { --ink: var(--muted); }
  .conn-mark[data-state="agent"] .cursor { transform: translateX(-9px); animation: working 3.6s ease-in-out infinite; }
  .conn-mark[data-state="request"] .cursor { animation: asking 2.8s ease-in-out infinite; }
  .conn-mark[data-state="grace"] .cursor { transform: translateX(-9px); }
  .conn-mark[data-state="grace"] .clock { opacity: 1; }
  .conn-mark[data-state="blocked"] .cursor { transform: translateX(3px); animation: retreat 500ms ease-out 1; }
  .conn-mark[data-state="offline"] .rail { opacity: .4; }
  .conn-mark[data-state="offline"] .cursor { fill: none; stroke: var(--muted); stroke-width: 2; transform: translateX(3px); }
  .conn-mark[data-state="paused"] .cursor { opacity: 0; }
  .pause { fill: var(--warn); }
  @keyframes working { 0%,100% { opacity: 1; } 50% { opacity: .55; } }
  @keyframes asking { 0%,55%,100% { transform: translateX(0); } 75% { transform: translateX(-3px); } }
  @keyframes retreat { 0% { transform: translateX(-9px); } 55% { transform: translateX(4px); } 100% { transform: translateX(3px); } }
  .still .rail, .still .cursor { animation: none; transition: none; }
  @media (prefers-reduced-motion: reduce) { .rail, .cursor { animation: none !important; transition: none !important; } }
</style>
