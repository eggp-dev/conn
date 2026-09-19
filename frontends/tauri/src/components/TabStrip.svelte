<script lang="ts">
  import { t, tabName } from "../lib/i18n.svelte";
  import { st, tab, cur } from "../lib/store.svelte";
  import { agentColor } from "../lib/themes";
  import AppMenu from "./AppMenu.svelte";
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
    <button class="tab" class:on={id === st.active} class:agent class:knock={!!tb.attention || !!tb.ctlReq || (!!waiting && id !== st.active)} class:dead={!tb.processAlive && !tb.externalStarting}
            style:--c={c} role="tab" aria-selected={id === st.active} onmousedown={(e) => e.preventDefault()} onclick={() => onselect(id)} title={[tb.profileName, tb.title].filter(Boolean).join(" · ")}>
      <span class="dot"></span>
      <span class="n">{i + 1}</span>
      <span class="title">{tb.title}</span>
      {#if !tb.shared}<span class="badge" title={t("private.description")}>{t(tb.externalStarting ? "private.preparing" : "private.label")}</span>{/if}
      {#if tb.openedBy}<span class="by" style:--a={agentColor(tb.openedBy)} title={t("tab.opened.title", { agent: tb.openedBy })}>{tb.openedBy}</span>{/if}
      {#if tb.approval}<span class="badge warn">{t("badge.approval")}</span>{/if}
      {#if waiting && id !== st.active}<span class="badge prop" style:--a={agentColor(waiting.agentId)} title={t("badge.proposal.title", { agent: waiting.agentId })}>{t("badge.proposal", { agent: waiting.agentId })}</span>{/if}
      {#if tb.attention}<span class="badge">{t("badge.knock", { agent: tb.attention.agentId })}</span>{/if}
      <span class="x" role="button" tabindex="-1" onclick={(e) => { e.stopPropagation(); onclose(id); }}>×</span>
    </button>
  {/each}
  </div>
  <button class="new" disabled={st.externalPending} onmousedown={(e) => e.preventDefault()} onclick={()=>onnew()} title={t("tab.new")}>+</button>
  <span class="spacer"></span>
</div>

<style>
  .tabs { position: absolute; top: 0; left: 0; right: 0; height: 44px; z-index: 18; display: flex; align-items: center; gap: 4px; padding: 8px 64px 6px 16px; }
  .tabs.private { padding-right: min(380px, 56vw); }
  .new:disabled { opacity: .4; cursor: default; }
  .tablist { display: flex; align-items: center; gap: 4px; min-width: 0; overflow-x: auto; scrollbar-width: thin; }
  .tab { flex-shrink: 0; display: flex; align-items: center; gap: 7px; height: 28px; padding: 0 10px 0 9px; border-radius: 8px; border: 1px solid transparent; background: transparent; color: var(--muted); font-size: 12px; cursor: pointer; max-width: 340px; transition: background .15s, color .15s, border-color .15s; }
  .tab:hover { background: var(--surface2); }
  .tab.on { background: var(--surface); border-color: var(--line); color: var(--fg); }
  .tab.agent.on { border-color: color-mix(in srgb, var(--c) 55%, var(--line)); }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--c); flex: 0 0 7px; }
  .tab.knock .dot { animation: knock 1.4s ease-in-out infinite; }
  .tab.dead .dot { background: var(--danger); }
  @keyframes knock { 0%, 100% { transform: scale(1); opacity: .6; } 50% { transform: scale(1.6); opacity: 1; } }
  .n { font-variant-numeric: tabular-nums; color: var(--muted); font-size: 11px; }
  .title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 4ch; }
  .by { font-size: 10px; color: var(--a); border: 1px solid color-mix(in srgb, var(--a) 45%, transparent); border-radius: 999px; padding: 0 5px; line-height: 14px; white-space: nowrap; }
  .badge { font-size: 10px; padding: 1px 6px; border-radius: 999px; background: var(--surface2); color: var(--fg); white-space: nowrap; }
  .badge.warn { background: color-mix(in srgb, var(--warn) 22%, transparent); color: var(--warn); }
  .badge.prop { background: color-mix(in srgb, var(--a) 22%, transparent); color: var(--a); }
  .x { margin-left: 2px; color: transparent; padding: 0 2px; border-radius: 4px; }
  .tab:hover .x { color: var(--muted); }
  .x:hover { color: var(--fg); background: var(--line); }
  .new { flex-shrink: 0; width: 26px; height: 26px; border-radius: 7px; border: 1px dashed var(--line); background: transparent; color: var(--muted); cursor: pointer; font-size: 15px; line-height: 1; }
  .new:hover { color: var(--fg); border-color: var(--muted); }
  .spacer { flex: 1; }
</style>
