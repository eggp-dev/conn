<script lang="ts">
  import { DOCK_MOTION_MS, dockOffsetFrames, reducedMotion } from "../lib/motion";
  import { appShortcut, shortcutKey } from "../lib/shortcuts";
  import { onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { renderedViewportFrame, rasterizeSurface, canPublishSurface, surfaceChannel } from "../lib/surface";
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
  const surfaceId = crypto.randomUUID();
  let refreshSurface = () => {};
  let invalidateSurface = () => {};
  let windowFocused = $state(document.hasFocus());
  let documentVisible = $state(document.visibilityState === "visible");
  const obscured = $derived(st.settingsOpen || st.paletteOpen || st.centerOpen || st.timelineOpen || st.menuOpen || st.sharingOpen);
  const displayable = $derived(canPublishSurface({ active, attended: t?.attended ?? false, shared: t?.shared ?? false, focused: windowFocused, documentVisible, obscured, ready: t?.statusReady ?? false }));

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
    const th = THEMES[st.theme] ?? THEMES.midnight;
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
    term = new Terminal({ logLevel: "off", fontFamily: '"SF Mono", "JetBrains Mono", Menlo, monospace', fontSize: st.fontSize, cursorBlink: true, allowProposedApi: true, scrollback: 5000, macOptionIsMeta: true });
    // Consume clipboard escape sequences before any private output is attached.
    // Returning true prevents fall-through to built-in or future addon handlers.
    const privateClipboard = term.parser.registerOscHandler(52, () => true);
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
      if (e.key === "Escape" && (st.settingsOpen || st.timelineOpen || st.centerOpen || st.paletteOpen || tab(session)?.handback)) return false;
      const key = shortcutKey(e);
      if (appShortcut(e) && ["k", ",", "j", "t", "w", "Enter", "[", "]", "ArrowRight", "ArrowLeft"].includes(key)) return false;
      if (appShortcut(e) && /^[1-9]$/.test(key)) return false;
      return true;
    });
    const inputListener = observeTerminalInput(term, (data, origin) => {
      const tb = t;
      if (origin === "human") tb.humanInputRevision++;
      if (origin === "human" && st.completionOpen) { st.completionOpen = false; void cmd("completion_cancel", { session }).catch(() => {}); }
      if (origin === "terminal") {
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
      if (st.paletteOpen || st.settingsOpen || st.sharingOpen) return;
      if (!!tb.shared && tb.proposal?.ready) {
        if (data === "\r") { cmd("accept_proposal", { session, proposalId: tb.proposal.id }); return; }
        if (data === "\x1b") { cmd("reject_proposal", { session, proposalId: tb.proposal.id }); return; }
      }
      if (!!tb.shared && tb.approval) {
        const map: Record<string, string> = { a: "grant", y: "grant", d: "deny", n: "deny", "\x1b": "deny", A: "allow_session" };
        const d = data === "A" && tb.reviewRequired ? undefined : map[data];
        if (d) { cmd("approve", { session, approvalId: tb.approval.id, decision: d }); return; }
      }
      if (!!tb.shared && tb.grace) {
        if (data === "\r") { cmd("execute_now", { session, execId: tb.grace.execId }); return; }
        if (data === "\x1b") { cmd("cancel_exec", { session, execId: tb.grace.execId }); return; }
      }
      if (!tb.shared) tb.externalInputAvailable = false;
      cmd("input", { session, data });
    });
    const channel = surfaceChannel(cmd, session, surfaceId);
    let mounted = true;
    let revision = 0;
    let lastFrameContent = "";
    let cachedRasterKey = "";
    let cachedRaster: Awaited<ReturnType<typeof rasterizeSurface>> = null;
    let outputSeq = t.outputSeq;
    let generation = t.surfaceGeneration;
    let pendingWrites = 0;
    let publishing = false;
    let scheduled = 0;
    let invalidated = true;
    let moving = false;
    let layoutRevision = 0;
    let visualChangedAt = performance.now();
    let settleTimer: ReturnType<typeof setTimeout> | undefined;
    const invalidate = () => {
      if (scheduled) { cancelAnimationFrame(scheduled); scheduled = 0; }
      if (!invalidated) { invalidated = true; t.surfaceAvailable = false; void channel.invalidate(); }
    };
    invalidateSurface = invalidate;
    const publish = async () => {
      scheduled = 0;
      if (!mounted || !displayable || pendingWrites || moving) { invalidate(); return; }
      // Coalesce a typing/output burst before cloning the rendered cells.
      // Core already rejects snapshots whose output sequence is not current.
      const quietFor = performance.now() - visualChangedAt;
      if (quietFor < 60) { clearTimeout(settleTimer); settleTimer = setTimeout(refreshSurface, 60 - quietFor); return; }
      const screen = host.querySelector<HTMLElement>(".xterm-screen");
      if (!screen || !screen.getClientRects().length) { invalidate(); return; }
      const r = screen.getBoundingClientRect();
      if (r.left < 0 || r.top < 0 || r.right > innerWidth || r.bottom > innerHeight) { invalidate(); return; }
      const frame = renderedViewportFrame(term, host, { surfaceId, generation: t.surfaceGeneration, revision, outputSeq });
      if (!frame) { invalidate(); return; }
      const content = JSON.stringify([frame.generation,frame.outputSeq,frame.rows,frame.cols,frame.cursor,frame.screen,frame.alternateScreen]);
      const frameLayout = layoutRevision;
      publishing = true;
      const look = () => JSON.stringify([term.options.fontSize,term.options.fontFamily,term.options.theme,term.getSelectionPosition(),screen.clientWidth,screen.clientHeight]);
      const currentLook = look();
      const rasterKey = content + currentLook;
      if (cachedRasterKey !== rasterKey) { cachedRaster = await rasterizeSurface(screen); cachedRasterKey = rasterKey; }
      if (!mounted || !displayable || moving || pendingWrites || currentLook !== look() || frame.generation !== t.surfaceGeneration || frame.outputSeq !== outputSeq) { publishing = false; refreshSurface(); return; }
      if (cachedRaster) frame.image = cachedRaster; else frame.imageUnavailable = true;
      const presentation = rasterKey + (cachedRaster ? "image" : "text");
      if (presentation !== lastFrameContent) { lastFrameContent = presentation; revision++; }
      frame.revision = revision;
      invalidated = false;
      void channel.publish(frame).then(result => {
        if (result && mounted && displayable && !moving && frameLayout === layoutRevision && frame.generation === t.surfaceGeneration && frame.outputSeq === outputSeq) { t.surfaceAvailable = true; t.surfacePublishedSerial++; }
      }).catch(() => {}).finally(() => { publishing = false; });
    };
    refreshSurface = () => {
      if (!displayable) { invalidate(); return; }
      if (!scheduled && !publishing) scheduled = requestAnimationFrame(() => { scheduled = requestAnimationFrame(publish); });
    };
    term.onRender(() => { measure(); refreshSurface(); });
    term.onCursorMove(() => { measure(); refreshSurface(); });
    const viewportChanged = () => { visualChangedAt = performance.now(); invalidate(); refreshSurface(); };
    term.onScroll(viewportChanged);
    term.onSelectionChange(viewportChanged);
    const visibility = () => { windowFocused = document.hasFocus(); documentVisible = document.visibilityState === "visible"; if (!windowFocused || !documentVisible) invalidate(); else refreshSurface(); };
    window.addEventListener("focus", visibility); window.addEventListener("blur", visibility);
    document.addEventListener("visibilitychange", visibility);
    const heartbeat = setInterval(refreshSurface, 750);
    let previousBottom = parseFloat(getComputedStyle(host).bottom);
    let previousHeight = host.clientHeight;
    let motion: Animation | undefined;
    const ro = new ResizeObserver(() => {
      const bottom = parseFloat(getComputedStyle(host).bottom);
      const height = host.clientHeight;
      const dockChanged = bottom !== previousBottom;
      const delta = previousHeight - height;
      previousBottom = bottom;
      previousHeight = height;
      if (!active) return;
      layoutRevision++;
      // Commit the final terminal grid once. Animate its visual offset, not
      // its height: otherwise every frame resizes the PTY and redraws the shell.
      const transform = getComputedStyle(host).transform;
      const offset = transform === "none" ? 0 : new DOMMatrixReadOnly(transform).m42;
      motion?.cancel(); moving = false;
      const rowHeight = host.querySelector<HTMLElement>(".xterm-screen")!.clientHeight / term.rows;
      const before = term.buffer.active;
      const cursorBefore = before.baseY + before.cursorY - before.viewportY;
      fit.fit();
      const after = term.buffer.active;
      const cursorDelta = (cursorBefore - (after.baseY + after.cursorY - after.viewportY)) * rowHeight;
      if (dockChanged && delta && cursorDelta && !reducedMotion()) {
        moving = true; invalidate();
        host.style.willChange = "transform";
        motion = host.animate(dockOffsetFrames(cursorDelta + offset), {
          duration: DOCK_MOTION_MS, easing: "linear",
        });
        motion.onfinish = () => { moving = false; host.style.willChange = ""; measure(); refreshSurface(); };
      } else { host.style.willChange = ""; }
    });
    ro.observe(host);
    const un = onOutput((p) => {
      if (!mounted || p.session !== session) return;
      visualChangedAt = performance.now();
      pendingWrites++;
      // No backing-input bytes are retained; only parsed and displayed output is exported.
      term.write(b64ToBytes(p.data), () => {
        pendingWrites--;
        if (!mounted) return;
        if (p.generation >= generation) { generation = p.generation; outputSeq = p.outputSeq; }
        t.outputSeq = outputSeq;
        refreshSurface();
      });
    });
    un.then(() => { if (mounted) return cmd("attach_output", { session }); }).catch(() => {});
    return () => { mounted = false; clearInterval(heartbeat); clearTimeout(settleTimer); if (scheduled) cancelAnimationFrame(scheduled); window.removeEventListener("focus", visibility); window.removeEventListener("blur", visibility); document.removeEventListener("visibilitychange", visibility); void channel.dispose(); inputListener.dispose(); privateClipboard.dispose(); motion?.cancel(); ro.disconnect(); un.then((f) => f()); term.dispose(); };
  });

  $effect(() => { void displayable; void t?.surfaceGeneration; if (displayable) refreshSurface(); else invalidateSurface(); });

  $effect(() => {
    void st.theme; void st.themeRevision; void t?.controller.type; void t?.controller.agentId; void st.fontSize; void active;
    if (term) { applyLook(); if (active) setTimeout(() => { fit.fit(); measure(); }, 0); }
  });
</script>

<div class="host" bind:this={host} hidden={!active}></div>

<style>
  .host { position: absolute; inset: 44px var(--term-right, 16px) var(--term-bottom, 26px) 16px; padding: 0; transition: right .22s var(--ease); }
</style>
