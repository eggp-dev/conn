<script lang="ts">
  import { appShortcut, shortcutKey, shortcutLabel } from "./lib/shortcuts";
  import { refreshProfiles } from "./lib/profiles.svelte";
  import { onMount, tick } from "svelte";
  import Term from "./components/Term.svelte";
  import TabStrip from "./components/TabStrip.svelte";
  import Island from "./components/Island.svelte";
  import Palette from "./components/Palette.svelte";
  import type { Action, Parser } from "./components/Palette.svelte";
  import Center from "./components/Center.svelte";
  import SettingsSheet from "./components/SettingsSheet.svelte";
  import Ghost from "./components/Ghost.svelte";
  import Hold from "./components/Hold.svelte";
  import GraceBar from "./components/GraceBar.svelte";
  import ControlRequest from "./components/ControlRequest.svelte";
  import HandbackChip from "./components/HandbackChip.svelte";
  import LeaveChip from "./components/LeaveChip.svelte";
  import Timeline from "./components/Timeline.svelte";
  import Toasts from "./components/Toasts.svelte";
  import HandoffWash from "./components/HandoffWash.svelte";
  import { newTimeline, recordTimeline, importSavedActivity } from "./lib/timeline";
  import { invoke, listen } from "./lib/transport";
  import { cmd, changeMode, onEvent, onTabOpened, log, type Pacing } from "./lib/bridge";
  import { st, cur, tab, tabIndex, newTab, toast, announce, type TabState } from "./lib/store.svelte";
  import { THEMES, agentColor } from "./lib/themes";
  import { t as tr, tabName, fmtMs, setLang, LANGS, i18n } from "./lib/i18n.svelte";

  let terms = $state<Record<string, Term>>({});
  let chip = $state<HandbackChip>();
  let leave = $state<LeaveChip>();
  let washSeq = 0;
  const handbackTimers: Record<string, number> = {};

  const focusTerm = () => setTimeout(() => terms[st.active]?.focus(), 0);

  function wash(t: TabState, color: string, out: boolean) {
    const c = t.cursor;
    t.wash = { x: c.x + c.w / 2, y: c.y + c.h / 2, color, out, key: ++washSeq };
    setTimeout(() => { if (t.wash?.key === washSeq) t.wash = null; }, 1300);
  }
  function setController(t: TabState, kind: "human" | "agent", agentId?: string) {
    const was = t.controller.type;
    t.controller = { type: kind, agentId };
    if (kind === "agent") { t.handback = false; if (was !== "agent" && t.id === st.active) wash(t, agentColor(agentId), false); }
    else { t.grace = null; t.proposal = null; if (was === "agent" && t.id === st.active) wash(t, agentColor(t.lastAgent?.agentId), true); }
  }
  function showHandback(t: TabState) {
    if (!t.lastAgent) return;
    t.handback = true;
    if (handbackTimers[t.id]) clearTimeout(handbackTimers[t.id]);
    handbackTimers[t.id] = window.setTimeout(() => (t.handback = false), 60_000);
  }
  const typingTimers: Record<string, number> = {};

  // ---- tabs ----
  async function syncStatus(id: string) {
    const t = tab(id); if (!t) return;
    const s = await invoke<any>("status", { session: id });
    t.externalPrivate = s.externalPrivate === true;
    t.externalStarting = s.externalStarting === true;
    t.externalInputAvailable = s.externalInputAvailable === true;
    if (t.externalPrivate) {
      t.profileId = s.profileId ?? null; t.profileName = s.profileName ?? null;
      t.title = s.profileName || tr("private.title");
      t.processAlive = !!s.processAlive; t.attended = !!s.attended;
      t.timeline = newTimeline(true); t.controller = { type: "human" };
      t.agents = []; t.lastAgent = null; t.openedBy = null; t.entrustedTo = null;
      t.approval = null; t.ctlReq = null; t.proposal = null; t.grace = null; t.attention = null;
      t.handback = false; t.typing = false; t.paused = false;
      t.statusReady = true;
      return;
    }
    if (!t.profileId && s.profileName) t.title = s.profileName;
    t.profileId = s.profileId ?? null; t.profileName = s.profileName ?? null; t.reviewRequired = !!s.reviewRequired;
    t.mode = s.mode; t.effectiveMode = s.effectiveMode ?? s.mode; t.gate = !!s.controlGate; t.allows = s.sessionAllows ?? []; t.mask = s.affordanceMask ?? null; t.agents = s.connectedAgents ?? [];
    t.pacing = s.pacing; t.attended = !!s.attended; t.entrustedTo = s.entrustedTo ?? null; t.processAlive = !!s.processAlive;
    t.attention = s.attentionRequest ? { agentId: s.attentionRequest.agentId, reason: s.attentionRequest.reason } : null;
    t.openedBy = s.openedBy ?? null;
    if (s.lastAgent) t.lastAgent = { agentId: s.lastAgent.agentId, connected: s.lastAgent.connected, lastCmd: s.lastAgent.lastCmd ?? undefined };
    t.controller = s.controller.type === "agent" ? { type: "agent", agentId: s.controller.agentId } : { type: "human" };
    t.approval = s.pending?.length ? { id: s.pending[0].id, agentId: s.pending[0].agentId, cmd: s.pending[0].cmd, label: s.pending[0].label, at: Date.now(), intent: s.pending[0].intent ?? undefined, analysis: s.pending[0].analysis ?? undefined } : null;
    t.proposal = s.proposal ? { id: s.proposal.proposalId, agentId: s.proposal.agentId, text: s.proposal.text, ready: s.proposal.state === "ready", intent: s.proposal.intent ?? undefined } : null;
    t.ctlReq = s.controlRequests?.length ? { id: s.controlRequests[0].requestId, agentId: s.controlRequests[0].agentId, reason: s.controlRequests[0].reason, originalRequest: s.controlRequests[0].originalRequest } : null;
    t.statusReady = true;
  }
  function addTab(id: string) {
    st.tabSeq += 1;
    st.tabs[id] = newTab(id, st.tabSeq);
    st.order.push(id);
  }
  async function select(id: string) {
    if (id === st.active || !tab(id)) return;
    const from = tab(st.active);
    st.active = id;
    st.centerOpen = false; st.timelineOpen = false; st.leaveChip = null;
    await invoke("attend", { session: id });
    // Leaving a tab where an agent holds the conn pauses it; offer to entrust.
    if (from && !from.externalPrivate && from.controller.type === "agent" && !from.entrustedTo) {
      from.paused = true;
      st.leaveChip = { session: from.id, agentId: from.controller.agentId ?? "agent", until: Date.now() + 8000 };
      setTimeout(() => { if (st.leaveChip?.session === from.id) st.leaveChip = null; }, 8000);
    }
    const to = tab(id)!; to.paused = false; to.attention = null;
    terms[id]?.refit(); focusTerm();
  }
  async function openTab(profileId?: string) {
    if (st.externalPending) return;
    try {
    const sz = terms[st.active]?.size() ?? { rows: 24, cols: 80 };
    const id = await invoke<string>("open_tab", { rows: sz.rows, cols: sz.cols, profileId: profileId ?? null });
    addTab(id);
    await syncStatus(id);
    await select(id);
    st.booted = true;
    } catch(e) { toast(String(e), "danger"); }
  }
  async function closeTab(id: string) {
    if (st.order.length <= 1) { toast(tr("tab.last"), "warn"); return; }
    const idx = st.order.indexOf(id);
    await invoke("close_tab", { session: id });
    st.order = st.order.filter((x) => x !== id);
    delete st.tabs[id];
    delete terms[id];
    if (st.active === id) { const next = st.order[Math.max(0, idx - 1)]; st.active = next; await invoke("attend", { session: next }); terms[next]?.refit(); focusTerm(); }
  }
  const needsYou = (tb: TabState | undefined) => !!tb && !!(tb.approval || tb.attention || tb.ctlReq || tb.proposal?.ready);
  const nextNeedingAttention = () => st.order.find((id) => id !== st.active && needsYou(tab(id)));

  const TOOL_PRESETS: [string, string[] | null][] = [
    ["s.preset.observe", ["snapshot", "request_attention", "check_approval", "switch_tab"]],
    ["s.preset.notabs", ["snapshot", "request_control", "type", "send_key", "interrupt", "release_control", "check_approval", "request_attention", "switch_tab"]],
    ["s.preset.all", null],
  ];
  const maskIs = (p: string[] | null) => p === null ? cur().mask === null : !!cur().mask && p.length === cur().mask!.length && p.every((a) => cur().mask!.includes(a));
  const openSettings = (tab: typeof st.settingsTab) => { st.settingsTab = tab; st.settingsOpen = true; st.centerOpen = false; };
  const setTheme = (id: string) => { st.theme = id as any; localStorage.setItem("ss:theme", id); };

  // State-aware: labels carry the agent's name and current values; `now` items are
  // the things to answer right now and sort first.
  const actions = $derived.by((): Action[] => {
    const c = cur();
    const holder = c.controller.type === "agent" ? c.controller.agentId ?? "agent" : c.lastAgent?.agentId ?? "agent";
    const lang = i18n.lang; void lang;
    const all: Action[] = [
      { id: "take", group: "conn", now: c.controller.type === "agent", label: tr("a.take"), hint: tr("a.take.hint"), aliases: ["take", "revoke", "회수", "뺏기", "제어권"], run: () => cmd("take"), when: () => cur().controller.type === "agent" },
      { id: "handback", group: "conn", now: c.handback, label: tr("a.handback", { agent: holder }), keys: shortcutLabel("⌘⏎"), aliases: ["hand back", "give back", "되돌려주기", "다시"], run: () => chip?.handBack(), when: () => !!cur().lastAgent && cur().controller.type === "human" },
      { id: "entrust", group: "conn", now: c.controller.type === "agent", label: tr("a.entrust", { agent: holder }), hint: tr("a.entrust.hint"), aliases: ["entrust", "맡기기", "맡김", "leave"], run: () => cmd("entrust").then(() => toast(tr("entrust.set"), "ok")).catch((e) => toast(String(e), "warn")), when: () => !!cur().lastAgent && !cur().entrustedTo },
      { id: "approve", group: "approval", now: true, label: tr("a.approve", { label: c.approval?.label ?? "" }), hint: c.approval?.cmd, keys: "a", aliases: ["approve", "grant", "승인"], run: () => cur().approval && cmd("approve", { approvalId: cur().approval!.id, decision: "grant" }), when: () => !!cur().approval },
      { id: "deny", group: "approval", now: true, danger: true, label: tr("a.deny", { label: c.approval?.label ?? "" }), keys: "d", aliases: ["deny", "거부"], run: () => cur().approval && cmd("approve", { approvalId: cur().approval!.id, decision: "deny" }), when: () => !!cur().approval },
      { id: "allow-session", group: "approval", now: true, label: tr("a.allow_session", { label: c.approval?.label ?? "" }), keys: "A", aliases: ["allow", "session", "세션 허용"], run: () => cur().approval && cmd("approve", { approvalId: cur().approval!.id, decision: "allow_session" }), when: () => !!cur().approval && !cur().reviewRequired },
      { id: "ctl-grant", group: "conn", now: true, label: tr("a.ctl.grant", { agent: c.ctlReq?.agentId ?? "" }), hint: c.ctlReq?.reason, aliases: ["allow", "grant", "허용"], run: () => cur().ctlReq && cmd("decide_control", { requestId: cur().ctlReq!.id, grant: true }), when: () => !!cur().ctlReq },
      { id: "ctl-deny", group: "conn", now: true, danger: true, label: tr("a.ctl.deny", { agent: c.ctlReq?.agentId ?? "" }), aliases: ["deny", "거부"], run: () => cur().ctlReq && cmd("decide_control", { requestId: cur().ctlReq!.id, grant: false }), when: () => !!cur().ctlReq },
      ...st.order.map((id, i) => {
        const tb = tab(id)!;
        const flags = [tb.approval ? tr("badge.approval") : "", tb.proposal?.ready ? tr("badge.proposal", { agent: tb.proposal.agentId }) : "", tb.attention ? tr("badge.knock", { agent: tb.attention.agentId }) : "", tb.entrustedTo ? tr("badge.entrusted") : tb.paused ? tr("badge.paused") : "", tb.controller.type === "agent" ? tr("conn.agent", { agent: tb.controller.agentId ?? "" }) : ""].filter(Boolean).join(" · ");
        return { id: `tab-${id}`, group: "tabs", now: needsYou(tb) && id !== st.active, label: tr("a.tab.go", { name: `${i + 1} · ${tb.title}` }), hint: flags || undefined, keys: i < 9 ? shortcutLabel(`⌘${i + 1}`) : undefined, aliases: ["tab", "탭", tb.title, tb.openedBy ?? ""], active: () => st.active === id, run: () => select(id) } as Action;
      }),
      { id: "tab-new", group: "tabs", label: tr("a.tab.new"), keys: shortcutLabel("⌘T"), aliases: ["new tab", "새 탭"], run: openTab },
      { id: "tab-attn", group: "tabs", label: tr("a.tab.attn"), keys: shortcutLabel("⌘⌥→"), aliases: ["attention", "주의", "next"], run: () => { const n = nextNeedingAttention(); if (n) select(n); else toast(tr("tab.none.waiting")); } },
      { id: "tab-close", group: "tabs", label: tr("a.tab.close"), keys: shortcutLabel("⌘W"), aliases: ["close tab", "탭 닫기"], run: () => closeTab(st.active), when: () => st.order.length > 1 },
      ...(["observe", "copilot", "autopilot"] as const).map((m) => ({ id: `mode-${m}`, group: "mode", label: tr("a.mode.set", { mode: tr(`mode.${m}`) }), hint: tr(`mode.${m}.desc`), aliases: ["mode", "모드", m], active: () => cur().mode === m, run: () => changeMode(m) })),
      { id: "gate", group: "mode", label: tr("a.gate"), aliases: ["gate", "ask", "묻기", "게이트"], value: () => (cur().gate ? tr("on") : tr("off")), run: () => cmd("set_control_gate", { ask: !cur().gate }) },
      ...TOOL_PRESETS.map(([k, p]) => ({ id: `tools-${k}`, group: "agents", label: tr("a.tools.preset", { name: tr(k) }), aliases: ["tools", "도구", "affordance"], active: () => maskIs(p), run: () => cmd("set_affordances", { allow: p }) })),
      { id: "grace", group: "pacing", label: tr("a.grace"), hint: tr("a.grace.hint"), aliases: ["grace", "유예", "delay"], value: () => fmtMs(cur().pacing.enterGraceMs), run: () => (st.centerOpen = true) },
      { id: "interval", group: "pacing", label: tr("a.interval"), aliases: ["interval", "typing", "간격", "throttle"], value: () => `${cur().pacing.minWriteIntervalMs}ms`, run: () => openSettings("pacing") },
      { id: "lease", group: "pacing", label: tr("a.lease"), aliases: ["lease", "ttl"], value: () => `${cur().pacing.leaseTtlSecs}s`, run: () => openSettings("pacing") },
      ...Object.values(THEMES).map((th) => ({ id: `theme-${th.id}`, group: "theme", label: tr("a.theme", { name: th.name }), hint: tr(th.blurb), aliases: ["theme", "테마", th.id], active: () => st.theme === th.id, run: () => setTheme(th.id) })),
      { id: "effects", group: "theme", label: tr("a.effects"), aliases: ["effects", "효과", "wash"], value: () => (st.effects ? tr("on") : tr("off")), run: () => { st.effects = !st.effects; localStorage.setItem("ss:effects", st.effects ? "1" : "0"); } },
      { id: "font", group: "theme", label: tr("a.font"), aliases: ["font", "글꼴", "size"], value: () => `${st.fontSize}px`, run: () => openSettings("appearance") },
      { id: "timeline", group: "view", label: tr("a.timeline"), keys: shortcutLabel("⌘J"), aliases: ["timeline", "타임라인", "history"], run: () => (st.timelineOpen = !st.timelineOpen) },
      { id: "center", group: "view", label: tr("a.center"), aliases: ["center", "센터", "island", "quick"], run: () => (st.centerOpen = true) },
      ...LANGS.map((l) => ({ id: `lang-${l.id}`, group: "language", label: tr("a.lang", { name: l.name }), aliases: ["language", "언어", l.id, l.name], active: () => i18n.lang === l.id, run: () => setLang(l.id) })),
      ...(["profiles", "agents", "automation", "policy", "pacing", "appearance", "diagnostics"] as const).map((tb) => ({ id: `settings-${tb}`, group: "settings", label: tr("a.settings", { tab: tr(`s.nav.${tb}`) }), keys: shortcutLabel("⌘,"), aliases: ["settings", "설정", tb], run: () => openSettings(tb) })),
      { id: "connect-agent", group: "setup", label: tr("s.connect.copy"), aliases: ["connect", "mcp", "codex", "cli", "agent", "에이전트", "연결", "설정 복사"], run: () => openSettings("agents") },
    ];
    return c.externalPrivate ? all.filter(action => !["conn", "approval", "mode", "agents", "pacing", "setup"].includes(action.group)
      && !["center", "timeline", "settings-agents", "settings-policy", "settings-pacing"].includes(action.id)) : all;
  });

  // Typed arguments: "grace 3s", "유예 3초", "tab 2", "font 14", "theme paper".
  const num = (q: string, re: RegExp) => { const m = q.match(re); return m ? { n: parseFloat(m[1]), unit: (m[2] ?? "").toLowerCase() } : null; };
  const parsers: Parser[] = [
    (q) => { if (cur().externalPrivate) return []; const m = num(q, /^(?:grace|유예|delay)\s*(\d+(?:\.\d+)?)\s*(ms|s|초)?$/i); if (!m) return []; const ms = m.unit === "ms" ? m.n : m.n * 1000; return [{ id: "set-grace", group: "pacing", label: tr("a.grace.set", { v: fmtMs(ms) }), run: () => cmd("set_pacing", { patch: { enterGraceMs: Math.round(ms) } }) }]; },
    (q) => { if (cur().externalPrivate) return []; const m = num(q, /^(?:interval|typing|간격)\s*(\d+)\s*(ms)?$/i); if (!m) return []; return [{ id: "set-interval", group: "pacing", label: tr("a.interval.set", { v: `${m.n}ms` }), run: () => cmd("set_pacing", { patch: { minWriteIntervalMs: Math.round(m.n) } }) }]; },
    (q) => { if (cur().externalPrivate) return []; const m = num(q, /^(?:lease|ttl)\s*(\d+)\s*(s|초)?$/i); if (!m) return []; return [{ id: "set-lease", group: "pacing", label: tr("a.lease.set", { v: `${m.n}s` }), run: () => cmd("set_pacing", { patch: { leaseTtlSecs: Math.round(m.n) } }) }]; },
    (q) => { const m = num(q, /^(?:font|글꼴|size)\s*(\d+)\s*(px)?$/i); if (!m) return []; return [{ id: "set-font", group: "theme", label: tr("a.font.set", { v: m.n }), run: () => { st.fontSize = m.n; localStorage.setItem("ss:fontSize", String(m.n)); } }]; },
    (q) => { const m = num(q, /^(?:tab|탭)\s*(\d+)$/i); if (!m) return []; const id = st.order[m.n - 1]; if (!id) return []; return [{ id: "go-tab", group: "tabs", label: tr("a.tab.go", { name: `${m.n} · ${tab(id)!.title}` }), run: () => select(id) }]; },
    (q) => { const m = q.match(/^(?:theme|테마)\s+(\w+)$/i); if (!m) return []; const th = Object.values(THEMES).find((x) => x.id.startsWith(m[1].toLowerCase()) || x.name.toLowerCase().startsWith(m[1].toLowerCase())); if (!th) return []; return [{ id: "set-theme", group: "theme", label: tr("a.theme", { name: th.name }), run: () => setTheme(th.id) }]; },
  ];

  function onKey(e: KeyboardEvent) {
    if (!appShortcut(e)) {
      if (e.key === "Escape" && (st.paletteOpen || st.settingsOpen || st.timelineOpen || st.centerOpen)) { st.paletteOpen = false; st.settingsOpen = false; st.timelineOpen = false; st.centerOpen = false; focusTerm(); }
      return;
    }
    const key = shortcutKey(e);
    if (key === "k") { e.preventDefault(); st.centerOpen = false; st.paletteOpen = !st.paletteOpen; if (!st.paletteOpen) focusTerm(); }
    else if (key === ",") { e.preventDefault(); st.settingsOpen = !st.settingsOpen; if (!st.settingsOpen) focusTerm(); }
    else if (key === "j") { e.preventDefault(); if (cur().externalPrivate) return; st.timelineOpen = !st.timelineOpen; if (!st.timelineOpen) focusTerm(); }
    else if (key === "t") { e.preventDefault(); openTab(); }
    else if (key === "w") { e.preventDefault(); closeTab(st.active); }
    else if (key === "Enter") { e.preventDefault(); if (cur().externalPrivate) return; if (st.leaveChip) leave?.entrust(); else if (cur().handback) chip?.handBack(); }
    else if (/^[1-9]$/.test(key)) { e.preventDefault(); const id = st.order[Number(key) - 1]; if (id) select(id); }
    else if (e.shiftKey && (key === "[" || key === "]" || key === "{" || key === "}")) { e.preventDefault(); const i = st.order.indexOf(st.active); const d = key === "[" || key === "{" ? -1 : 1; select(st.order[(i + d + st.order.length) % st.order.length]); }
    else if (e.altKey && key === "ArrowRight") { e.preventDefault(); const n = nextNeedingAttention(); if (n) select(n); }
  }

  $effect(() => { void st.settingsOpen; const h = setTimeout(() => terms[st.active]?.refit(), 250); return () => clearTimeout(h); });

  $effect(() => {
    if (!st.followSystem) return;
    const mq = matchMedia("(prefers-color-scheme: light)");
    const apply = () => (st.theme = mq.matches ? "paper" : "midnight");
    apply(); mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  });

  onMount(async () => {
    const connectionListener = listen<{ connected: boolean }>("ss:connection", ({payload}) => { st.backendOnline = payload.connected; });
    const tabListener = onTabOpened(async (p) => {
      // Agent tabs stay in the background; an explicit external-launch request may select its new tab.
      if (!tab(p.session)) addTab(p.session);
      try { await syncStatus(p.session); } catch { return; }
      // A prepared tab can be aborted while its initial status is in flight.
      if (!tab(p.session)) return;
      st.externalPending = st.order.some(id => tab(id)?.externalStarting);
      if (p.focus) { st.active = p.session; st.centerOpen = false; st.timelineOpen = false; focusTerm(); }
    });
    const abortListener = listen<{ session: string }>("ss:tab_aborted", async ({payload}) => {
      const id = payload.session;
      const index = st.order.indexOf(id);
      if (index < 0) return;
      st.order = st.order.filter(other => other !== id);
      delete st.tabs[id]; delete terms[id];
      st.externalPending = st.order.some(other => tab(other)?.externalStarting);
      if (st.active === id) {
        st.active = st.order[Math.max(0, index - 1)] ?? "";
        st.centerOpen = false; st.timelineOpen = false;
        if (st.active) { try { await invoke("attend", {session:st.active}); } catch {} }
        focusTerm();
      }
    });
    const eventListener = onEvent((ev) => {
      const t = ev.session ? tab(ev.session) : cur();
      if (!t || !t.statusReady) return;
      if (t.externalPrivate || ev.externalPrivate) {
        if (ev.event === "process_exited") t.processAlive = false;
        if (ev.event === "attention_changed") t.attended = !!ev.attended;
        return;
      }
      recordTimeline(t.timeline, ev);
      const here = t.id === st.active;
      const where = here ? "" : tabName(tabIndex(t.id));
      const W = (k: string, p: Record<string, string | number> = {}) => (where ? tr(`${k}.where`, { ...p, where }) : tr(k, p));
      const sfx = where ? ` · ${where}` : "";
      switch (ev.event) {
        case "control_granted": {
          const prev = t.lastAgent; t.lastAgent = { agentId: ev.agentId, connected: true, lastCmd: prev && prev.agentId === ev.agentId ? prev.lastCmd : undefined };
          setController(t, "agent", ev.agentId); t.ctlReq = null;

          announce(W("conn.agent", { agent: ev.agentId }), agentColor(ev.agentId), "conn", 2600, ev.reason);
          break;
        }
        case "control_revoked":
          setController(t, "human");
          if (ev.reason === "human_input" || ev.reason === "taken") showHandback(t);

          {
            const a = { agent: ev.agentId };
            const why: Record<string, string> = { human_input: "", taken: "", released: tr("conn.released", a), expired: tr("conn.expired", a), disconnected: tr("conn.disconnected", a), process_exited: tr("conn.shell_exited") };
            announce(W("conn.yours"), agentColor(ev.agentId), "conn", 2600, why[ev.reason] ?? ev.reason);
          }
          break;
        case "control_handed_back": announce(tr("conn.again", { agent: ev.agentId }), agentColor(ev.agentId), "conn", 1800); break;
        case "control_requested": t.ctlReq = { id: ev.request.requestId, agentId: ev.request.agentId, reason: ev.request.reason, originalRequest: ev.request.originalRequest }; announce(W("conn.asks", { agent: ev.request.agentId }), agentColor(ev.request.agentId), "conn", 3000, ev.request.reason); break;
        case "control_request_resolved": if (t.ctlReq?.id === ev.requestId) t.ctlReq = null; break;
        case "attention_requested": t.attention = { agentId: ev.agentId, reason: ev.reason }; announce(tr("attention.asks", { agent: ev.agentId, where: tabName(tabIndex(t.id)) }), agentColor(ev.agentId), "attention", 3200, ev.reason); break;
        case "tab_opened":
          // An agent opened this tab. It starts unattended: the agent cannot see or
          // write until you go there (⌘⌥→) or entrust it. The tab knocks meanwhile.
          t.openedBy = ev.agentId; t.attention = { agentId: ev.agentId, reason: ev.reason };

          announce(tr("tab.opened.by", { agent: ev.agentId, where: tabName(tabIndex(t.id)) }), agentColor(ev.agentId), "attention", 3200, ev.reason ?? tr("tab.opened.hint"));
          break;
        case "agent_switched_tab":
          if (t.id === ev.to) {

            if (here) announce(tr("agent.moved.here", { agent: ev.agentId }), agentColor(ev.agentId), "info", 2200, tr("agent.moved.from", { where: tabName(tabIndex(ev.from)) }));
          } else if (here) {
            announce(tr("agent.moved.to", { agent: ev.agentId, where: tabName(tabIndex(ev.to)) }), agentColor(ev.agentId), "info", 2200);
          }
          break;
        case "attention_changed": t.attended = !!ev.attended; if (ev.attended) t.attention = null; break;
        case "entrusted": t.entrustedTo = ev.agentId ?? null; if (ev.agentId) { t.paused = false; } break;
        case "control_suspended": t.paused = true; break;
        case "control_resumed": t.paused = false; break;
        case "agent_input":
          t.typing = true; if (typingTimers[t.id]) clearTimeout(typingTimers[t.id]); typingTimers[t.id] = window.setTimeout(() => (t.typing = false), 500);
          break;
        case "proposal_changed": {
          const becameReady = ev.proposal.state === "ready" && !(t.proposal?.ready);
          t.proposal = { id: ev.proposal.proposalId, agentId: ev.proposal.agentId, text: ev.proposal.text, ready: ev.proposal.state === "ready", intent: ev.proposal.intent ?? undefined };
          // A proposal waiting on another tab is a knock: you decide there, it cannot run itself.
          if (becameReady && !here) announce(tr("proposal.waiting", { agent: ev.proposal.agentId, where }), agentColor(ev.proposal.agentId), "attention", 3200, ev.proposal.intent ?? ev.proposal.text);
          break;
        }
        case "proposal_resolved": {
          if (t.proposal?.id === ev.proposalId) t.proposal = null;
          if (ev.state === "executed") { if (t.lastAgent) t.lastAgent.lastCmd = ev.cmd; }
          if (ev.state === "denied") { toast(tr("policy.blocked", { cmd: ev.cmd }), "danger"); }
          if (ev.state === "rejected") { toast(tr("proposal.rejected"), "warn"); }
          break;
        }
        case "approval_requested":
          t.approval = { id: ev.request.id, agentId: ev.request.agentId, cmd: ev.request.cmd, label: ev.request.label, at: Date.now(), intent: ev.request.intent ?? undefined, analysis: ev.request.analysis ?? undefined };
          announce(W("approval.needed", { agent: ev.request.agentId }), "var(--warn)", "approval", 3000, `${ev.request.label}${ev.request.intent ? " · " + ev.request.intent : " · " + ev.request.cmd}`);
          break;
        case "approval_resolved":

          if (t.approval?.id === ev.approvalId) t.approval = null;
          toast(`${ev.state} (${ev.by})${sfx}`, ev.state === "granted" ? "ok" : "danger");
          invoke<any>("status", { session: t.id }).then((s) => (t.allows = s.sessionAllows ?? []));
          break;
        case "session_allows_changed": t.allows = ev.allows; break;
        case "exec_scheduled": t.grace = { execId: ev.execId, cmd: ev.cmd, ms: ev.graceMs, start: performance.now(), intent: ev.intent ?? undefined }; break;
        case "exec_cancelled":
          t.grace = null; toast(tr("exec.cancelled", { reason: ev.reason }) + sfx, "warn"); break;
        case "agent_exec":
          t.grace = null; if (t.lastAgent) t.lastAgent.lastCmd = ev.cmd;

          if (String(ev.policy).startsWith("deny")) { t.policyBlockedUntil = performance.now() + 2400; toast(tr("policy.blocked", { cmd: ev.cmd }) + sfx, "danger"); }
          break;
        case "human_exec": t.title = ev.cmd.length > 24 ? ev.cmd.slice(0, 24) + "…" : ev.cmd; break;
        case "process_exited": t.processAlive = false; setController(t, "human"); announce(tr("shell.exited") + sfx, "var(--danger)", "warn"); break;
        case "pacing_changed": t.pacing = ev.pacing as Pacing; break;
        case "mode_changed": t.mode = ev.mode; t.effectiveMode = ev.effectiveMode ?? ev.mode; break;
        case "control_gate_changed": t.gate = !!ev.ask; break;
        case "affordance_mask_changed": t.mask = ev.allow; break;
      }
    });
    try {
      await Promise.all([connectionListener, tabListener, abortListener, eventListener]);
      await refreshProfiles();
      const info = await invoke<{ socket: string; shell: string; session: string | null; sessions: string[]; externalPending?: boolean }>("start", { rows: 24, cols: 80 });
      st.socket = info.socket ?? ""; st.shell = info.shell ?? "";
      for (const id of info.sessions) if (!tab(id)) addTab(id);
      st.active = info.session ?? "";
      st.externalPending = !!info.externalPending;
      for (const id of info.sessions) await syncStatus(id);
      if (st.active && !cur().externalPrivate) {
        const tail = await invoke<any[]>("audit_tail", { n: 60 });
        importSavedActivity(cur().timeline, tail);
      }
      st.booted = true; st.backendOnline = true;
      await tick();
      await invoke("ui_ready");
      setTimeout(() => { terms[st.active]?.refit(); focusTerm(); }, 50);
      setInterval(async () => { for (const id of st.order) { try { const s = await invoke<any>("status", { session: id }); const t = tab(id); if (t) { t.processAlive = !!s.processAlive; t.externalStarting = s.externalStarting === true; t.externalInputAvailable = s.externalInputAvailable === true; if (!t.externalPrivate) t.agents = s.connectedAgents ?? []; } } catch {} } }, 5000);
    } catch (e) {
      st.settingsTab = "profiles"; st.settingsOpen = true;
      log(`start failed: ${e}`); toast(tr("engine.failed", { err: String(e) }), "danger");
    }
  });
</script>

<svelte:window onkeydown={onKey} />

<main class="app" class:agent={cur().controller.type === "agent"} class:asking={!!cur().ctlReq || !!cur().attention} style:--term-right={st.settingsOpen ? "616px" : "16px"} style:--agent={cur().controller.type === "agent" ? agentColor(cur().controller.agentId) : cur().ctlReq ? agentColor(cur().ctlReq?.agentId) : cur().attention ? agentColor(cur().attention?.agentId) : "#8b7cff"}>
  {#each st.order as id (id)}
    {#if tab(id)?.statusReady}<Term bind:this={terms[id]} session={id} />{/if}
  {/each}
  <div class="frame" aria-hidden="true"></div>
  {#if !cur().externalPrivate}
    <HandoffWash />
    <Ghost />
    <Hold />
  {/if}
  <TabStrip onselect={select} onclose={closeTab} onnew={openTab} onsettings={() => openSettings(st.settingsTab)} onpalette={() => { st.settingsOpen = false; st.centerOpen = false; st.paletteOpen = true; }} ontimeline={() => { st.timelineOpen = !st.timelineOpen; }} />
  {#if st.active}<Island onopen={() => { st.paletteOpen = false; st.centerOpen = !st.centerOpen; }} />{/if}
  {#if !cur().externalPrivate}
  <ControlRequest />
  <GraceBar />
  <HandbackChip bind:this={chip} />
  <LeaveChip bind:this={leave} />
  <Timeline />
  {/if}
  <Toasts />
  {#if st.centerOpen && !cur().externalPrivate}<Center onclose={() => { st.centerOpen = false; focusTerm(); }} onpalette={() => { st.centerOpen = false; st.paletteOpen = true; }} onsettings={() => { st.centerOpen = false; st.settingsOpen = true; }} />{/if}
  {#if st.paletteOpen}<Palette {actions} {parsers} onclose={() => { st.paletteOpen = false; focusTerm(); }} />{/if}
  {#if st.settingsOpen}<SettingsSheet onclose={() => { st.settingsOpen = false; focusTerm(); }} />{/if}
</main>

<style>
  .app { position: relative; width: 100vw; height: 100vh; overflow: hidden; background: var(--bg); }
  .frame { position: absolute; inset: 6px; border-radius: 12px; pointer-events: none; z-index: 5;
    border: 1.5px solid var(--agent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--agent) 35%, transparent), inset 0 0 70px color-mix(in srgb, var(--agent) 16%, transparent), 0 0 0 1px color-mix(in srgb, var(--agent) 20%, transparent);
    opacity: 0; transform: scale(1.012); will-change: opacity, transform;
    transition: opacity .26s var(--ease), transform .26s var(--ease), border-color .4s var(--ease); }
  .app.agent .frame { opacity: 1; transform: scale(1); transition: opacity .42s cubic-bezier(.16, 1, .3, 1), transform .6s cubic-bezier(.16, 1, .3, 1), border-color .4s var(--ease); }
  .app.asking .frame { opacity: .55; transform: scale(1); animation: knock 1.6s ease-in-out infinite; }
  @keyframes knock { 0%, 100% { opacity: .35; } 50% { opacity: .7; } }
  @media (prefers-reduced-motion: reduce) { .frame, .app.agent .frame { transition: opacity .2s; transform: none; animation: none; } }
</style>
