<script lang="ts">
  import RequestDetails from "./RequestDetails.svelte";
  import { shortcutLabel } from "../lib/shortcuts";
  import { t } from "../lib/i18n.svelte";
  import { st, cur, toast } from "../lib/store.svelte";
  import { cmd } from "../lib/bridge";
  import { agentColor } from "../lib/themes";
  import { visibleTimeline, timelineSteps, controlFor, isCommand, isProblem, isPolicyBlocked, policyBlockLabel, type TimelineItem, type TimelineFilter, type TimelineStep } from "../lib/timeline";
  const history = $derived(cur().timeline);
  const all = $derived(visibleTimeline(history, "all"));
  const strip = $derived(all.slice(-200));
  let filter = $state<TimelineFilter>("all");
  let selected = $state<string | null>(null);
  let hover = $state<string | null>(null);
  let hoverX = $state(0);
  const items = $derived(visibleTimeline(history, filter).reverse());
  const target = $derived(strip.find(it => it.id === hover));
  const color = (it: TimelineItem) => it.actor === "human" ? "var(--muted)" : agentColor(it.actor);
  const actor = (it: TimelineItem) => it.actor === "human" ? t("tl.you") : it.actor;
  const fmt = (ms: number) => new Date(ms).toLocaleTimeString();
  const kind = (it: TimelineItem) => t(`tl.kind.${it.kind}`);
  const status = (it: TimelineItem) => t(isPolicyBlocked(it) ? "tl.policyBlocked" : `tl.status.${it.status}`);
  function blockReason(it: TimelineItem) {
    const label = policyBlockLabel(it);
    const keys: Record<string, string> = { "run dangerous commands alone": "tl.block.isolation", "protected path": "tl.block.protected", "delete root": "tl.block.root", "fork bomb": "tl.block.fork" };
    return keys[label] ? t(keys[label]) : label ? t("tl.block.rule", { label }) : t("tl.block.unknown");
  }
  const name = (it: TimelineItem) => [actor(it), kind(it), it.text, status(it)].filter(Boolean).join(" · ");
  const canRetype = (it: TimelineItem) => isCommand(it) && !!it.text && !["pending", "scheduled", "granted"].includes(it.status);
  const grace = (it: TimelineItem) => it.steps.find(s => s.type === "scheduled")?.ms;
  const hasApproval = (it: TimelineItem) => !!controlFor(history, it)?.steps.some(s => s.type === "control_granted") || it.steps.some(s => s.type === "approval_granted" || s.type === "proposal_committed");
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
    <div class="meta"><b>{actor(target)}</b><span>{kind(target)}</span><time>{fmt(target.t)}</time></div>
    <span class="outcome" class:problem={isProblem(target)}>{status(target)}</span>
    {#if target.text}{#if isCommand(target)}<code>{target.text}</code>{:else}<div>{target.text}</div>{/if}{/if}
    {#if isPolicyBlocked(target)}<div class="block-reason">{blockReason(target)}</div>{/if}
    {#if target.intent}<div class="muted">{target.intent}</div>{/if}
    <div class="muted">{t("tl.inspect")}</div>
  </div>
{/if}

{#if st.timelineOpen}
  <section class="panel" aria-label={t("tl.title")}>
    <header>
      <b>{t("tl.title")}</b><span class="muted">{t("tl.records", { n: items.length })}</span>
      <button class="btn ghost close" onclick={() => (st.timelineOpen = false)}>{t("close")} <kbd>esc</kbd></button>
    </header>
    <div class="filters" role="group" aria-label={t("tl.filter")}>
      {#each ["all", "commands", "collaboration"] as value}
        <button class:chosen={filter === value} aria-pressed={filter === value} onclick={() => { filter = value as TimelineFilter; selected = null; }}>{t(`tl.filter.${value}`)}</button>
      {/each}
      <span class="scope muted">{t("tl.scope")}</span>
    </div>
    <ol aria-label={t("tl.title")}>
      {#each items as it (it.id)}
        <li>
          <details open={selected === it.id} ontoggle={(e) => { if (e.currentTarget.open) selected = it.id; else if (selected === it.id) selected = null; }}>
            <summary>
              <span class="marker" class:collab={!isCommand(it)} class:problem={isProblem(it)} style:--c={color(it)} aria-hidden="true"></span>
              <div class="entry">
                <div class="meta"><b>{actor(it)}</b><span>{kind(it)}</span><time>{fmt(it.t)}</time>{#if it.saved}<span>{t("tl.saved")}</span>{/if}</div>
                {#if it.text}{#if isCommand(it)}<code>{it.text}</code>{:else}<div class="text">{it.text}</div>{/if}{/if}
                {#if isPolicyBlocked(it)}<div class="block-reason">{blockReason(it)}</div>{/if}
                <div class="results">
                  <span class="outcome" class:problem={isProblem(it)}>{status(it)}</span>
                  {#if isCommand(it) && hasApproval(it)}<span class="muted">{t("tl.approved")}</span>{/if}
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
              {#if isCommand(it)}<p class="muted execution-note">{t("tl.execution.note")}</p>{/if}
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
  <div class="strip">
    {#each strip as it (it.id)}
      <button class="seg" class:collab={!isCommand(it)} class:problem={isProblem(it)} class:pending={it.status === "pending" || it.status === "scheduled"} style:--c={color(it)} aria-label={name(it)}
        onmouseenter={(e) => { hover = it.id; const r = e.currentTarget.getBoundingClientRect(); hoverX = r.left + r.width / 2; }}
        onfocus={(e) => { hover = it.id; const r = e.currentTarget.getBoundingClientRect(); hoverX = r.left + r.width / 2; }} onblur={() => (hover = null)} onclick={() => inspect(it)}></button>
    {:else}{#each Array.from({ length: 40 }) as _}<span class="seg ghost"></span>{/each}{/each}
  </div>
  <button class="label" onclick={() => (st.timelineOpen = !st.timelineOpen)}><span>{t("tl.title")}</span><b>{all.length}</b><kbd>{shortcutLabel("⌘J")}</kbd></button>
</div>

<style>
  .tl { position: absolute; left: 0; right: 0; bottom: 0; height: 22px; z-index: 15; }
  .hit { position: absolute; inset: 0; background: transparent; border: 0; cursor: pointer; }
  .strip { position: absolute; left: 16px; right: 170px; bottom: 3px; display: flex; gap: 2px; height: 12px; align-items: flex-end; pointer-events: none; }
  .seg { flex: 1 1 8px; max-width: 28px; min-width: 2px; height: 5px; border: 0; border-radius: 1.5px; padding: 0; background: var(--c); pointer-events: auto; cursor: pointer; transition: height .1s; }
  .seg:hover, .seg:focus-visible { height: 10px; outline: 1px solid var(--fg); outline-offset: 2px; }
  .seg.collab { flex: 0 1 6px; max-width: 6px; height: 6px; border-radius: 50%; background: transparent; border: 1px solid var(--c); }
  .seg.problem { outline: 1px solid var(--danger); outline-offset: 1px; }
  .seg.pending { background: transparent; border: 1px solid var(--c); }
  .seg.ghost { background: var(--line); opacity: .7; pointer-events: none; }
  .label { position: absolute; right: 12px; bottom: 2px; display: flex; gap: 6px; align-items: baseline; font-size: 11px; color: var(--muted); background: transparent; border: 0; cursor: pointer; padding: 2px 4px; }
  .label:hover, .tl.open .label { color: var(--fg); }
  .label b, time { font-variant-numeric: tabular-nums; }
  .card { position: fixed; bottom: 30px; z-index: 16; width: min(300px, calc(100vw - 24px)); padding: 12px; border-radius: 10px; background: var(--surface); border: 1px solid var(--c); box-shadow: var(--shadow); pointer-events: none; font-size: 12px; display: grid; gap: 8px; overflow-wrap: anywhere; }
  .panel { position: absolute; left: 16px; right: 16px; bottom: 28px; max-height: min(60vh, 560px); z-index: 16; display: flex; flex-direction: column; background: var(--surface); border: 1px solid var(--line); border-radius: 12px; box-shadow: var(--shadow); overflow: hidden; font-size: 13px; }
  header { display: flex; align-items: center; flex-wrap: wrap; gap: 10px; padding: 10px 14px; }
  .close { margin-left: auto; }
  .filters { display: flex; align-items: center; flex-wrap: wrap; gap: 4px; padding: 0 14px 10px; border-bottom: 1px solid var(--line); }
  .filters button { padding: 5px 10px; border: 1px solid transparent; border-radius: 6px; color: var(--muted); background: transparent; cursor: pointer; font-size: 12px; }
  .filters button.chosen { background: var(--surface2); border-color: var(--line); color: var(--fg); }
  .scope { margin-left: auto; font-size: 11px; }
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
  @media (max-width: 640px) { .scope { width: 100%; margin: 4px 0 0; } .panel { max-height: 65vh; } }
</style>
