<script lang="ts">
  import UpdateDialog from "./UpdateDialog.svelte";
  import ConnMark from "./ConnMark.svelte";
  import { st, cur } from "../lib/store.svelte";
  import { agentColor } from "../lib/themes";
  import { connMarkState, graceProgress } from "../lib/connMark";
  import { tick } from "svelte";
  import { t } from "../lib/i18n.svelte";
  import { shortcutLabel } from "../lib/shortcuts";
  let { onnew, onpalette, ontimeline, onsettings }: { onnew: () => void; onpalette: () => void; ontimeline: () => void; onsettings: () => void } = $props();
  let now = $state(performance.now());
  const active = $derived(cur());
  const preparing = $derived(active.externalStarting || (!st.active && st.externalPending));
  const mark = $derived(preparing ? "human" : connMarkState(active, st.backendOnline, now));
  const markLabel = $derived(preparing ? t("private.preparing") : t(`mark.${mark}`));
  const progress = $derived(graceProgress(active.grace, now));
  $effect(() => {
    const grace = active.grace;
    const blockedUntil = active.policyBlockedUntil;
    now = performance.now();
    if (!grace && blockedUntil <= performance.now()) return;
    let frame = 0;
    const update = () => {
      now = performance.now();
      if ((grace && now < grace.start + grace.ms) || now < blockedUntil) frame = requestAnimationFrame(update);
    };
    frame = requestAnimationFrame(update);
    return () => cancelAnimationFrame(frame);
  });
  let open = $state(false);
  let updates = $state(false);
  let root: HTMLDivElement;
  let trigger: HTMLButtonElement;
  let menu = $state<HTMLDivElement>();
  const entries = $derived([
    { label: t("menu.new"), key: "⌘T", action: onnew, disabled: st.externalPending },
    { label: t("menu.palette"), key: "⌘K", action: onpalette },
    ...(!active.externalPrivate ? [{ label: t("menu.timeline"), key: "⌘J", action: ontimeline, disabled: false }] : []),
    { label: t("center.settings") + "…", key: "⌘,", action: onsettings },
    { label: t("update.title") + "…", key: "", action: () => { updates = true; } },
  ]);
  function close(restore = false) { open = false; if (restore) trigger.focus(); }
  async function show(last = false) {
    open = true; await tick();
    const items = menu?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]');
    items?.[last ? items.length - 1 : 0]?.focus();
  }
  function keydown(e: KeyboardEvent) {
    if (!open) return;
    const items = Array.from(menu!.querySelectorAll<HTMLButtonElement>('[role="menuitem"]'));
    const index = items.indexOf(document.activeElement as HTMLButtonElement);
    if (["ArrowDown", "ArrowUp", "Home", "End", "Escape"].includes(e.key)) {
      e.preventDefault(); e.stopPropagation();
      if (e.key === "Escape") { close(true); return; }
      const next = e.key === "Home" ? 0 : e.key === "End" ? items.length - 1 : (index + (e.key === "ArrowUp" ? -1 : 1) + items.length) % items.length;
      items[next]?.focus();
    } else if (e.key === "Tab") close();
  }
</script>

<svelte:window onpointerdown={(e) => { if (open && !root.contains(e.target as Node)) close(); }} />
<div class="app-menu" bind:this={root} onfocusout={(e) => { if (open && !root.contains(e.relatedTarget as Node)) close(); }}>
  <button class="trigger" bind:this={trigger} aria-haspopup="menu" aria-expanded={open} aria-controls="conn-app-menu" aria-label={t("mark.menu", { state: markLabel })} title={markLabel}
    onclick={() => open ? close(true) : show()}
    onkeydown={(e) => { if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); show(e.key === "ArrowUp"); } }}>
    <ConnMark state={mark} {progress} color={agentColor(active.controller.agentId)} motion={st.effects} />
  </button>
  {#if open}
    <div class="menu" id="conn-app-menu" role="menu" tabindex="-1" aria-label="Conn" bind:this={menu} onkeydown={keydown}>
      {#each entries as entry, i}
        {#if i === 3}<div class="separator" role="separator"></div>{/if}
        <button role="menuitem" tabindex="-1" disabled={entry.disabled} onclick={() => { close(true); entry.action(); }}>
          <span>{entry.label}</span><span class="shortcut" aria-hidden="true">{shortcutLabel(entry.key)}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

{#if updates}<UpdateDialog onclose={async () => { updates = false; await tick(); trigger.focus(); }} />{/if}

<style>
  .app-menu { position: relative; flex: 0 0 auto; margin-right: 6px; }
  .trigger { display: flex; align-items: center; justify-content: center; width: 32px; height: 32px; padding: 3px; border: 1px solid var(--line); border-radius: 7px; background: var(--surface); color: var(--fg); cursor: pointer; font-size: 12px; }
  .trigger:hover, .trigger[aria-expanded="true"] { background: var(--surface2); border-color: var(--muted); }
  .trigger:focus-visible { outline: 2px solid var(--agent); outline-offset: 2px; }
  .menu { position: absolute; top: calc(100% + 8px); left: 0; width: min(300px, calc(100vw - 32px)); padding: 6px; border: 1px solid var(--line); border-radius: 10px; background: var(--surface); box-shadow: var(--shadow); }
  .menu button { display: flex; align-items: center; justify-content: space-between; gap: 24px; width: 100%; min-height: 36px; padding: 8px 10px; border: 0; border-radius: 6px; background: transparent; color: var(--fg); text-align: left; font-size: 13px; cursor: pointer; }
  .menu button:hover, .menu button:focus { outline: none; background: var(--surface2); }
  .menu button:disabled { opacity: .45; cursor: default; }
  .shortcut { color: var(--muted); font-size: 11px; white-space: nowrap; }
  .separator { height: 1px; margin: 5px 4px; background: var(--line); }
</style>
