<script lang="ts">
  // The control centre: grows out of the island. The eight-tenths of settings you
  // touch while working — mode, ask-first, grace, who is here and what they may do.
  import { fade } from "svelte/transition";
  import { st, cur, toast } from "../lib/store.svelte";
  import { cmd, changeMode } from "../lib/bridge";
  import { agentColor } from "../lib/themes";
  import { t, fmtMs } from "../lib/i18n.svelte";
  let { onclose }: { onclose: () => void } = $props();
  const tb = $derived(cur());
  const isAgent = $derived(tb.controller.type === "agent");
  const color = $derived(isAgent ? agentColor(tb.controller.agentId) : "var(--muted)");

  type Live = { conn: number; agentId: string; affordances: string[] };
  let live = $state<Live[]>([]);
  async function poll() { try { live = await cmd<Live[]>("agents"); } catch { live = []; } }
  $effect(() => { void st.active; poll(); const i = setInterval(poll, 1500); return () => clearInterval(i); });

  const MODES = ["observe", "copilot", "autopilot"] as const;
  async function entrust() {
    try { await cmd("entrust"); toast(t("entrust.set"), "ok"); } catch (e) { toast(String(e), "warn"); }
  }
  function key(e: KeyboardEvent) { if (e.key === "Escape") { e.stopPropagation(); onclose(); } }
</script>

<div class="scrim" transition:fade={{ duration: 100 }} onclick={onclose} role="presentation"></div>
<section class="center conn-popover" style:--c={color} onkeydown={key} tabindex="-1" aria-label={t("center.title")}>
  <header>
    <span class="core"></span>
    <b>{!tb.processAlive ? t("shell.exited") : isAgent ? t("conn.agent", { agent: tb.controller.agentId ?? "" }) : t("conn.yours")}</b>
    <span class="muted tabn">{tb.title}</span>
    {#if isAgent}<button class="btn mini" onclick={() => cmd("take")}>{t("center.take")}</button>{/if}
  </header>

  <div class="seg">
    {#each MODES as m}
      <button class:on={tb.mode === m} onclick={() => changeMode(m)} title={t(`mode.${m}.desc`)}>{t(`mode.${m}`)}</button>
    {/each}
  </div>
  {#if tb.effectiveMode !== tb.mode}<p class="muted note">→ {t(`mode.${tb.effectiveMode}`)} · {t("badge.entrusted")}</p>{/if}

  <label class="row">
    <span>{t("gate.short")}</span>
    <span class="spacer"></span>
    <input type="checkbox" class="sw" checked={tb.gate} onchange={(e) => cmd("set_control_gate", { ask: (e.target as HTMLInputElement).checked })} />
  </label>

  <div class="row col">
    <span>{t("center.grace")}<em class="v">{fmtMs(tb.pacing.enterGraceMs)}</em></span>
    <input type="range" min="0" max="5000" step="250" value={tb.pacing.enterGraceMs} oninput={(e) => cmd("set_pacing", { patch: { enterGraceMs: +(e.target as HTMLInputElement).value } })} />
  </div>

  <h4>{t("center.agents")}</h4>
  {#if live.length === 0}
    <p class="muted">{t("center.no_agents")}</p>
  {:else}
    {#each live as a (a.conn)}
      <div class="agent" style:--a={agentColor(a.agentId)}>
        <span class="dot"></span>
        <b>{a.agentId}</b>
        <span class="tools">
          {#if a.affordances.length}
            {#each a.affordances as f}<code>{f}</code>{/each}
          {:else}<span class="muted">{t("center.nothing")}</span>{/if}
        </span>
        {#if tb.lastAgent?.agentId === a.agentId && !tb.entrustedTo}<button class="btn mini" onclick={entrust}>{t("center.entrust")}</button>{/if}
      </div>
    {/each}
  {/if}


</section>

<style>
  .scrim { position: absolute; inset: 0; z-index: 30; }
  .center { position: absolute; top: 44px; right: 14px; z-index: 31; width: min(360px, 92vw); padding: 12px 14px; outline: 0;
    --popover-origin: calc(100% - 20px) -10px; font-size: 12.5px; }
  header { display: flex; align-items: center; gap: 8px; margin-bottom: 10px; }
  .core { width: 8px; height: 8px; border-radius: 50%; background: var(--c); flex: 0 0 8px; }
  header b { font-size: 13px; }
  .tabn { flex: 1; font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .seg { display: grid; grid-template-columns: repeat(3, 1fr); gap: 4px; background: var(--surface2); padding: 3px; border-radius: 10px; }
  .seg button { border: 0; background: transparent; color: var(--muted); padding: 6px 0; border-radius: 8px; cursor: pointer; font-size: 12px; transition: background .15s, color .15s; }
  .seg button.on { background: var(--surface); color: var(--fg); font-weight: 600; box-shadow: 0 1px 4px rgba(0,0,0,.18); }
  .note { margin: 4px 2px 0; font-size: 11px; }
  .row { display: flex; align-items: center; gap: 8px; margin: 10px 0 0; }
  .row.col { flex-direction: column; align-items: stretch; gap: 4px; }
  .row .v { margin-left: 8px; color: var(--muted); font-style: normal; font-variant-numeric: tabular-nums; }
  .spacer { flex: 1; }
  input[type=range] { width: 100%; margin: 0; }
  .sw { appearance: none; width: 30px; height: 18px; border-radius: 999px; background: var(--surface2); border: 1px solid var(--line); position: relative; cursor: pointer; transition: background .2s; }
  .sw::after { content: ""; position: absolute; top: 2px; left: 2px; width: 12px; height: 12px; border-radius: 50%; background: var(--muted); transition: transform .2s var(--ease), background .2s; }
  .sw:checked { background: color-mix(in srgb, var(--agent) 60%, var(--surface2)); }
  .sw:checked::after { transform: translateX(12px); background: #fff; }
  h4 { margin: 14px 0 6px; font-size: 10.5px; text-transform: uppercase; letter-spacing: .08em; color: var(--muted); }
  .agent { display: flex; align-items: center; gap: 8px; padding: 5px 0; }
  .agent .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--a); flex: 0 0 8px; }
  .agent b { color: var(--a); }
  .tools { flex: 1; display: flex; flex-wrap: wrap; gap: 3px; }
  .tools code { font-size: 10px; padding: 1px 5px; border-radius: 999px; background: var(--surface2); color: var(--muted); }
  .btn.mini { padding: 2px 8px; font-size: 11px; }
  p.muted { margin: 2px 0; font-size: 12px; }
</style>
