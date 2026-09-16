<script lang="ts">
  import { appShortcut, shortcutKey } from "../lib/shortcuts";
  import { onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { observeTerminalInput } from "../lib/terminalInput";
  import { cmd, onOutput, b64ToBytes } from "../lib/bridge";
  import { st, tab } from "../lib/store.svelte";
  import { THEMES, applyTheme, agentColor } from "../lib/themes";

  let { session }: { session: string } = $props();
  let host: HTMLDivElement;
  let term: Terminal;
  let fit: FitAddon;
  const t = $derived(tab(session)!);
  const active = $derived(st.active === session);

  export function focus() { term?.focus(); }
  export function size() { return { rows: term?.rows ?? 24, cols: term?.cols ?? 80 }; }
  export function refit() { fit?.fit(); measure(); }

  function measure() {
    const screen = host?.querySelector<HTMLElement>(".xterm-screen");
    if (!screen || !term || !t) return;
    const w = screen.clientWidth / term.cols;
    const h = screen.clientHeight / term.rows;
    const b = term.buffer.active;
    const r = screen.getBoundingClientRect();
    const pr = (host.offsetParent as HTMLElement | null)?.getBoundingClientRect() ?? host.getBoundingClientRect();
    t.cursor = { x: r.left - pr.left + b.cursorX * w, y: r.top - pr.top + b.cursorY * h, w, h, row: b.cursorY };
  }

  function applyLook() {
    const th = THEMES[st.theme];
    const agent = t?.controller.type === "agent";
    const accent = agent ? agentColor(t.controller.agentId) : "#8b7cff";
    const xt = applyTheme(th, accent);
    if (agent) { xt.cursor = accent; xt.cursorAccent = th.tokens.bg; }
    term.options.theme = xt;
    term.options.cursorStyle = agent ? "underline" : "block";
    term.options.fontSize = st.fontSize;
    if (active) { fit.fit(); measure(); }
  }

  onMount(() => {
    term = new Terminal({ logLevel: t.externalPrivate ? "off" : "info", fontFamily: '"SF Mono", "JetBrains Mono", Menlo, monospace', fontSize: st.fontSize, cursorBlink: true, allowProposedApi: true, scrollback: 5000, macOptionIsMeta: true });
    // Consume clipboard escape sequences before any private output is attached.
    // Returning true prevents fall-through to built-in or future addon handlers.
    const privateClipboard = term.parser.registerOscHandler(52, () => t.externalPrivate);
    fit = new FitAddon();
    term.loadAddon(fit);
    term.open(host);
    // Register before the first fit: the PTY starts at 80x24, while xterm
    // immediately adopts the pane size. Missing this event desynchronizes wrapping.
    term.onResize(({ rows, cols }) => { cmd("resize", { session, rows, cols }); measure(); });
    applyLook();
    term.attachCustomKeyEventHandler((e) => {
      if (e.type !== "keydown") return true;
      // Escape closes an open overlay instead of reaching the shell.
      if (e.key === "Escape" && (st.settingsOpen || st.timelineOpen || st.centerOpen || st.paletteOpen)) return false;
      const key = shortcutKey(e);
      if (appShortcut(e) && ["k", ",", "j", "t", "w", "Enter", "[", "]", "ArrowRight", "ArrowLeft"].includes(key)) return false;
      if (appShortcut(e) && /^[1-9]$/.test(key)) return false;
      return true;
    });
    const inputListener = observeTerminalInput(term, (data, origin) => {
      const tb = t;
      if (tb.externalPrivate && origin === "terminal") {
        // The first child output can precede the final started-status refresh.
        cmd("terminal_response", { session, data });
        return;
      }
      // During preparation, genuine input cancels the pending launch. The native
      // boundary discards these bytes instead of sending them to a future child.
      if (tb.externalStarting) {
        cmd("input", { session, data });
        return;
      }
      if (st.paletteOpen || st.settingsOpen) return;
      if (!tb.externalPrivate && tb.proposal?.ready) {
        if (data === "\r") { cmd("accept_proposal", { session, proposalId: tb.proposal.id }); return; }
        if (data === "\x1b") { cmd("reject_proposal", { session, proposalId: tb.proposal.id }); return; }
      }
      if (!tb.externalPrivate && tb.approval) {
        const map: Record<string, string> = { a: "grant", y: "grant", d: "deny", n: "deny", "\x1b": "deny", A: "allow_session" };
        const d = data === "A" && tb.reviewRequired ? undefined : map[data];
        if (d) { cmd("approve", { session, approvalId: tb.approval.id, decision: d }); return; }
      }
      if (!tb.externalPrivate && tb.grace) {
        if (data === "\r") { cmd("execute_now", { session, execId: tb.grace.execId }); return; }
        if (data === "\x1b") { cmd("cancel_exec", { session, execId: tb.grace.execId }); return; }
      }
      if (tb.externalPrivate) tb.externalInputAvailable = false;
      cmd("input", { session, data });
    });
    term.onRender(measure);
    term.onCursorMove(measure);
    const ro = new ResizeObserver(() => { if (active) fit.fit(); });
    ro.observe(host);
    let mounted = true;
    const un = onOutput((p) => { if (mounted && p.session === session) term.write(b64ToBytes(p.data)); });
    un.then(() => { if (mounted) return cmd("attach_output", { session }); }).catch(() => {});
    return () => { mounted = false; inputListener.dispose(); privateClipboard.dispose(); ro.disconnect(); un.then((f) => f()); term.dispose(); };
  });

  $effect(() => {
    void st.theme; void t?.controller.type; void t?.controller.agentId; void st.fontSize; void active;
    if (term) { applyLook(); if (active) setTimeout(() => { fit.fit(); measure(); term.focus(); }, 0); }
  });
</script>

<div class="host" bind:this={host} hidden={!active}></div>

<style>
  .host { position: absolute; inset: 44px var(--term-right, 16px) 26px 16px; padding: 0; transition: right .22s var(--ease); }
</style>
