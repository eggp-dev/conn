<script lang="ts">
  import CollaborationActivity from './components/CollaborationActivity.svelte';
  import TerminalPreparation from './components/TerminalPreparation.svelte';
  import {signalPending} from './lib/collaboration/attention';
  import {requestStillPending, sessionLabel, type ActivitySnapshot, type NoticeTarget} from './lib/collaboration/activity';
  import { Reconciler } from "./lib/collaboration/reconcile";
  import { applySessionSnapshot, applySessionEvent, type SessionSnapshot } from "./lib/collaboration/session";
  import { Admissions } from "./lib/collaboration/admissions";
  import { admissionSnapshot } from "./lib/collaboration/api";
  import { resolveReview } from "./lib/collaboration/review.svelte";
  import { resolveControl } from "./lib/collaboration/control.svelte";
  import { appShortcut, shortcutKey, shortcutLabel } from "./lib/shortcuts";
  import { refreshExtensions, selectTheme } from "./lib/extensions.svelte";
  import { refreshProfiles } from "./lib/profiles.svelte";
  import { onMount, tick } from "svelte";
  import Term from "./components/Term.svelte";
  import TabStrip from "./components/TabStrip.svelte";
  import Island from "./components/Island.svelte";
  import Palette from "./components/Palette.svelte";
  import type { Action, Parser } from "./components/Palette.svelte";
  import SharingDialog from "./components/SharingDialog.svelte";
  import Center from "./components/Center.svelte";
  import SettingsSheet from "./components/SettingsSheet.svelte";
  import Ghost from "./components/Ghost.svelte";
  import Hold from "./components/Hold.svelte";
  import GraceBar from "./components/GraceBar.svelte";
  import ControlRequest from "./components/ControlRequest.svelte";
  import AdmissionRequest from "./components/AdmissionRequest.svelte";
  import HandbackChip from "./components/HandbackChip.svelte";
  import Timeline from "./components/Timeline.svelte";
  import Toasts from "./components/Toasts.svelte";
  import HandoffWash from "./components/HandoffWash.svelte";
  import { recordTimeline, savedTimelines } from "./lib/timeline";
  import { invoke, listen } from "./lib/transport";
  import { cmd, changeMode, onEvent, onTabOpened, onAdmission, log, TOOL_PRESETS } from "./lib/bridge";
  import { st, cur, tab, tabIndex, newTab, toast, announce, type TabState } from "./lib/store.svelte";
  import { THEMES, agentColor } from "./lib/themes";
  import { t as tr, tabName, fmtMs, setLang, LANGS, i18n } from "./lib/i18n.svelte";

  let terms = $state<Record<string, Term>>({});
  let chip = $state<HandbackChip>();
  let washSeq = 0;
  const handbackTimers: Record<string, number> = {};

  const focusTerm = () => setTimeout(() => terms[st.active]?.focus(), 0);

  function wash(t: TabState, color: string, out: boolean) {
    t.wash = { ...(terms[t.id]?.cursorCenter() ?? { x: 4, y: 8 }), color, out, key: ++washSeq };
    setTimeout(() => { if (t.wash?.key === washSeq) t.wash = null; }, 1300);
  }
  function showHandback(t: TabState) {
    if (!t.lastAgent) return;
    t.handback = true;
    if (handbackTimers[t.id]) clearTimeout(handbackTimers[t.id]);
    handbackTimers[t.id] = window.setTimeout(() => (t.handback = false), 60_000);
  }
  const typingTimers: Record<string, number> = {};
  $effect(()=>{
    const keys=[...st.admissions.map(a=>`admission:${a.connId}`),...st.activity.connections.filter(a=>a.preparation).map(a=>`prepare:${a.connId}:${a.preparation!.id}`),
      ...st.order.flatMap(id=>{const t=tab(id)!;return t.shared ? [t.ctlReq?.id,t.approval?.id,t.proposal?.ready?t.proposal.id:null,t.attention?`attention:${id}`:null].filter(Boolean).map(key=>`${id}:${key}`):[]})];
    void signalPending(keys).catch(()=>{});
  });

  // ---- tabs ----
  const sessions = new Reconciler(
    (id: string) => invoke<SessionSnapshot>("status", { session: id }),
    (id, snapshot) => { const current = tab(id); if (current) applySessionSnapshot(current, snapshot, tr("private.title")); },
  );
  const syncStatus = (id: string) => sessions.sync(id);
  function addTab(id: string) {
    st.tabSeq += 1;
    st.tabs[id] = newTab(id, st.tabSeq);
    st.order.push(id);
  }
  async function select(id: string, focusInput = true) {
    if (id === st.active || !tab(id)) return;
    st.active = id;
    st.centerOpen = false; st.timelineOpen = false;
    await invoke("attend", { session: id });
    tab(id)!.attention = null;
    terms[id]?.refit(); if(focusInput) focusTerm();
  }
  async function openTab(profileId?: string) {
    if (st.externalPending) return;
    try {
    const sz = terms[st.active]?.size() ?? { rows: 24, cols: 80 };
    const id = await invoke<string>("open_tab", { rows: sz.rows, cols: sz.cols, profileId: profileId ?? null });
    addTab(id);
    await syncStatus(id);
    await select(id);
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
  let focusedConnection=$state<number>();
  async function focusRequest(key:string) {
    await tick();
    [...document.querySelectorAll<HTMLElement>('[data-request-key]')].find(el=>el.dataset.requestKey===key)?.focus();
  }
  async function navigateNotice(target?: NoticeTarget) {
    if (!target) { st.centerOpen=!st.centerOpen; return; }
    st.settingsOpen=false; st.paletteOpen=false; st.centerOpen=false; st.sharingOpen=false;
    if ('session' in target) {
      if (!tab(target.session)) { toast(tr('activity.gone')); return; }
      try { await syncStatus(target.session); } catch { toast(tr('decision.failed'),'danger'); return; }
      const current=tab(target.session);
      if (!current) { toast(tr('activity.gone')); return; }
      await select(target.session,!target.requestId);
      if (!requestStillPending(current,target)) { toast(tr('activity.resolved')); return; }
      if(target.requestId && target.kind) await focusRequest(`${target.session}:${target.kind}:${target.requestId}`);
    } else {
      const live=st.activity.connections.find(a=>a.connId===target.connId);
      if (target.preparationId !== undefined ? live?.preparation?.id !== target.preparationId : !live?.preparation && !st.admissions.some(a=>a.connId===target.connId)) { toast(tr('activity.resolved')); return; }
      focusedConnection=target.connId;
      await focusRequest(live?.preparation ? `preparation:${target.connId}:${live.preparation.id}` : `connection:${target.connId}`);
    }
  }
  async function choosePreparation(id:string) { await navigateNotice({session:id}); if(st.active===id && tab(id)?.processAlive) st.sharingOpen=true; }
  async function newPreparation() {
    const sz=terms[st.active]?.size()??{rows:24,cols:80};
    const id=await invoke<string>('prepare_terminal',sz);addTab(id);await syncStatus(id);await select(id);st.sharingOpen=true;
  }
  const nextNeedingAttention = () => st.order.find((id) => id !== st.active && needsYou(tab(id)));

  const maskIs = (p: string[] | null) => p === null ? cur().mask === null : !!cur().mask && p.length === cur().mask!.length && p.every((a) => cur().mask!.includes(a));
  const openSettings = (tab: typeof st.settingsTab) => { st.settingsTab = tab; st.settingsOpen = true; st.centerOpen = false; };
  const setTheme = (id: string) => { void selectTheme(id).catch(() => toast(tr("ext.saveFailed"), "warn")); };

  // State-aware: labels carry the agent's name and current values; `now` items are
  // the things to answer right now and sort first.
  const actions = $derived.by((): Action[] => {
    const c = cur();
    const approval = c.approval;
    const control = c.ctlReq;
    const holder = c.controller.type === "agent" ? c.controller.agentId ?? "agent" : c.lastAgent?.agentId ?? "agent";
    const lang = i18n.lang; void lang;
    const all: Action[] = [
      { id: "sharing", group: "view", label: tr(c.shared ? "sharing.manage" : "sharing.start"), aliases: ["share", "private", "공유", "비공유"], run: () => { st.sharingOpen = true; } },
      { id: "take", group: "conn", now: c.controller.type === "agent", label: tr("a.take"), hint: tr("a.take.hint"), aliases: ["take", "revoke", "회수", "뺏기", "제어권"], run: () => cmd("take"), when: () => cur().controller.type === "agent" },
      { id: "handback", group: "conn", now: c.handback, label: tr("a.handback", { agent: holder }), keys: shortcutLabel("⌘⏎"), aliases: ["hand back", "give back", "되돌려주기", "다시"], run: () => chip?.handBack(), when: () => !!cur().lastAgent && cur().controller.type === "human" },
      { id: "approve", group: "approval", now: true, label: tr("a.approve", { label: c.approval?.label ?? "" }), hint: c.approval?.cmd, keys: "a", aliases: ["approve", "grant", "승인"], run: () => approval && resolveReview(c.id, approval.id, "grant"), when: () => !!cur().approval },
      { id: "deny", group: "approval", now: true, danger: true, label: tr("a.deny", { label: c.approval?.label ?? "" }), keys: "d", aliases: ["deny", "거부"], run: () => approval && resolveReview(c.id, approval.id, "deny"), when: () => !!cur().approval },
      { id: "allow-session", group: "approval", now: true, label: tr("a.allow_session", { label: c.approval?.label ?? "" }), keys: "A", aliases: ["allow", "session", "세션 허용"], run: () => approval && resolveReview(c.id, approval.id, "allow_session"), when: () => !!cur().approval && !cur().reviewRequired },
      { id: "ctl-grant", group: "conn", now: true, label: tr("a.ctl.grant", { agent: c.ctlReq?.agentId ?? "" }), hint: c.ctlReq?.reason, aliases: ["allow", "grant", "허용"], run: () => control && resolveControl({ session: c.id, requestId: control.id }, "grant"), when: () => !!cur().ctlReq },
      { id: "ctl-deny", group: "conn", now: true, danger: true, label: tr("a.ctl.deny", { agent: c.ctlReq?.agentId ?? "" }), aliases: ["deny", "거부"], run: () => control && resolveControl({ session: c.id, requestId: control.id }, "deny"), when: () => !!cur().ctlReq },
      ...st.order.map((id, i) => {
        const tb = tab(id)!;
        const flags = [tb.approval ? tr("badge.approval") : "", tb.proposal?.ready ? tr("badge.proposal", { agent: tb.proposal.agentId }) : "", tb.attention ? tr("badge.knock", { agent: tb.attention.agentId }) : "", tb.controller.type === "agent" ? tr("conn.agent", { agent: tb.controller.agentId ?? "" }) : ""].filter(Boolean).join(" · ");
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
      ...(["profiles", "agents", "automation", "policy", "pacing", "appearance", "extensions", "diagnostics"] as const).map((tb) => ({ id: `settings-${tb}`, group: "settings", label: tr("a.settings", { tab: tr(`s.nav.${tb}`) }), keys: shortcutLabel("⌘,"), aliases: ["settings", "설정", tb], run: () => openSettings(tb) })),
      { id: "connect-agent", group: "setup", label: tr("s.connect.copy"), aliases: ["connect", "mcp", "codex", "cli", "agent", "에이전트", "연결", "설정 복사"], run: () => openSettings("agents") },
    ];
    return !c.shared ? all.filter(action => !["conn", "approval", "mode", "agents", "pacing"].includes(action.group)
      && !["center", "timeline", "settings-policy", "settings-pacing"].includes(action.id)) : all;
  });

  // Typed arguments: "grace 3s", "유예 3초", "tab 2", "font 14", "theme paper".
  const num = (q: string, re: RegExp) => { const m = q.match(re); return m ? { n: parseFloat(m[1]), unit: (m[2] ?? "").toLowerCase() } : null; };
  const parsers: Parser[] = [
    (q) => { if (!cur().shared) return []; const m = num(q, /^(?:grace|유예|delay)\s*(\d+(?:\.\d+)?)\s*(ms|s|초)?$/i); if (!m) return []; const ms = m.unit === "ms" ? m.n : m.n * 1000; return [{ id: "set-grace", group: "pacing", label: tr("a.grace.set", { v: fmtMs(ms) }), run: () => cmd("set_pacing", { patch: { enterGraceMs: Math.round(ms) } }) }]; },
    (q) => { if (!cur().shared) return []; const m = num(q, /^(?:interval|typing|간격)\s*(\d+)\s*(ms)?$/i); if (!m) return []; return [{ id: "set-interval", group: "pacing", label: tr("a.interval.set", { v: `${m.n}ms` }), run: () => cmd("set_pacing", { patch: { minWriteIntervalMs: Math.round(m.n) } }) }]; },
    (q) => { if (!cur().shared) return []; const m = num(q, /^(?:lease|ttl)\s*(\d+)\s*(s|초)?$/i); if (!m) return []; return [{ id: "set-lease", group: "pacing", label: tr("a.lease.set", { v: `${m.n}s` }), run: () => cmd("set_pacing", { patch: { leaseTtlSecs: Math.round(m.n) } }) }]; },
    (q) => { const m = num(q, /^(?:font|글꼴|size)\s*(\d+)\s*(px)?$/i); if (!m) return []; return [{ id: "set-font", group: "theme", label: tr("a.font.set", { v: m.n }), run: () => { st.fontSize = m.n; localStorage.setItem("ss:fontSize", String(m.n)); } }]; },
    (q) => { const m = num(q, /^(?:tab|탭)\s*(\d+)$/i); if (!m) return []; const id = st.order[m.n - 1]; if (!id) return []; return [{ id: "go-tab", group: "tabs", label: tr("a.tab.go", { name: `${m.n} · ${tab(id)!.title}` }), run: () => select(id) }]; },
    (q) => { const m = q.match(/^(?:theme|테마)\s+(\w+)$/i); if (!m) return []; const th = Object.values(THEMES).find((x) => x.id.startsWith(m[1].toLowerCase()) || x.name.toLowerCase().startsWith(m[1].toLowerCase())); if (!th) return []; return [{ id: "set-theme", group: "theme", label: tr("a.theme", { name: th.name }), run: () => setTheme(th.id) }]; },
  ];

  let dockHeight = $state(0);
  let timelineHeight = $state(0);
  const dockSpace = $derived(dockHeight);
  const timelineSpace = $derived(!cur().shared || !st.timelineOpen || !timelineHeight ? 0 : timelineHeight + 10);
  function onKey(e: KeyboardEvent) {
    if (!appShortcut(e)) {
      if (e.key === "Escape" && (st.paletteOpen || st.settingsOpen || st.timelineOpen || st.centerOpen || st.sharingOpen || cur().handback)) {
        e.preventDefault();
        st.paletteOpen = false; st.settingsOpen = false; st.timelineOpen = false; st.centerOpen = false; st.sharingOpen = false;
        cur().handback = false;
        focusTerm();
      }
      return;
    }
    const key = shortcutKey(e);
    if (key === "k") { e.preventDefault(); st.centerOpen = false; st.paletteOpen = !st.paletteOpen; if (!st.paletteOpen) focusTerm(); }
    else if (key === ",") { e.preventDefault(); st.settingsOpen = !st.settingsOpen; if (!st.settingsOpen) focusTerm(); }
    else if (key === "j") { e.preventDefault(); if (!cur().shared) return; st.timelineOpen = !st.timelineOpen; if (!st.timelineOpen) focusTerm(); }
    else if (key === "t") { e.preventDefault(); openTab(); }
    else if (key === "w") { e.preventDefault(); closeTab(st.active); }
    else if (key === "Enter") { e.preventDefault(); if (!cur().shared) return; if (cur().handback) chip?.handBack(); }
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

  onMount(() => {
    let disposed = false;
    let statusTimer: ReturnType<typeof setInterval> | undefined;
    let admissions = new Admissions();
    let reconciling = false;
    async function reconcileAdmissions() {
      if (reconciling) return;
      reconciling = true;
      const current = admissions;
      try { const snapshot = await admissionSnapshot(); if (!disposed && current === admissions) { current.snapshot(snapshot); st.admissions = current.requests; } }
      catch { /* Keep actionable requests during a temporary transport failure. */ }
      finally { reconciling = false; }
    }
    const activity = new Reconciler<ActivitySnapshot>(()=>invoke('activity_snapshot'),(_id,snapshot)=>{
      if(snapshot.revision>=st.activity.revision) st.activity=snapshot;
    });
    const refreshActivity=()=>activity.sync('app').catch(()=>{});
    const activityListener=listen('ss:activity_changed',()=>{activity.changed('app');void refreshActivity();});
    const activityTimer=setInterval(refreshActivity,1500);
    const admissionTimer = setInterval(reconcileAdmissions, 2500);
    const connectionListener = listen<{ connected: boolean }>("ss:connection", ({payload}) => {
      st.backendOnline = payload.connected;
      if (payload.connected) { admissions = new Admissions(); void reconcileAdmissions(); }
    });
    const admissionListener = onAdmission((p) => {
      admissions.event(p); st.admissions = admissions.requests;
      if (p.state === "pending") { announce(tr("admission.announce", { agent: p.agentId }), agentColor(p.agentId), "attention", 3200, undefined, {connId:p.connId}); }
    });
    const tabListener = onTabOpened(async (p) => {
      // Agent tabs stay in the background; an explicit external-launch request may select its new tab.
      if (!tab(p.session)) addTab(p.session);
      try { await syncStatus(p.session); } catch { return; }
      // A prepared tab can be aborted while its initial status is in flight.
      if (!tab(p.session)) return;
      st.externalPending = st.order.some(id => tab(id)?.externalStarting);
      if (p.agentId) announce(tr('tab.opened.by',{agent:p.agentId,where:sessionLabel(st.order,st.tabs,p.session)}),agentColor(p.agentId),'attention',3200,p.reason??undefined,{session:p.session});
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
      sessions.changed(ev.session);
      const t = tab(ev.session);
      if (!t || !t.statusReady) return;
      const previousController = t.controller.type;
      const proposalWasReady = t.proposal?.ready;
      applySessionEvent(t, ev);
      if (previousController !== t.controller.type && t.id === st.active) {
        wash(t, agentColor(t.controller.agentId ?? t.lastAgent?.agentId), t.controller.type === "human");
      }
      if (ev.event === "sharing_changed") {
        recordTimeline(t.timeline, ev);
        if (!t.shared) st.timelineOpen = false;
        void syncStatus(t.id).catch(() => {});
        return;
      }
      if (!t.shared) return;
      recordTimeline(t.timeline, ev);
      const here = t.id === st.active;
      const where = here ? "" : tabName(tabIndex(t.id));
      const W = (k: string, p: Record<string, string | number> = {}) => (where ? tr(`${k}.where`, { ...p, where }) : tr(k, p));
      const sfx = where ? ` · ${where}` : "";
      switch (ev.event) {
        case "control_granted": {
          announce(W("conn.agent", { agent: ev.agentId }), agentColor(ev.agentId), "conn", 2600, ev.reason ?? undefined, {session:t.id});
          break;
        }
        case "control_revoked":
          if (ev.reason === "human_input" || ev.reason === "taken") showHandback(t);

          {
            const a = { agent: ev.agentId };
            const why: Record<string, string> = { human_input: "", taken: "", released: tr("conn.released", a), expired: tr("conn.expired", a), disconnected: tr("conn.disconnected", a), process_exited: tr("conn.shell_exited") };
            announce(W("conn.yours"), agentColor(ev.agentId), "conn", 2600, why[ev.reason] ?? ev.reason, {session:t.id});
          }
          break;
        case "control_handed_back": announce(tr("conn.again", { agent: ev.agentId }), agentColor(ev.agentId), "conn", 1800); break;
        case "control_requested": announce(W("conn.asks", { agent: ev.request.agentId }), agentColor(ev.request.agentId), "conn", 3000, ev.request.reason ?? undefined, {session:t.id,kind:"control",requestId:ev.request.requestId}); break;
        case "attention_requested": announce(tr("attention.asks", { agent: ev.agentId, where: tabName(tabIndex(t.id)) }), agentColor(ev.agentId), "attention", 3200, ev.reason ?? undefined, {session:t.id}); break;
        case "tab_opened": break; // announced after tab registration, even if this event arrived early
        case "agent_switched_tab":
          if (t.id === ev.to) announce(tr('agent.moved.to',{agent:ev.agentId,where:sessionLabel(st.order,st.tabs,ev.to)}),agentColor(ev.agentId),'info',2200,undefined,{session:ev.to});
          break;
        case "agent_input":
          t.typing = true; if (typingTimers[t.id]) clearTimeout(typingTimers[t.id]); typingTimers[t.id] = window.setTimeout(() => (t.typing = false), 500);
          break;
        case "proposal_changed": {
          const becameReady = ev.proposal.state === "ready" && !proposalWasReady;
          // A proposal waiting on another tab is a knock: you decide there, it cannot run itself.
          if (becameReady && !here) announce(tr("proposal.waiting", { agent: ev.proposal.agentId, where }), agentColor(ev.proposal.agentId), "attention", 3200, ev.proposal.intent ?? ev.proposal.text, {session:t.id,kind:"proposal",requestId:ev.proposal.proposalId});
          break;
        }
        case "proposal_resolved": {
          if (ev.state === "denied") { toast(tr("policy.blocked", { cmd: ev.cmd }), "danger"); }
          if (ev.state === "rejected") { toast(tr("proposal.rejected"), "warn"); }
          break;
        }
        case "approval_requested":
          announce(W("approval.needed", { agent: ev.request.agentId }), "var(--warn)", "approval", 3000, `${ev.request.label}${ev.request.intent ? " · " + ev.request.intent : " · " + ev.request.cmd}`, {session:t.id,kind:"review",requestId:ev.request.id});
          break;
        case "approval_resolved":

          toast(`${ev.state} (${ev.by})${sfx}`, ev.state === "granted" ? "ok" : "danger");
          void syncStatus(t.id).catch(() => {});
          break;
        case "exec_cancelled":
          toast(tr("exec.cancelled", { reason: ev.reason }) + sfx, "warn"); break;
        case "agent_exec":

          if (String(ev.policy).startsWith("deny")) { t.policyBlockedUntil = performance.now() + 2400; toast(tr("policy.blocked", { cmd: ev.cmd }) + sfx, "danger"); }
          break;
        case "process_exited": announce(tr("shell.exited") + sfx, "var(--danger)", "warn"); break;
      }
    });
    void (async () => { try {
      await Promise.all([connectionListener, admissionListener, tabListener, abortListener, eventListener, activityListener]);
      if (disposed) return;
      await reconcileAdmissions();
      await refreshProfiles();
      await refreshExtensions().catch(() => {});
      const info = await invoke<{ socket: string; session: string | null; sessions: string[]; externalPending?: boolean }>("start", { rows: 24, cols: 80 });
      if (disposed) return;
      st.socket = info.socket ?? "";
      for (const id of info.sessions) if (!tab(id)) addTab(id);
      st.active = info.session ?? "";
      st.externalPending = !!info.externalPending;
      for (const id of info.sessions) await syncStatus(id);
      if (st.active && !!cur().shared) {
        const tail = await invoke<any[]>("audit_tail", { n: 60 });
        const saved = savedTimelines(tail);
        for (const [id, history] of Object.entries(saved)) {
          const live = tab(id);
          // Live events win if they arrived while startup was reading the audit.
          if (live?.shared && !live.timeline.items.length) live.timeline = history;
        }
      }
      // A window opened later, or a reload, must still see who is waiting.
      await reconcileAdmissions();
      await refreshActivity();
      st.backendOnline = true;
      await tick();
      await invoke("ui_ready");
      setTimeout(() => { terms[st.active]?.refit(); focusTerm(); }, 50);
      statusTimer = setInterval(() => { for (const id of st.order) void syncStatus(id).catch(() => {}); }, 5000);
    } catch (e) {
      st.settingsTab = "profiles"; st.settingsOpen = true;
      log(`start failed: ${e}`); toast(tr("engine.failed", { err: String(e) }), "danger");
    } })();
    return () => {
      disposed = true; sessions.dispose(); activity.dispose(); clearInterval(activityTimer); clearInterval(admissionTimer); clearInterval(statusTimer);
      for (const listener of [connectionListener, admissionListener, tabListener, abortListener, eventListener, activityListener]) void listener.then(unlisten => unlisten());
    };
  });
</script>

<svelte:window onkeydown={onKey} />

<main style:--handback-space={`${dockSpace}px`} style:--term-bottom={`${26 + dockSpace + timelineSpace}px`} class="app" class:agent={cur().controller.type === "agent"} class:asking={!!cur().ctlReq || !!cur().attention} style:--term-right={st.settingsOpen ? "616px" : "16px"} style:--agent={cur().controller.type === "agent" ? agentColor(cur().controller.agentId) : cur().ctlReq ? agentColor(cur().ctlReq?.agentId) : cur().attention ? agentColor(cur().attention?.agentId) : "#8b7cff"}>
  {#each st.order as id (id)}
    {#if tab(id)?.statusReady}<Term bind:this={terms[id]} session={id} />{/if}
  {/each}
  <div class="frame" aria-hidden="true"></div>
  {#if !!cur().shared}
    <HandoffWash />
  {/if}
  <TabStrip onselect={select} onclose={closeTab} onnew={openTab} onsettings={() => openSettings(st.settingsTab)} onpalette={() => { st.settingsOpen = false; st.centerOpen = false; st.paletteOpen = true; }} ontimeline={() => { st.timelineOpen = !st.timelineOpen; }} />
  {#if st.active}<Island onnotice={navigateNotice} onopen={() => { st.paletteOpen = false; st.centerOpen = !st.centerOpen; }} />{/if}
  <div class="interaction-dock" bind:clientHeight={dockHeight}><CollaborationActivity onselect={(session)=>void navigateNotice({session})} /><TerminalPreparation focused={focusedConnection} onchoose={choosePreparation} onnew={newPreparation} />{#if !st.settingsOpen}<AdmissionRequest focused={focusedConnection} />{/if}{#if cur().shared}<ControlRequest /><Hold /><Ghost /><GraceBar /><HandbackChip bind:this={chip} />{/if}</div>
  {#if !!cur().shared}
  <Timeline bind:height={timelineHeight} />
  {/if}
  <Toasts />
  {#if st.centerOpen}<Center onclose={() => { st.centerOpen = false; focusTerm(); }} />{/if}
  {#if st.paletteOpen}<Palette {actions} {parsers} onclose={() => { st.paletteOpen = false; focusTerm(); }} />{/if}
  {#if st.sharingOpen}{#key st.active}<SharingDialog onclose={() => { st.sharingOpen = false; focusTerm(); }} />{/key}{/if}
  {#if st.settingsOpen}<SettingsSheet onclose={() => { st.settingsOpen = false; queueMicrotask(focusTerm); }} />{/if}
</main>

<style>
  .interaction-dock { position:absolute; left:0; right:0; bottom:26px; z-index:19; max-height:45vh; overflow:auto; }

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
