<script lang="ts">
  import { tick } from "svelte";
  import { dockReveal, dockDismiss } from "../lib/motion";
  let { height = $bindable(0) } = $props<{ height?: number }>();
  import RequestDetails from "./RequestDetails.svelte";
  import { shortcutLabel } from "../lib/shortcuts";
  import { t } from "../lib/i18n.svelte";
  import { st, cur, toast } from "../lib/store.svelte";
  import { cmd } from "../lib/bridge";
  import { agentColor } from "../lib/themes";
  import { visibleTimeline, timelineSteps, controlFor, commandDecisions, isExternalAutomation, isCommand, isProblem, isPolicyBlocked, policyBlockLabel, type TimelineItem, type TimelineFilter, type TimelineStep } from "../lib/timeline";
  const history = $derived(cur().timeline);
  const all = $derived(visibleTimeline(history, "all"));
  $effect(() => { st.active; selected = null; hover = null; });
  const strip = $derived(all.slice(-200));
  let stripNode: HTMLDivElement;
  let start = $state(true), end = $state(true);
  let lastSession = "";
  function edges() { if (stripNode) { start = stripNode.scrollLeft < 2; end = stripNode.scrollWidth - stripNode.clientWidth - stripNode.scrollLeft < 2; } }
  $effect.pre(() => {
    const session = st.active; void strip;
    const node = stripNode;
    const follow = session !== lastSession || !node || node.scrollWidth - node.clientWidth - node.scrollLeft < 2;
    const anchor = node ? Array.from(node.children).find(el => (el as HTMLElement).offsetLeft >= node.scrollLeft) as HTMLElement | undefined : undefined;
    const id = anchor?.dataset.id, offset = anchor ? anchor.offsetLeft - node.scrollLeft : 0;
    lastSession = session;
    void tick().then(() => {
      if (!stripNode || st.active !== session) return;
      if (follow) stripNode.scrollLeft = stripNode.scrollWidth;
      else { const kept = Array.from(stripNode.children).find(el => (el as HTMLElement).dataset.id === id) as HTMLElement | undefined; if (kept) stripNode.scrollLeft = kept.offsetLeft - offset; }
      edges();
    });
  });
  function browse(direction: number) { stripNode.scrollBy({ left: direction * stripNode.clientWidth * .8, behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'instant' : 'smooth' }); }
  let filter = $state<TimelineFilter>("all");
  let selected = $state<string | null>(null);
  let hover = $state<string | null>(null);
  let hoverX = $state(0);
  const items = $derived(visibleTimeline(history, filter).reverse());
  const target = $derived(strip.find(it => it.id === hover));
  const color = (it: TimelineItem) => it.actor === "human" ? "var(--muted)" : agentColor(it.actor);
  const actor = (it: TimelineItem) => it.actor === "human" ? t("tl.you") : it.actor === "shell" ? t("tl.shell") : it.actor;
  const fmt = (ms: number) => new Date(ms).toLocaleTimeString();
  const kind = (it: TimelineItem) => t(`tl.kind.${it.kind}`);
  const status = (it: TimelineItem) => t(isPolicyBlocked(it) ? "tl.policyBlocked" : it.kind === "control" && it.status === "granted" ? "tl.step.control_granted" : `tl.status.${it.status}`);
  function blockReason(it: TimelineItem) {
    const label = policyBlockLabel(it);
    const keys: Record<string, string> = { "run dangerous commands alone": "tl.block.isolation", "protected path": "tl.block.protected", "delete root": "tl.block.root", "fork bomb": "tl.block.fork" };
    return keys[label] ? t(keys[label]) : label ? t("tl.block.rule", { label }) : t("tl.block.unknown");
  }
  const name = (it: TimelineItem) => [actor(it), kind(it), it.text, status(it)].filter(Boolean).join(" · ");
  const canRetype = (it: TimelineItem) => isCommand(it) && !!it.text && !["pending", "scheduled", "granted", "running"].includes(it.status);
  const grace = (it: TimelineItem) => it.steps.find(s => s.type === "scheduled")?.ms;
  function detailText(detail: TimelineStep) {
    if (!detail.text) return "";
    if (detail.type === "control_returned" || detail.type === "cancelled") {
      const key = `reason.${detail.text}`;
      return t(key) === key ? detail.text : t(key);
    }
    return detail.text;
  }
  async function copy(it: TimelineItem) {
    try { await navigator.clipboard.writeText(it.text); toast(t("copied"), "ok"); }
    catch { toast(t("copy.failed"), "warn"); }
  }
  async function retype(it: TimelineItem) {
    if (!canRetype(it)) return;
    await cmd("input", { data: it.text });
    toast(t("tl.retyped"), "ok");
    (document.querySelector('.host:not([hidden]) .xterm-helper-textarea') as HTMLElement | null)?.focus();
  }
  function inspect(it: TimelineItem) { filter = "all"; selected = it.id; st.timelineOpen = true; hover = null; }
</script>

{#if target && !st.timelineOpen}
  <div class="card" style:left="{Math.max(12, Math.min(window.innerWidth - 312, hoverX - 40))}px" style:--c={color(target)}>
    <div class="meta"><b>{actor(target)}</b><span>{kind(target)}</span>{#if isExternalAutomation(history, target)}<span>{t("tl.external")}</span>{/if}<time>{fmt(target.t)}</time></div>
    <div class="results"><span class="outcome" class:problem={isProblem(target)}>{status(target)}</span>{#each commandDecisions(target) as decision}<span class="muted">{t(`tl.${decision}`)}</span>{/each}</div>
    {#if target.text}{#if isCommand(target)}<code>{target.text}</code>{:else}<div>{target.text}</div>{/if}{/if}
    {#if isPolicyBlocked(target)}<div class="block-reason">{blockReason(target)}</div>{/if}
    {#if target.intent}<div class="muted">{target.intent}</div>{/if}
    <div class="muted">{t("tl.inspect")}</div>
  </div>
{/if}

{#if st.timelineOpen}
  <section class="panel" bind:clientHeight={height} in:dockReveal out:dockDismiss aria-label={t("tl.title")}>
    <header>
      <b>{t("tl.title")}</b><span class="muted">{t("tl.records", { n: items.length })}</span>
      <button class="btn ghost close" onclick={() => (st.timelineOpen = false)}>{t("close")} <kbd>esc</kbd></button>
    </header>
    <div class="filters" role="group" aria-label={t("tl.filter")}>
      {#each ["all", "commands", "collaboration"] as value}
        <button class:chosen={filter === value} aria-pressed={filter === value} onclick={() => { filter = value as TimelineFilter; selected = null; }}>{t(`tl.filter.${value}`)}</button>
      {/each}

    </div>

    <ol aria-label={t("tl.title")}>
      {#each items as it (it.id)}
        <li>
          <details open={selected === it.id} ontoggle={(e) => { if (e.currentTarget.open) selected = it.id; else if (selected === it.id) selected = null; }}>
            <summary>
              <span class="marker" class:collab={!isCommand(it)} class:problem={isProblem(it)} style:--c={color(it)} aria-hidden="true"></span>
              <div class="entry">
                <div class="meta"><b>{actor(it)}</b><span>{kind(it)}</span>{#if isExternalAutomation(history, it)}<span>{t("tl.external")}</span>{/if}<time>{fmt(it.t)}</time>{#if it.saved}<span>{t("tl.saved")}</span>{/if}</div>
                {#if it.text}{#if isCommand(it)}<code>{it.text}</code>{:else}<div class="text">{it.text}</div>{/if}{/if}
                {#if isPolicyBlocked(it)}<div class="block-reason">{blockReason(it)}</div>{/if}
                <div class="results">
                  <span class="outcome" class:problem={isProblem(it)}>{status(it)}</span>
                  {#each commandDecisions(it) as decision}<span class="muted">{t(`tl.${decision}`)}</span>{/each}
                  {#if grace(it)}<span class="muted">{t("tl.wait", { seconds: grace(it)! / 1000 })}</span>{/if}
                  <span class="disclose muted">{selected === it.id ? t("tl.less") : t("tl.more")}</span>
                </div>
              </div>
            </summary>
            <div class="detail">
              {#if it.kind === "control" || it.controlId || it.originalRequest}
                <RequestDetails request={it.originalRequest ?? controlFor(history, it)?.originalRequest} />
              {/if}
              {#if it.intent}<p class="intent">{it.intent}</p>{/if}
              {#if isCommand(it) && !it.commandId}<p class="muted execution-note">{t("tl.execution.note")}</p>{/if}
              {#if it.cwd}<p class="muted">{t("tl.cwd")}: <code>{it.cwd}</code></p>{/if}
              {#if it.commandId}<p class="muted">{t("tl.exit")}: {it.exitCode ?? t("tl.status.unknown")}{#if it.durationMs != null} · {t("tl.duration", { seconds: (it.durationMs / 1000).toFixed(2) })}{/if}</p>{/if}
              <ol class="steps" aria-label={t("tl.more")}>
                {#each timelineSteps(history, it) as detail}
                  <li><time>{fmt(detail.t)}</time><div><b>{t(detail.type === "denied" && isPolicyBlocked(it) ? "tl.policyBlocked" : `tl.step.${detail.type}`)}</b>{#if detail.ms}<span> · {detail.ms / 1000}s</span>{/if}{#if detail.text}<p>{detailText(detail)}</p>{/if}</div></li>
                {/each}
              </ol>
              {#if it.policy}<p class="muted">{t("tl.policy")}: <code>{it.policy}</code></p>{/if}
              {#if canRetype(it)}
                <div class="actions"><button class="btn ghost" onclick={() => copy(it)}>{t("tl.hint.copy")}</button><button class="btn" onclick={() => retype(it)}>{t("tl.retype")}</button></div>
              {/if}
            </div>
          </details>
        </li>
      {:else}<li class="empty muted">{t("tl.empty.filtered")}</li>{/each}
    </ol>
  </section>
{/if}

<div class="tl" class:open={st.timelineOpen} onmouseleave={() => (hover = null)} role="presentation">
  <button class="hit" onclick={() => (st.timelineOpen = !st.timelineOpen)} aria-label={t("tl.toggle")}></button>
  <div class="strip-nav">
  <button class="browse" disabled={start} aria-label={t("tl.older")} onclick={() => browse(-1)}>‹</button>
  <div class="strip" bind:this={stripNode} onscroll={edges} onwheel={e => { if (stripNode.scrollWidth <= stripNode.clientWidth || e.ctrlKey) return; e.preventDefault(); stripNode.scrollLeft += Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY; }} onkeydown={e => { if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(e.key)) return; e.preventDefault(); const buttons = Array.from(stripNode.querySelectorAll('button')); const i = buttons.indexOf(document.activeElement as HTMLButtonElement); buttons[e.key === 'Home' ? 0 : e.key === 'End' ? buttons.length - 1 : Math.max(0, Math.min(buttons.length - 1, i + (e.key === 'ArrowRight' ? 1 : -1)))]?.focus(); }} role="group" aria-label={t("tl.title")}>
    {#each strip as it (it.id)}
      <button data-id={it.id} class="seg" class:collab={!isCommand(it)} class:problem={isProblem(it)} class:pending={it.status === "pending" || it.status === "scheduled"} style:--c={color(it)} aria-label={name(it)}
        onmouseenter={(e) => { hover = it.id; const r = e.currentTarget.getBoundingClientRect(); hoverX = r.left + r.width / 2; }}
        onfocus={(e) => { hover = it.id; const r = e.currentTarget.getBoundingClientRect(); hoverX = r.left + r.width / 2; }} onblur={() => (hover = null)} onclick={() => inspect(it)}></button>
    {/each}
  </div>
  <button class="browse" disabled={end} aria-label={t("tl.newer")} onclick={() => browse(1)}>›</button>
  </div>
  <button class="label" onclick={() => (st.timelineOpen = !st.timelineOpen)}><span>{t("tl.title")}</span><b>{all.length}</b><kbd>{shortcutLabel("⌘J")}</kbd></button>
</div>

<style>
  .tl { position: absolute; left: 0; right: 0; bottom: 0; height: 22px; z-index: 15; }
  .hit { position: absolute; inset: 0; background: transparent; border: 0; cursor: pointer; }
  .strip-nav { position: absolute; left: 10px; right: 190px; bottom: 0; height: 24px; display: flex; align-items: center; gap: 2px; }
  .browse { flex: 0 0 24px; height: 24px; border: 0; background: transparent; color: var(--muted); cursor: pointer; border-radius: 4px; }
  .browse:disabled { opacity: .2; cursor: default; }
  .browse:not(:disabled):hover { background: var(--surface2); color: var(--fg); }
  .strip { position: relative; flex: 1; min-width: 0; display: flex; gap: 4px; height: 24px; align-items: center; overflow-x: auto; overflow-y: hidden; scrollbar-width: none; overscroll-behavior-x: contain; }
  .strip::-webkit-scrollbar { display: none; }
  .seg { position: relative; flex: 0 0 24px; width: 24px; height: 24px; border: 0; padding: 0; background: transparent; cursor: pointer; border-radius: 4px; }
  .seg::after { content: ''; position: absolute; inset: 9px 4px; border-radius: 2px; background: var(--c); transition: transform 120ms; }
  .seg:hover::after, .seg:focus-visible::after { transform: scaleY(1.4); }
  .seg:focus-visible { outline: 1px solid var(--fg); outline-offset: -1px; }
  .seg.collab::after { inset: 8px; border-radius: 50%; background: transparent; border: 1px solid var(--c); }
  .seg.problem::after { outline: 1px solid var(--danger); outline-offset: 1px; }
  .seg.pending::after { background: transparent; border: 1px solid var(--c); }
  @media (prefers-reduced-motion: reduce) { .seg::after { transition: none; } }
  .label { position: absolute; right: 12px; bottom: 2px; display: flex; gap: 6px; align-items: baseline; font-size: 11px; color: var(--muted); background: transparent; border: 0; cursor: pointer; padding: 2px 4px; }
  .label:hover, .tl.open .label { color: var(--fg); }
  .label b, time { font-variant-numeric: tabular-nums; }
  .card { position: fixed; bottom: 30px; z-index: 16; width: min(300px, calc(100vw - 24px)); padding: 12px; border-radius: 10px; background: var(--surface); border: 1px solid var(--c); box-shadow: var(--shadow); pointer-events: none; font-size: 12px; display: grid; gap: 8px; overflow-wrap: anywhere; }
  .panel { position: absolute; left: 16px; right: 16px; bottom: calc(28px + var(--handback-space, 0px)); height: min(45vh, 420px); max-height: calc(100vh - 160px - var(--handback-space, 0px)); z-index: 16; display: flex; flex-direction: column; background: var(--surface); border: 1px solid var(--line); border-radius: 12px; box-shadow: var(--shadow); overflow: hidden; font-size: 13px; }
  header { display: flex; align-items: center; flex-wrap: wrap; gap: 10px; padding: 10px 14px; }
  .close { margin-left: auto; }
  .filters { display: flex; align-items: center; flex-wrap: wrap; gap: 4px; padding: 0 14px 10px; border-bottom: 1px solid var(--line); }
  .filters button { padding: 5px 10px; border: 1px solid transparent; border-radius: 6px; color: var(--muted); background: transparent; cursor: pointer; font-size: 12px; }
  .filters button.chosen { background: var(--surface2); border-color: var(--line); color: var(--fg); }
  ol { list-style: none; padding: 0; margin: 0; overflow: auto; }
  .panel > ol { padding: 0 14px; }
  .panel > ol > li { border-bottom: 1px solid var(--line); }
  .panel > ol > li:last-child { border-bottom: 0; }
  summary { display: flex; align-items: flex-start; gap: 12px; padding: 12px 0; cursor: pointer; list-style: none; }
  summary::-webkit-details-marker { display: none; }
  summary:focus-visible, button:focus-visible { outline: 2px solid var(--agent); outline-offset: -2px; }
  .marker { width: 9px; height: 7px; margin-top: 5px; flex: 0 0 9px; border-radius: 2px; background: var(--c); }
  .marker.collab { height: 9px; border-radius: 50%; border: 1px solid var(--c); background: transparent; }
  .marker.problem { outline: 1px solid var(--danger); outline-offset: 2px; }
  .entry { min-width: 0; flex: 1; display: grid; gap: 6px; }
  .meta { display: flex; align-items: baseline; gap: 6px 10px; flex-wrap: wrap; font-size: 11.5px; color: var(--muted); }
  .meta b { color: var(--fg); overflow-wrap: anywhere; }
  .meta time { margin-left: auto; }
  code, .text, .intent { white-space: pre-wrap; overflow-wrap: anywhere; line-height: 1.55; }
  .results { display: flex; align-items: center; flex-wrap: wrap; gap: 6px 12px; font-size: 11.5px; }
  .outcome { width: fit-content; padding: 1px 6px; border: 1px solid var(--line); border-radius: 5px; font-size: 11px; }
  .outcome.problem { color: var(--danger); border-color: color-mix(in srgb, var(--danger) 35%, var(--line)); }
  .block-reason { color: var(--danger); font-size: 12px; line-height: 1.5; white-space: pre-wrap; overflow-wrap: anywhere; }
  .disclose { margin-left: auto; }
  .detail { padding: 0 0 12px 21px; }
  p { margin: 6px 0; overflow-wrap: anywhere; }
  .execution-note { font-size: 11px; }
  .steps { border-left: 1px solid var(--line); padding: 4px 12px; margin: 10px 0; }
  .steps li { display: flex; gap: 12px; padding: 5px 0; font-size: 12px; }
  .steps time { flex-shrink: 0; color: var(--muted); font-size: 11px; }
  .steps div { min-width: 0; }
  .steps p { color: var(--muted); white-space: pre-wrap; }
  .actions { display: flex; gap: 8px; justify-content: flex-end; }
  .empty { padding: 20px 0; }
  @media (max-width: 640px) { .panel { height: 50vh; } }
</style>
