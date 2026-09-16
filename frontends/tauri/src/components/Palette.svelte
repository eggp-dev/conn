<script lang="ts" module>
  export type Action = {
    id: string;
    label: string;
    hint?: string;
    keys?: string;
    group: string;
    run: () => void;
    when?: () => boolean;
    /** Contextual: something to answer right now. Listed first. */
    now?: boolean;
    /** Current value, shown on the right (a setting). */
    value?: () => string;
    /** The option that is currently in effect (shows a check). */
    active?: () => boolean;
    aliases?: string[];
    danger?: boolean;
  };
  /** Turns a typed query with an argument ("grace 3s", "tab 2") into actions. */
  export type Parser = (q: string) => Action[];
</script>

<script lang="ts">
  import { shortcutLabel } from "../lib/shortcuts";
  import { fade } from "svelte/transition";
  import { t } from "../lib/i18n.svelte";
  import { log } from "../lib/bridge";
  let { actions, parsers = [], onclose }: { actions: Action[]; parsers?: Parser[]; onclose: () => void } = $props();
  let q = $state("");
  let idx = $state(0);
  let input: HTMLInputElement;

  const GLYPH: Record<string, string> = { conn: "◉", approval: "⚠", tabs: "▤", mode: "◐", pacing: "◷", theme: "◑", view: "▣", setup: "⚙", settings: "⚙", agents: "●", language: "Aa" };
  const ORDER = ["conn", "approval", "agents", "tabs", "mode", "pacing", "theme", "view", "language", "settings", "setup"];

  function recents(): string[] {
    try { return JSON.parse(localStorage.getItem("ss:recent") || "[]"); } catch { return []; }
  }
  function remember(id: string) {
    const r = [id, ...recents().filter((x) => x !== id)].slice(0, 8);
    try { localStorage.setItem("ss:recent", JSON.stringify(r)); } catch {}
  }

  const visible = $derived(actions.filter((a) => (a.when ? a.when() : true)));

  /** Every word of the query must hit; substring beats subsequence, label beats alias beats group. */
  function scoreWord(a: Action, w: string): number {
    const label = a.label.toLowerCase();
    if (label.startsWith(w)) return 6;
    if (label.includes(w)) return 4;
    const alias = (a.aliases ?? []).map((x) => x.toLowerCase());
    if (alias.some((x) => x.startsWith(w))) return 3.5;
    if (alias.some((x) => x.includes(w))) return 3;
    if (t(`g.${a.group}`).toLowerCase().includes(w) || a.group.includes(w)) return 2;
    if ((a.hint ?? "").toLowerCase().includes(w) || (a.value?.() ?? "").toLowerCase().includes(w)) return 1.5;
    const hay = `${label} ${alias.join(" ")}`;
    let i = 0;
    for (const ch of w) { i = hay.indexOf(ch, i); if (i < 0) return 0; i++; }
    return 1;
  }
  function score(a: Action, s: string): number {
    let total = 0;
    for (const w of s.split(/\s+/).filter(Boolean)) { const x = scoreWord(a, w); if (!x) return 0; total += x; }
    return total;
  }

  type Row = { kind: "head"; text: string } | { kind: "item"; a: Action };
  const rows = $derived.by((): Row[] => {
    const s = q.trim().toLowerCase();
    const out: Row[] = [];
    if (!s) {
      const now = visible.filter((a) => a.now);
      if (now.length) { out.push({ kind: "head", text: t("pal.now") }); now.forEach((a) => out.push({ kind: "item", a })); }
      const rec = recents().map((id) => visible.find((a) => a.id === id && !a.now)).filter((a): a is Action => !!a).slice(0, 4);
      if (rec.length) { out.push({ kind: "head", text: t("pal.recent") }); rec.forEach((a) => out.push({ kind: "item", a })); }
      for (const g of ORDER) {
        const items = visible.filter((a) => a.group === g && !a.now);
        if (!items.length) continue;
        out.push({ kind: "head", text: t(`g.${g}`) });
        items.forEach((a) => out.push({ kind: "item", a }));
      }
      return out;
    }
    const parsed = parsers.flatMap((p) => { try { return p(q.trim()); } catch { return []; } });
    parsed.forEach((a) => out.push({ kind: "item", a }));
    const rec = recents();
    const scored = visible
      .map((a) => ({ a, s: score(a, s) * (a.now ? 1.5 : 1) * (rec.includes(a.id) ? 1.2 : 1) }))
      .filter((x) => x.s > 0)
      .sort((x, y) => y.s - x.s)
      .slice(0, 12);
    scored.forEach(({ a }) => out.push({ kind: "item", a }));
    return out;
  });
  const items = $derived(rows.filter((r): r is { kind: "item"; a: Action } => r.kind === "item").map((r) => r.a));

  $effect(() => { void rows; idx = 0; });
  $effect(() => { setTimeout(() => input?.focus(), 10); });
  $effect(() => {
    const el = document.querySelector<HTMLElement>(`.pal li.on`);
    el?.scrollIntoView({ block: "nearest" });
  });

  function go(a: Action) {
    try { remember(a.id); a.run(); } catch (e) { log(`palette: ${a.id} failed: ${e}`); }
    onclose();
  }
  function key(e: KeyboardEvent) {
    if (e.key === "ArrowDown") { idx = Math.min(idx + 1, items.length - 1); e.preventDefault(); }
    else if (e.key === "ArrowUp") { idx = Math.max(idx - 1, 0); e.preventDefault(); }
    else if (e.key === "Enter") { const a = items[idx]; if (a) go(a); }
    else if (e.key === "Escape") onclose();
  }
  const isOn = (a: Action) => items[idx] === a;
</script>

<div class="scrim" transition:fade={{ duration: 120 }} onclick={onclose} role="presentation"></div>
<div class="pal palette-surface" role="dialog" aria-label="command palette">
  <div class="row">
    <span class="mag">{shortcutLabel("⌘K")}</span>
    <input bind:this={input} bind:value={q} placeholder={t("pal.placeholder")} onkeydown={key} spellcheck="false" autocomplete="off" role="combobox" aria-expanded="true" aria-controls="pal-list" aria-activedescendant={items[idx] ? `pal-${items[idx].id}` : undefined} aria-label={t("pal.listbox")} />
  </div>
  <ul id="pal-list" role="listbox" aria-label={t("pal.listbox")}>
    {#each rows as r, i (r.kind === "head" ? `h:${r.text}:${i}` : `i:${r.a.id}:${i}`)}
      {#if r.kind === "head"}
        <li class="head" role="presentation">{r.text}</li>
      {:else}
        {@const a = r.a}
        <li id="pal-{a.id}" class:on={isOn(a)} class:now={a.now} class:danger={a.danger} onmouseenter={() => (idx = items.indexOf(a))} onclick={() => go(a)} role="option" aria-selected={isOn(a)} aria-label={[a.label, a.hint, a.value?.(), a.active?.() ? "✓" : "", a.keys].filter(Boolean).join(" · ")}>
          <span class="g" title={t(`g.${a.group}`)}>{GLYPH[a.group] ?? "·"}</span>
          <span class="l">{a.label}{#if a.hint}<span class="h">{a.hint}</span>{/if}</span>
          {#if a.value}<span class="v">{a.value()}</span>{/if}
          {#if a.active?.()}<span class="chk">✓</span>{/if}
          {#if a.keys}<kbd>{a.keys}</kbd>{/if}
        </li>
      {/if}
    {:else}
      <li class="none" role="presentation">{t("pal.none")}</li>
    {/each}
  </ul>
  <footer><kbd>↑↓</kbd> {t("pal.nav")} <kbd>⏎</kbd> {t("pal.run")} <kbd>esc</kbd> {t("pal.close")}</footer>
</div>

<style>
  .scrim { position: absolute; inset: 0; z-index: 40; background: rgba(0,0,0,.25); }
  .pal { z-index: 41; }
  .row { display: flex; align-items: center; gap: 10px; padding: 13px 16px; border-bottom: 1px solid var(--line); }
  .mag { color: var(--muted); font-size: 14px; }
  input { flex: 1; background: transparent; border: 0; outline: 0; color: var(--fg); font: 14.5px var(--font-ui); }
  input::placeholder { color: var(--muted); opacity: .8; }
  ul { list-style: none; margin: 0; padding: 6px; max-height: 58vh; overflow: auto; }
  li { display: flex; align-items: center; gap: 10px; padding: 7px 10px; border-radius: 9px; cursor: pointer; font-size: 13px; }
  li.head { cursor: default; padding: 8px 10px 3px; color: var(--muted); font-size: 10.5px; text-transform: uppercase; letter-spacing: .08em; }
  li.on { background: var(--surface2); }
  li.now .g { color: var(--agent); }
  li.danger .l { color: var(--danger); }
  li.none { color: var(--muted); cursor: default; }
  .g { width: 18px; text-align: center; color: var(--muted); font-size: 12px; flex: 0 0 18px; }
  .l { flex: 1; display: flex; align-items: baseline; gap: 8px; min-width: 0; }
  .l .h { color: var(--muted); font-size: 11.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .v { color: var(--muted); font-size: 12px; font-variant-numeric: tabular-nums; }
  li.on .v { color: var(--fg); }
  .chk { color: var(--ok); font-size: 12px; }
  kbd { font-size: 10.5px; }
  footer { display: flex; gap: 12px; align-items: center; padding: 7px 14px; border-top: 1px solid var(--line); color: var(--muted); font-size: 11px; }
  footer kbd { margin-right: 2px; }

</style>
