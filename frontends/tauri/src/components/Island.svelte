<script lang="ts">
  import { shortcutLabel } from "../lib/shortcuts";
  import { t as tr, fmtMs } from "../lib/i18n.svelte";
  // The island: a dot in the top-right corner that says who has the conn. When
  // something happens it pops to the centre, rolls a short message in, then
  // settles back into the dot. Compositor-only motion (transform / opacity).
  import { st, cur } from "../lib/store.svelte";
  import { agentColor } from "../lib/themes";
  let { onopen }: { onopen: () => void } = $props();
  const t = $derived(cur());
  const isAgent = $derived(t.controller.type === "agent");
  const color = $derived(isAgent ? agentColor(t.controller.agentId) : "var(--muted)");
  const who = $derived(!t.processAlive ? tr("shell.exited") : isAgent ? tr("conn.agent", { agent: t.controller.agentId ?? "" }) : tr("conn.yours"));
  // What the next agent command will do: effective mode (with its cap), and the grace.
  const capped = $derived(t.effectiveMode !== t.mode);
  const facts = $derived([
    t.effectiveMode !== "autopilot" || capped ? tr(`mode.${t.effectiveMode}`) + (capped ? ` (${tr("island.capped")})` : "") : "",
    t.pacing.enterGraceMs > 0 ? tr("island.grace", { v: fmtMs(t.pacing.enterGraceMs) }) : "",
  ].filter(Boolean));
  const label = $derived(facts.length ? `${who} · ${facts.join(" · ")}` : who);
  const showLabel = $derived(isAgent || facts.length > 0);
  let now = $state(Date.now());
  $effect(() => { const i = setInterval(() => (now = Date.now()), 500); return () => clearInterval(i); });
  const ring = $derived(t.approval ? Math.max(0, 1 - (now - t.approval.at) / (t.pacing.approvalTtlSecs * 1000)) : 0);
  const C = 2 * Math.PI * 9;
  const words = $derived(st.announcement ? st.announcement.text.split(" ") : []);
</script>

<!-- dot (idle state) -->
<button class="dot" class:agent={isAgent} class:facts={showLabel} class:typing={t.typing} class:hidden={!!st.announcement} class:dead={!t.processAlive}
        style:--c={color} onclick={onopen} title={`${label} · ${shortcutLabel("⌘K")}`}>
  {#if t.approval}
    <svg class="ring" viewBox="0 0 22 22"><circle cx="11" cy="11" r="9" fill="none" stroke="var(--line)" stroke-width="2"/><circle cx="11" cy="11" r="9" fill="none" stroke="var(--warn)" stroke-width="2" stroke-dasharray={C} stroke-dashoffset={C * (1 - ring)} transform="rotate(-90 11 11)"/></svg>
  {/if}
  <span class="core"></span>
  <span class="lbl">{label}</span>
</button>

<!-- pill (announcement) -->
{#if st.announcement}
  {#key st.announcement.id}
    <button class="pill {st.announcement.kind}" style:--c={st.announcement.color} style:--ms="{st.announcement.ms}ms" onclick={onopen}>
      <span class="core"></span>
      <span class="text">
        {#each words as w, i}<span class="w" style:animation-delay="{60 + i * 45}ms">{w}</span>{/each}
      </span>
      {#if st.announcement.detail}<span class="detail">{st.announcement.detail}</span>{/if}
    </button>
  {/key}
{/if}

<style>
  .dot { position: absolute; top: 12px; right: 14px; z-index: 21; height: 26px; display: flex; align-items: center; gap: 8px; padding: 0 10px 0 8px; border-radius: 999px;
    border: 1px solid var(--line); background: color-mix(in srgb, var(--surface) 82%, transparent); backdrop-filter: blur(12px); cursor: pointer;
    transform: scale(1); opacity: 1; transition: transform .28s cubic-bezier(.34, 1.56, .64, 1), opacity .18s, border-color .4s; will-change: transform, opacity; }
  .dot.hidden { transform: scale(.6); opacity: 0; pointer-events: none; }
  .dot.agent { border-color: color-mix(in srgb, var(--c) 55%, transparent); }
  .core { position: relative; width: 8px; height: 8px; border-radius: 50%; background: var(--c); flex: 0 0 8px; transition: background .4s, transform .25s var(--ease); }
  .core::after { content: ""; position: absolute; inset: -3px; border-radius: 50%; border: 1.5px solid var(--c); opacity: 0; transform: scale(.6); }
  .dot.agent .core::after { animation: ring 2s cubic-bezier(.16, 1, .3, 1) infinite; }
  .dot.typing .core { transform: scale(1.5); }
  .dot.dead .core { background: var(--danger); }
  @keyframes ring { 0% { transform: scale(.6); opacity: .8; } 70%, 100% { transform: scale(2.2); opacity: 0; } }
  .lbl { font-size: 11.5px; color: var(--muted); max-width: 0; overflow: hidden; white-space: nowrap; opacity: 0; transition: max-width .3s var(--ease), opacity .2s; }
  .dot:hover .lbl, .dot.agent .lbl, .dot.facts .lbl { max-width: 320px; opacity: 1; }
  .dot.agent .lbl { color: var(--c); }
  .ring { position: absolute; left: 1px; top: 1px; width: 22px; height: 22px; }

  .pill { position: absolute; top: 12px; left: 50%; z-index: 22; height: 34px; max-width: min(72vw, 720px); display: flex; align-items: center; gap: 10px; padding: 0 16px 0 12px; border-radius: 999px; white-space: nowrap;
    border: 1px solid color-mix(in srgb, var(--c) 50%, var(--line)); background: color-mix(in srgb, var(--surface) 90%, transparent); backdrop-filter: blur(14px) saturate(140%);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--c) 25%, transparent), 0 14px 40px color-mix(in srgb, var(--c) 30%, transparent); cursor: pointer; color: var(--fg); font-size: 13px; font-weight: 600;
    transform-origin: 50% 0; animation: pop .55s cubic-bezier(.34, 1.56, .64, 1) both, settle .3s ease-in calc(var(--ms, 2600ms) - .3s) both; will-change: transform, opacity; }
  .pill .core::after { animation: ring 1.2s cubic-bezier(.16, 1, .3, 1) 2; }
  .pill.approval { border-color: color-mix(in srgb, var(--warn) 60%, var(--line)); }
  .pill.warn { border-color: color-mix(in srgb, var(--danger) 60%, var(--line)); }
  .text { display: inline-flex; gap: .3em; overflow: hidden; flex: 0 0 auto; }
  .detail { color: var(--muted); font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; flex: 0 1 auto; opacity: 0; animation: fadein .35s ease-out .35s forwards; }
  .detail::before { content: "— "; }
  @keyframes fadein { to { opacity: 1; } }
  .w { display: inline-block; transform: translateY(110%); opacity: 0; animation: roll .42s cubic-bezier(.2, .9, .2, 1) both; }
  @keyframes pop { 0% { transform: translateX(-50%) scale(.55); opacity: 0; } 100% { transform: translateX(-50%) scale(1); opacity: 1; } }
  @keyframes settle { 0% { transform: translateX(-50%) scale(1); opacity: 1; } 100% { transform: translateX(-50%) scale(.6); opacity: 0; } }
  @keyframes roll { to { transform: translateY(0); opacity: 1; } }
  @media (prefers-reduced-motion: reduce) { .pill { animation: none; } .w { animation: none; transform: none; opacity: 1; } .dot.agent .core::after { animation: none; } }
</style>
