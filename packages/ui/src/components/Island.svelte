<script lang="ts">
  import { useConnApp } from '../runtime/context';
  const app = useConnApp();
  const { t: tr, fmtMs, cmd, st, cur, toast } = app;

  import { tick } from 'svelte';
  import ControlRequest from './ControlRequest.svelte';
  import { badgeMorph } from '../lib/motion';
  import { shortcutLabel } from "../lib/shortcuts";
  // The island: a dot in the top-right corner that says who has the conn. When
  // something happens it pops to the centre, rolls a short message in, then
  // settles back into the dot. Compositor-only motion (transform / opacity).
  import { agentColor } from '../lib/themes';
  let { onopen, onnotice }: { onopen: () => void; onnotice: (target?: import("../lib/collaboration/activity").NoticeTarget) => void } = $props();
  const t = $derived(cur());
  let badge = $state<HTMLButtonElement>();
  let requestPanel = $state<HTMLElement>();
  let collapsedKey = $state('');
  const requestKey = $derived(t.ctlReq ? `${t.id}:control:${t.ctlReq.id}` : '');
  const controlOpen = $derived(!!requestKey && requestKey !== collapsedKey && !st.centerOpen && !st.settingsOpen && !st.paletteOpen && !st.sharingOpen);
  export async function revealControl() { collapsedKey = ''; await tick(); requestPanel?.focus(); }
  function collapse() { collapsedKey = requestKey; badge?.focus(); }
  function requestKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); collapse(); }
  }
  function openBadge() { if (t.ctlReq) void revealControl(); else onopen(); }

  let stopping = $state(false);
  async function stopExternal() {
    if (stopping) return;
    stopping = true;
    const session = t.id;
    try { await cmd("take", { session }); const current = st.tabs[session]; if (current) current.externalInputAvailable = false; }
    catch { toast(tr("private.stop_failed"), "danger"); }
    finally { stopping = false; }
  }
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

{#if !t.shared}
  <div class="private" title={tr("private.description")}>
    <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><rect x="3" y="7" width="10" height="7" rx="2" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M5 7V5a3 3 0 0 1 6 0v2" fill="none" stroke="currentColor" stroke-width="1.5"/></svg>
    <button class="sharing-trigger" onclick={onopen}><span>{tr(t.externalStarting ? "private.preparing" : t.processAlive ? "private.label" : "private.finished")}</span></button>
    {#if t.externalOrigin && t.processAlive && !t.externalStarting}<button disabled={stopping || !t.externalInputAvailable} onclick={stopExternal}>{tr(!t.externalInputAvailable ? "private.stopped" : "private.stop")}</button>{/if}
  </div>
{:else}
<!-- dot (idle state) -->
<button class="dot" class:agent={isAgent} class:facts={showLabel} class:typing={t.typing} class:pending={!!t.ctlReq} class:hidden={controlOpen || (!!st.announcement && !t.ctlReq)} class:dead={!t.processAlive}
        bind:this={badge} tabindex={controlOpen ? -1 : 0} aria-expanded={controlOpen} aria-label={t.ctlReq ? tr("ctl.asks") : label} style:--c={t.ctlReq ? agentColor(t.ctlReq.agentId) : color} onclick={openBadge} title={`${label} · ${shortcutLabel("⌘K")}`}>
  {#if t.approval}
    <svg class="ring" viewBox="0 0 22 22"><circle cx="11" cy="11" r="9" fill="none" stroke="var(--line)" stroke-width="2"/><circle cx="11" cy="11" r="9" fill="none" stroke="var(--warn)" stroke-width="2" stroke-dasharray={C} stroke-dashoffset={C * (1 - ring)} transform="rotate(-90 11 11)"/></svg>
  {/if}
  <span class="core"></span>
  <span class="lbl">{t.ctlReq ? tr("ctl.waiting", {agent:t.ctlReq.agentId}) : label}</span>
</button>
{/if}

{#if controlOpen && t.ctlReq}
  {#key requestKey}
    <div class="control-morph" bind:this={requestPanel} data-request-key={requestKey} role="alertdialog" aria-label={tr('ctl.asks')} tabindex="-1" onkeydown={requestKeydown} style:--c={agentColor(t.ctlReq.agentId)} transition:badgeMorph={badge}>
      <div class="morph-surface"></div>
      <div class="morph-content">
        <header><div><span class="eyebrow">{tr('ctl.asks')}</span><strong>{t.ctlReq.agentId}</strong><span class="shell-name">{t.title}</span></div><button class="btn ghost mini" onclick={collapse}>{tr('ctl.collapse')}</button></header>
        <ControlRequest />
      </div>
    </div>
  {/key}
{/if}

<!-- pill (announcement) -->
{#if st.announcement && !t.ctlReq}
  {#key st.announcement.id}
    <button class="pill {st.announcement.kind}" style:--c={st.announcement.color} style:--ms="{st.announcement.ms}ms" onclick={()=>onnotice(st.announcement?.target)}>
      <span class="core"></span>
      <span class="text">
        {#each words as w, i}<span class="w" style:animation-delay="{60 + i * 45}ms">{w}</span>{/each}
      </span>
      {#if st.announcement.detail}<span class="detail">{st.announcement.detail}</span>{/if}
    </button>
  {/key}
{/if}

<style>
  .dot.pending { border-color:var(--c); }
  .dot.pending .lbl { color:var(--fg); }
  .control-morph { position:absolute; z-index:25; top:44px; right:14px; width:min(470px, calc(100% - 28px)); outline:none; font-size:13px; --morph-x:0px; --morph-y:0px; --morph-sx:1; --morph-sy:1; --morph-radius:14px; --morph-content:1; }
  .morph-surface { position:absolute; inset:0; border:1px solid color-mix(in srgb,var(--c) 55%,var(--line)); background:var(--surface); border-radius:var(--morph-radius); box-shadow:var(--shadow),0 0 24px color-mix(in srgb,var(--c) 12%,transparent); transform-origin:top left; transform:translate(var(--morph-x),var(--morph-y)) scale(var(--morph-sx),var(--morph-sy)); pointer-events:none; }
  .morph-content { position:relative; padding:18px; max-height:min(540px, calc(100dvh - 64px)); overflow:auto; opacity:var(--morph-content); }
  .control-morph:focus-visible .morph-surface { outline:2px solid var(--c); outline-offset:3px; }
  .control-morph header { display:flex; align-items:flex-start; justify-content:space-between; gap:12px; margin-bottom:14px; }
  .control-morph header>div { display:grid; gap:4px; min-width:0; }
  .eyebrow { font-size:11px; color:var(--muted); }
  .control-morph strong { color:var(--c); font-size:18px; overflow-wrap:anywhere; }
  .shell-name { color:var(--muted); font-size:11px; overflow-wrap:anywhere; }

  .private { position: absolute; top: 12px; right: 14px; z-index: 21; display: flex; align-items: center; gap: 7px; min-height: 26px; max-width: min(55vw, 360px); padding: 0 9px; border: 1px solid var(--line); border-radius: 999px; background: var(--surface); color: var(--muted); font-size: 11.5px; }
  .private span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .private svg { flex-shrink: 0; }
  .private button { padding: 3px 4px 3px 9px; border: 0; border-left: 1px solid var(--line); background: transparent; color: var(--fg); cursor: pointer; font: inherit; white-space: nowrap; }
  .private button:disabled { color: var(--muted); cursor: default; }
  .private button:focus-visible { outline: 2px solid var(--agent); outline-offset: 2px; border-radius: 3px; }
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
  .dot:hover .lbl, .dot.agent .lbl, .dot.facts .lbl, .dot.pending .lbl { max-width: 320px; opacity: 1; }
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
