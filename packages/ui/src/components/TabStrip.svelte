<script lang="ts">
  import { useConnApp } from '../runtime/context';
  const app = useConnApp();
  const { t, st, tab, cur } = app;
  import { agentColor } from '../lib/themes';
  import AppMenu from "./AppMenu.svelte";
  import SessionAgents from "./SessionAgents.svelte";
  let { onselect, onclose, onnew, onsettings, onpalette, ontimeline }: { onpalette: () => void; ontimeline: () => void; onsettings: () => void; onselect: (id: string) => void; onclose: (id: string) => void; onnew: (profileId?: string) => void } = $props();
</script>

<div class="tabs" class:private={!cur().shared}>
  <AppMenu onnew={() => onnew()} {onsettings} {onpalette} {ontimeline} />
  <div class="tablist" role="tablist" aria-label={t("g.tabs")}>
  {#each st.order as id, i (id)}
    {@const tb = tab(id)!}
    {@const agent = tb.controller.type === "agent"}
    {@const waiting = tb.proposal?.ready ? tb.proposal : null}
    {@const c = agent ? agentColor(tb.controller.agentId) : tb.attention ? agentColor(tb.attention.agentId) : waiting ? agentColor(waiting.agentId) : "var(--muted)"}
    <div class="tab-item" class:on={id === st.active} class:agent class:knock={!!tb.attention || !!tb.ctlReq || (!!waiting && id !== st.active)} class:dead={!tb.processAlive && !tb.externalStarting}
            style:--c={c}>
    <button class="tab" role="tab" aria-selected={id === st.active} onmousedown={(e) => e.preventDefault()} onclick={() => onselect(id)} title={[tb.profileName, tb.title].filter(Boolean).join(" · ")}>
      <span class="dot"></span>
      <span class="n">{i + 1}</span>
      <span class="title">{tb.title}</span>
      {#if !tb.shared}<span class="badge" title={t("private.description")}>{t(tb.externalStarting ? "private.preparing" : "private.label")}</span>{/if}
      {#if tb.approval}<span class="badge warn">{t("badge.approval")}</span>{/if}
      {#if waiting && id !== st.active}<span class="badge prop" style:--a={agentColor(waiting.agentId)} title={t("badge.proposal.title", { agent: waiting.agentId })}>{t("badge.proposal", { agent: waiting.agentId })}</span>{/if}
      {#if tb.attention}<span class="badge">{t("badge.knock", { agent: tb.attention.agentId })}</span>{/if}
    </button>
    <SessionAgents session={id} />
    <button class="x" aria-label={t("tab.close", {tab: tb.title || app.tabName(i + 1)})} onclick={() => onclose(id)}>×</button>
    </div>
  {/each}
  </div>
  <button class="new" disabled={st.externalPending} onmousedown={(e) => e.preventDefault()} onclick={()=>onnew()} title={t("tab.new")}>+</button>
  <span class="spacer"></span>
</div>

<style>
  .tabs { position: absolute; top: 0; left: 0; right: 0; height: 44px; z-index: 18; display: flex; align-items: center; gap: 4px; padding: 8px 64px 6px 16px; }
  .tabs.private { padding-right: min(380px, 56vw); }
  .tabs:has(:global(.app-menu .menu)) { z-index: 29; }
  .new:disabled { opacity: .4; cursor: default; }
  .tablist { display: flex; align-items: center; gap: 4px; min-width: 0; overflow-x: auto; scrollbar-width: thin; }
  .tab-item { flex-shrink: 0; display: flex; align-items: center; gap: 7px; height: 28px; padding: 0 6px 0 9px; min-width: 0; border-radius: 8px; border: 1px solid transparent; background: transparent; color: var(--muted); font-size: 12px; cursor: pointer; max-width: 340px; transition: background .15s, color .15s, border-color .15s; }
  .tab-item:hover { background: var(--surface2); }
  .tab-item.on { background: var(--surface); border-color: var(--line); color: var(--fg); }
  .tab-item.agent.on { border-color: color-mix(in srgb, var(--c) 55%, var(--line)); }
  .tab { display: flex; align-items: center; gap: 7px; min-width: 0; padding: 0; border: 0; background: transparent; color: inherit; font: inherit; cursor: pointer; }
  .tab:focus-visible, .x:focus-visible { outline: 2px solid var(--agent); outline-offset: 2px; border-radius: 4px; }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--c); flex: 0 0 7px; }
  .tab-item.knock .dot { animation: knock 1.4s ease-in-out infinite; }
  .tab-item.dead .dot { background: var(--danger); }
  @keyframes knock { 0%, 100% { transform: scale(1); opacity: .6; } 50% { transform: scale(1.6); opacity: 1; } }
  .n { font-variant-numeric: tabular-nums; color: var(--muted); font-size: 11px; }
  .title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; max-width: 200px; }
  .badge { flex-shrink: 0; font-size: 10px; padding: 1px 6px; border-radius: 999px; background: var(--surface2); color: var(--fg); white-space: nowrap; }
  .badge.warn { background: color-mix(in srgb, var(--warn) 22%, transparent); color: var(--warn); }
  .badge.prop { background: color-mix(in srgb, var(--a) 22%, transparent); color: var(--a); }
  .x { flex: 0 0 20px; width: 20px; height: 22px; padding: 0; border: 0; background: transparent; color: var(--muted); border-radius: 4px; cursor: pointer; font: inherit; }
  .x:hover { color: var(--fg); background: var(--line); }
  .new { flex-shrink: 0; width: 26px; height: 26px; border-radius: 7px; border: 1px dashed var(--line); background: transparent; color: var(--muted); cursor: pointer; font-size: 15px; line-height: 1; }
  .new:hover { color: var(--fg); border-color: var(--muted); }
  .spacer { flex: 1; }
</style>
