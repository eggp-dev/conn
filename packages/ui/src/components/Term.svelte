<script lang="ts">
  import { useConnApp } from '../runtime/context';
  const app = useConnApp();
  const { cmd, onOutput, listen, st, tab, THEMES, applyTheme } = app;

  import { DOCK_MOTION_MS, dockOffsetFrames, reducedMotion } from "../lib/motion";
  import { appShortcut, shortcutKey } from "../lib/shortcuts";
  import { onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { Unicode11Addon } from "@xterm/addon-unicode11";
  import { observeTerminalInput } from "../lib/terminalInput";
  import { terminalResizeQueue } from "../lib/terminalResize";
  import { terminalOutputQueue } from "../lib/terminalOutput";
  import { b64ToBytes } from '../lib/bridge';
  import { agentColor } from '../lib/themes';

  let { session }: { session: string } = $props();
  let host: HTMLDivElement;
  let term: Terminal;
  let fit: FitAddon;
  let requestFit = () => {};
  let renderReady = $state(false);
  let syncSlow = $state(false);
  let recover = () => {};
  $effect(() => {
    if (renderReady || !active || !st.backendOnline) { syncSlow = false; return; }
    const timer = app.timeout(() => { syncSlow = true; }, 3000);
    return () => app.clearTimeout(timer);
  });
  const t = $derived(tab(session)!);
  const active = $derived(st.active === session);
  export function focus() { term?.focus(); }
  export function size() { return { rows: term?.rows ?? 24, cols: term?.cols ?? 80 }; }
  export function refit() { requestFit(); }

  /** Centre of the cursor cell in the app frame. Measured on demand: only the handoff wash needs it. */
  export function cursorCenter() {
    const screen = host?.querySelector<HTMLElement>(".xterm-screen");
    if (!screen || !term) return null;
    const w = screen.clientWidth / term.cols;
    const h = screen.clientHeight / term.rows;
    const b = term.buffer.active;
    const r = screen.getBoundingClientRect();
    const pr = (host.offsetParent as HTMLElement | null)?.getBoundingClientRect() ?? host.getBoundingClientRect();
    return { x: r.left - pr.left + (b.cursorX + .5) * w, y: r.top - pr.top + (b.cursorY + .5) * h };
  }

  function applyLook() {
    const th = THEMES[st.theme] ?? THEMES.midnight;
    const agent = t?.controller.type === "agent";
    const accent = agent ? agentColor(t.controller.agentId) : "#8b7cff";
    const xt = applyTheme(th, accent, host.closest<HTMLElement>(".app")!);
    if (agent) { xt.cursor = accent; xt.cursorAccent = th.tokens.bg; }
    term.options.theme = xt;
    term.options.cursorStyle = agent ? "underline" : "block";
    term.options.fontSize = st.fontSize;
    if (active) requestFit();
  }

  onMount(() => {
    term = new Terminal({ logLevel: "off", fontFamily: '"SF Mono", "JetBrains Mono", Menlo, monospace', fontSize: st.fontSize, cursorBlink: true, allowProposedApi: true, scrollback: 5000, macOptionIsMeta: true });
    term.loadAddon(new Unicode11Addon());
    term.unicode.activeVersion = '11';
    // Consume clipboard escape sequences before any private output is attached.
    // Returning true prevents fall-through to built-in or future addon handlers.
    const privateClipboard = term.parser.registerOscHandler(52, () => true);
    fit = new FitAddon();
    term.loadAddon(fit);
    term.open(host);
    // Request the pane size, but commit it to xterm through the backend output
    // stream so shell redraws and size changes reach both parsers in one order.
    const sizes = terminalResizeQueue(
      ({ rows, cols }) => cmd("resize", { session, rows, cols }),
      (error) => { console.error("Terminal resize failed", error); app.toast(app.t("terminal.resize_failed"), "warn"); },
    );
    requestFit = () => {
      if (!renderReady || !st.backendOnline) return;
      const size = fit.proposeDimensions();
      if (size && Number.isFinite(size.rows) && Number.isFinite(size.cols)) sizes.resize(size);
    };
    applyLook();
    term.attachCustomKeyEventHandler((e) => {
      if (e.type !== "keydown") return true;
      // Escape closes an open overlay instead of reaching the shell.
      if (e.key === "Escape" && (st.settingsOpen || st.timelineOpen || st.centerOpen || st.paletteOpen || tab(session)?.handback)) return false;
      const key = shortcutKey(e);
      if (appShortcut(e) && ["k", ",", "j", "t", "w", "Enter", "[", "]", "ArrowRight"].includes(key)) return false;
      if (appShortcut(e) && /^[1-9]$/.test(key)) return false;
      return true;
    });
    const inputListener = observeTerminalInput(term, (data, origin) => {
      if (!renderReady || !st.backendOnline) return;
      // The common client presents reconnect for uncertain input; consume the rejected
      // fire-and-forget promise so it cannot become an unhandled browser error.
      const send = (name: string, args: Record<string, unknown>) => { void cmd(name, args).catch(error => { if (st.backendOnline) app.toast(String(error), 'warn'); }); };
      const tb = t;
      if (origin === "terminal") {
        // The first child output can precede the final started-status refresh.
        send("terminal_response", { session, data });
        return;
      }
      // During preparation, genuine input cancels the pending launch. The native
      // boundary discards these bytes instead of sending them to a future child.
      if (tb.externalStarting) {
        send("input", { session, data });
        return;
      }
      if (st.paletteOpen || st.settingsOpen || st.sharingOpen) return;
      if (!!tb.shared && tb.proposal?.ready) {
        if (data === "\r") { send("accept_proposal", { session, proposalId: tb.proposal.id }); return; }
        if (data === "\x1b") { send("reject_proposal", { session, proposalId: tb.proposal.id }); return; }
      }
      // Terminal typing always belongs to the human. A late approval must not
      // turn an intended "a"/"A" keystroke into an execution or a broad grant.
      if (!!tb.shared && tb.grace) {
        if (data === "\r") { send("execute_now", { session, execId: tb.grace.execId }); return; }
        if (data === "\x1b") { send("cancel_exec", { session, execId: tb.grace.execId }); return; }
      }
      if (!tb.shared) tb.externalInputAvailable = false;
      send("input", { session, data });
    });
    let mounted = true;
    let previousBottom = parseFloat(getComputedStyle(host).bottom);
    let motion: Animation | undefined;
    const output = terminalOutputQueue(({ rows, cols }) => {
      if (rows === term.rows && cols === term.cols) return;
      const bottom = parseFloat(getComputedStyle(host).bottom);
      const dockChanged = bottom !== previousBottom;
      previousBottom = bottom;
      // Commit the final terminal grid once. Animate its visual offset, not
      // its height: otherwise every frame resizes the PTY and redraws the shell.
      const transform = getComputedStyle(host).transform;
      const offset = transform === "none" ? 0 : new DOMMatrixReadOnly(transform).m42;
      motion?.cancel();
      const rowHeight = host.querySelector<HTMLElement>(".xterm-screen")!.clientHeight / term.rows;
      const before = term.buffer.active;
      const cursorBefore = before.baseY + before.cursorY - before.viewportY;
      term.resize(cols, rows);
      const after = term.buffer.active;
      const cursorDelta = (cursorBefore - (after.baseY + after.cursorY - after.viewportY)) * rowHeight;
      if (active && dockChanged && cursorDelta && !reducedMotion()) {
        host.style.willChange = "transform";
        motion = host.animate(dockOffsetFrames(cursorDelta + offset), {
          duration: DOCK_MOTION_MS, easing: "linear",
        });
        motion.onfinish = () => { host.style.willChange = ""; };
      } else { host.style.willChange = ""; }
    }, (data, done) => term.write(data, done), () => { motion?.cancel(); host.style.willChange = ""; term.reset(); });
    const ro = new ResizeObserver(() => { if (active) requestFit(); });
    ro.observe(host);
    const un = onOutput((p) => {
      if (!mounted || p.session !== session) return;
      if (p.reset) renderReady = false;
      output.push({ size: p.size, data: b64ToBytes(p.data), reset: p.reset, applied: p.reset ? () => { if (mounted) { renderReady = true; sizes.invalidate(); requestFit(); } } : undefined });
    });
    recover = () => {
      renderReady = false; output.clear(); sizes.invalidate();
      void cmd('attach_output', {session}).catch(error => app.toast(String(error), 'warn'));
    };
    let attachmentRequested = false;
    const attach = () => {
      if (!mounted || attachmentRequested) return;
      attachmentRequested = true;
      void cmd('attach_output', {session}).catch(() => { attachmentRequested = false; });
    };
    const sync = listen<{ session: string; ready: boolean }>('ss:terminal_sync', ({payload}) => {
      if (payload.session === session && !payload.ready) { renderReady = false; output.clear(); }
    });
    const connection = listen<{ connected: boolean }>('ss:connection', ({payload}) => {
      renderReady = false; output.clear(); sizes.invalidate();
      if (!payload.connected) attachmentRequested = false;
      else attach();
    });
    Promise.all([un, sync, connection]).then(attach).catch(() => {});
    return () => { mounted = false; sizes.dispose(); output.dispose(); requestFit = () => {}; recover = () => {}; inputListener.dispose(); privateClipboard.dispose(); motion?.cancel(); ro.disconnect(); for (const listener of [un, sync, connection]) void listener.then(f => f()); term.dispose(); };
  });

  $effect(() => {
    void st.theme; void st.themeRevision; void t?.controller.type; void t?.controller.agentId; void st.fontSize; void active;
    if (term) { applyLook(); if (active) app.timeout(() => requestFit(), 0); }
  });
</script>

<div class="host" data-session={session} data-terminal-ready={renderReady} aria-busy={!renderReady} bind:this={host} hidden={!active}></div>

{#if active && syncSlow && st.backendOnline}
  <div class="terminal-sync" role="status"><span>{app.t('terminal.syncing')}</span><button class="btn" onclick={recover}>{app.t('terminal.restore')}</button></div>
{/if}

<style>
  .terminal-sync { position: absolute; z-index: 20; right: 24px; top: var(--term-top, 44px); display: flex; gap: 12px; align-items: center; padding: 10px 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--surface); color: var(--muted); font-size: 12px; }
  /* Settings is a modal, so opening it must not resize the underlying shell.
     Real pane changes commit the grid once; never animate a PTY's dimensions. */
  .host { position: absolute; inset: var(--term-top, 44px) 16px var(--term-bottom, 26px) 16px; padding: 0; }
</style>
