<script lang="ts">
  import { tick } from 'svelte';
  import { useConnApp } from '../runtime/context';
  import { agentColor } from '../lib/themes';
  const app = useConnApp();
  const { st, tab, t } = app;
  let { session }: { session: string } = $props();
  const terminal = $derived(tab(session));
  const connections = $derived(terminal?.shared ? st.activity.connections.filter(connection => connection.session === session) : []);
  const representative = $derived(connections.find(connection => terminal?.controller.type === 'agent' && connection.agentId === terminal.controller.agentId) ?? connections[0]);
  let open = $state(false);
  let trigger = $state<HTMLButtonElement>();
  let popup = $state<HTMLDivElement>();
  let tabTrigger: HTMLButtonElement | undefined;
  let left = $state(16);
  let top = $state(44);
  let activeWhenOpened = '';
  const popupId = $derived(`session-agents-${session}`);

  function close(restore = false) {
    const fallback = tabTrigger;
    open = false;
    if (restore) void tick().then(() => (trigger?.isConnected ? trigger : fallback)?.focus());
  }
  function position() {
    if (!app.root || !trigger) return;
    const root = app.root.getBoundingClientRect(), anchor = trigger.getBoundingClientRect();
    const width = Math.min(360, root.width - 32);
    left = Math.max(16, Math.min(anchor.left - root.left, root.width - width - 16));
    top = anchor.bottom - root.top + 8;
  }
  async function show() {
    if (!trigger) return;
    activeWhenOpened = st.active;
    tabTrigger = trigger.closest('.tab-item')?.querySelector<HTMLButtonElement>('[role="tab"]') ?? undefined;
    position(); open = true;
    await tick(); popup?.focus();
  }
  // The tab strip scrolls; render details in this app's root so they cannot be
  // clipped by that scroll container or inherit another app's theme/state.
  function portal(node: HTMLDivElement) {
    app.root?.appendChild(node);
    const list = trigger?.closest('[role="tablist"]');
    const moved = () => close(true);
    list?.addEventListener('scroll', moved);
    return { destroy() { list?.removeEventListener('scroll', moved); node.remove(); } };
  }
  function outside(event: PointerEvent) {
    if (open && !trigger?.contains(event.target as Node) && !popup?.contains(event.target as Node)) close();
  }
  function focusout(event: FocusEvent) {
    const next = event.relatedTarget as Node | null;
    if (open && next && !trigger?.contains(next) && !popup?.contains(next)) close();
  }
  function key(event: KeyboardEvent) {
    if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(true); }
    // This is a read-only, non-modal popup. Continue normal header tab order.
    else if (event.key === 'Tab') {
      event.preventDefault();
      const next = trigger?.closest('.tab-item')?.querySelector<HTMLButtonElement>(event.shiftKey ? '[role="tab"]' : '.x');
      close(); void tick().then(() => next?.focus());
    }
  }
  $effect(() => { if (open && (!connections.length || st.active !== activeWhenOpened)) close(true); });
</script>

<svelte:window onpointerdown={outside} onresize={() => { if (open) position(); }} />
{#if representative}
  <div class="agents" onfocusout={focusout}>
    <button class="trigger" bind:this={trigger} aria-haspopup="dialog" aria-expanded={open} aria-controls={open ? popupId : undefined}
      aria-label={t('tab.agents.open', {n: connections.length, tab: terminal?.title ?? ''})}
      title={t('tab.agents.open', {n: connections.length, tab: terminal?.title ?? ''})}
      style:--a={agentColor(representative.agentId)} onclick={() => open ? close(true) : show()}>
      <span class="name">{representative.agentId}</span>
      {#if connections.length > 1}<span class="count">+{connections.length - 1}</span>{/if}
    </button>
  </div>
{/if}
{#if open}
  <div use:portal bind:this={popup} id={popupId} class="connections conn-popover" role="dialog" aria-labelledby={`${popupId}-title`}
    tabindex="-1" style:left={`${left}px`} style:top={`${top}px`} onkeydown={key} onfocusout={focusout}>
    <header><h2 id={`${popupId}-title`}>{t('tab.agents.title', {n: connections.length})}</h2><button class="dismiss" aria-label={t('close')} onclick={() => close(true)}>×</button></header>
    <p class="control">{terminal?.controller.type === 'agent' ? t('conn.agent', {agent: terminal.controller.agentId ?? ''}) : t('conn.yours')}</p>
    <ul>{#each connections as connection (connection.connId)}
      <li><span class="agent-name" style:--a={agentColor(connection.agentId)}>{connection.agentId}</span><small>{t('tab.agents.connection', {n: connection.connId})}</small></li>
    {/each}</ul>
  </div>
{/if}

<style>
  .agents { flex: 0 1 auto; min-width: 0; max-width: 132px; }
  .trigger { display: flex; align-items: center; gap: 5px; max-width: 100%; min-width: 0; height: 20px; padding: 0 6px; border: 1px solid color-mix(in srgb, var(--a) 45%, transparent); border-radius: 999px; background: transparent; color: var(--a); font-size: 10px; cursor: pointer; }
  .name { min-width: 0; max-width: 88px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .count { flex-shrink: 0; color: var(--fg); font-variant-numeric: tabular-nums; }
  .trigger:hover, .trigger[aria-expanded="true"] { background: var(--surface2); }
  .trigger:focus-visible, .dismiss:focus-visible { outline: 2px solid var(--agent); outline-offset: 2px; }
  .connections { position: absolute; z-index: 30; width: min(360px, calc(100% - 32px)); max-height: calc(100% - 64px); padding: 12px 14px; overflow-y: auto; font-size: 12px; outline: none; --popover-origin: 24px -10px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 13px; }
  .dismiss { flex-shrink: 0; width: 26px; height: 26px; border: 0; border-radius: 4px; background: transparent; color: var(--muted); font-size: 16px; cursor: pointer; }
  .dismiss:hover { background: var(--surface2); color: var(--fg); }
  .control { margin: 6px 0 10px; color: var(--muted); overflow-wrap: anywhere; }
  ul { list-style: none; padding: 0; margin: 0; }
  li { display: flex; align-items: flex-start; gap: 12px; padding: 9px 0; border-top: 1px solid var(--line); }
  .agent-name { flex: 1; min-width: 0; color: var(--a); overflow-wrap: anywhere; }
  small { flex-shrink: 0; padding-top: 1px; color: var(--muted); font-size: 10px; font-variant-numeric: tabular-nums; }
</style>
