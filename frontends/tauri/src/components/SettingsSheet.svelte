<script lang="ts">
  import ExtensionsPane from "./ExtensionsPane.svelte";
  import { selectTheme } from "../lib/extensions.svelte";
  import AutomationPane from "./AutomationPane.svelte";
  import AgentConnections from "./AgentConnections.svelte";
  import DiagnosticConnections from "./DiagnosticConnections.svelte";
  import ProfilesPane from "./ProfilesPane.svelte";
  import { tick, onMount } from "svelte";
  import { invoke } from "../lib/transport";
  import { fade } from "svelte/transition";
  import { st, cur, tab, toast, type Analysis } from "../lib/store.svelte";
  import { cmd, changeMode, type Mode } from "../lib/bridge";
  import { THEMES, agentColor } from "../lib/themes";
  import { t, fmtMs, i18n, setLang, LANGS } from "../lib/i18n.svelte";
  let { onclose }: { onclose: () => void } = $props();
  const NAV = ["profiles", "agents", "automation", "policy", "pacing", "appearance", "extensions", "diagnostics"] as const;

  let profileDirty = $state(false);
  let pendingAction = $state<(() => void) | null>(null);
  function guard(action: () => void) { if (profileDirty || policyDirty) pendingAction = action; else action(); }
  function close() { guard(onclose); }
  function navigate(id: typeof NAV[number]) { if (id !== st.settingsTab) guard(() => { profileDirty = false; policyDirty = false; st.settingsTab = id; void tick().then(() => navigation?.querySelector<HTMLButtonElement>('[aria-current="page"]')?.focus()); }); }
  let navigation: HTMLElement;
  let dialog = $state<HTMLElement>();
  $effect(() => {
    if (!dialog?.parentElement) return;
    const siblings = Array.from(dialog.parentElement.children).filter(el => el !== dialog && !el.classList.contains("scrim"));
    const previous = siblings.map(el => el.hasAttribute("inert"));
    siblings.forEach(el => el.setAttribute("inert", ""));
    return () => siblings.forEach((el, i) => { if (!previous[i]) el.removeAttribute("inert"); });
  });
  const categories = $derived(NAV.filter(id => !!cur().shared || !["agents", "policy", "pacing"].includes(id)));
  $effect(() => { if (!categories.includes(st.settingsTab as typeof NAV[number])) st.settingsTab = categories[0]; });
  $effect(() => { queueMicrotask(() => navigation?.querySelector<HTMLButtonElement>('[aria-current="page"]')?.focus()); });
  // ---- scope: this tab or all tabs; plus stored defaults for new tabs ----
  let scope = $state<"tab" | "all">("tab");
  const sharedTabs = $derived(st.order.filter(id => tab(id)?.statusReady && !!tab(id)?.shared));
  const privateSection = $derived(!cur().shared && ["agents", "policy", "pacing"].includes(st.settingsTab));
  async function apply(name: string, args: Record<string, unknown>) {
    if (!cur().shared) return;
    if (name === "set_mode") {
      for (const id of scope === "all" ? sharedTabs : [st.active]) await changeMode(args.mode as Mode, id);
      return;
    }
    if (scope === "all") { await Promise.all(sharedTabs.map((id) => cmd(name, { ...args, session: id }))); }
    else await cmd(name, args);
  }
  // Connection-level: applies to the whole app, not to one tab.
  let admissionAsk = $state(true);
  onMount(() => { void invoke<{ ask: boolean }>("admission_policy").then(p => { admissionAsk = p.ask; }).catch(() => {}); });
  async function setAdmission(ask: boolean) {
    try { admissionAsk = (await invoke<{ ask: boolean }>("set_admission", { ask })).ask; }
    catch (e) { toast(String(e), "warn"); }
  }
  async function saveDefaults() {
    const c = cur();
    if (!c.shared) return;
    await cmd("set_defaults", { defaults: { mode: c.mode, gate: c.gate, pacing: c.pacing, mask: c.mask } });
    toast(t("s.defaults.saved"), "ok");
  }

  // ---- tools ----
  const ALL = ["snapshot", "request_control", "type", "send_key", "interrupt", "release_control", "check_approval", "request_attention", "open_tab", "switch_tab"];
  const GROUPS: [string, string[]][] = [
    ["s.tg.see", ["snapshot"]],
    ["s.tg.write", ["request_control", "type", "send_key", "interrupt", "release_control"]],
    ["s.tg.tabs", ["open_tab", "switch_tab"]],
    ["s.tg.knock", ["request_attention", "check_approval"]],
  ];
  const PRESETS: [string, string[] | null][] = [
    ["s.preset.observe", ["snapshot", "request_attention", "check_approval", "switch_tab"]],
    ["s.preset.notabs", ALL.filter((a) => a !== "open_tab")],
    ["s.preset.all", null],
  ];
  const has = (a: string) => !cur().mask || cur().mask!.includes(a);
  const presetOn = (p: string[] | null) => p === null ? cur().mask === null : !!cur().mask && p.length === cur().mask!.length && p.every((a) => cur().mask!.includes(a));
  function setMask(a: string, on: boolean) {
    const set = new Set(cur().mask ?? ALL);
    on ? set.add(a) : set.delete(a);
    const arr = ALL.filter((x) => set.has(x));
    apply("set_affordances", { allow: arr.length === ALL.length ? null : arr });
  }
  type Live = { conn: number; agentId: string; affordances: string[] };
  let live = $state<Live[]>([]);
  async function poll() { if (st.settingsTab !== "agents" || !cur().shared) { live = []; return; } try { live = await cmd<Live[]>("agents"); } catch { live = []; } }
  $effect(() => { void st.active; void st.settingsTab; poll(); const i = setInterval(poll, 2000); return () => clearInterval(i); });

  // ---- policy ----
  let policyText = $state("");
  let policyPath = $state("");
  let policyDirty = $state(false);
  let showSource = $state(false);
  let testCmd = $state("");
  let verdict = $state<Analysis | null>(null);
  type Rules = { rules: { kind: string; label: string; pattern?: string; command?: string; args?: string | null }[]; protected: string[]; opaque: string; isolateDangerous: boolean; requireIntent: boolean; default: string };
  let rules = $state<Rules | null>(null);
  async function loadPolicy() {
    const r = await cmd<{ path: string; text: string }>("policy_read");
    policyText = r.text; policyPath = r.path; policyDirty = false;
    rules = await cmd<Rules>("policy_rules");
  }
  async function savePolicy() {
    try { await cmd("policy_write", { text: policyText }); policyDirty = false; toast(t("s.policy.saved"), "ok"); setTimeout(loadPolicy, 300); }
    catch (e) { toast(t("s.policy.save_failed", { err: String(e) }), "danger"); }
  }
  $effect(() => { if (!!cur().shared && st.settingsTab === "policy" && !rules) loadPolicy(); });
  $effect(() => {
    const c = testCmd;
    if (!cur().shared || !c.trim()) { verdict = null; return; }
    const h = setTimeout(async () => { verdict = await cmd("policy_test", { cmd: c }); }, 120);
    return () => clearTimeout(h);
  });
  const matchedLabels = $derived(new Set((verdict?.segments ?? []).map((s) => s.label).filter(Boolean)));
  const recentCmds = $derived.by(() => {
    const seen = new Set<string>(); const out: string[] = [];
    for (const it of [...cur().timeline.items].reverse()) { if (it.kind === "exec" && it.text && !seen.has(it.text)) { seen.add(it.text); out.push(it.text); if (out.length >= 6) break; } }
    return out;
  });

  // ---- pacing ----
  const pace = (patch: Record<string, number>) => apply("set_pacing", { patch });
  const PACE_PRESETS: [string, Record<string, number>, boolean | null][] = [
    ["careful", { enterGraceMs: 3000, minWriteIntervalMs: 80 }, true],
    ["balanced", { enterGraceMs: 1000, minWriteIntervalMs: 40 }, null],
    ["fast", { enterGraceMs: 0, minWriteIntervalMs: 0 }, null],
  ];
  async function pacePreset(patch: Record<string, number>, gate: boolean | null) {
    await pace(patch);
    if (gate !== null) await apply("set_control_gate", { ask: gate });
  }

  // ---- appearance ----
  function setTheme(id: string) { void selectTheme(id).catch(() => toast(t("ext.saveFailed"), "warn")); }

  // ---- connect an external agent ----
  type CliStatus = { canConfigure: boolean; canInstall: boolean; bundled: string | null; onPath: string | null };
  type AgentConfiguration = { command: string; endpoint: string; mcpJson: string; codexToml: string };
  let setupStatus = $state<CliStatus | null>(null);
  let setupLoading = $state(false);
  let setupBusy = $state(false);
  let setupError = $state("");
  let config = $state<AgentConfiguration | null>(null);
  let configFormat = $state<"mcpJson" | "codexToml">("mcpJson");
  let showConfiguration = $state(false);
  const configText = $derived(config?.[configFormat] ?? "");
  async function loadSetup() {
    setupLoading = true;
    setupError = "";
    try { setupStatus = await cmd<CliStatus>("cli_status"); }
    catch (e) { setupError = t("s.connect.load_failed", { error: String(e) }); }
    finally { setupLoading = false; }
  }
  async function copyAgentConfiguration() {
    setupBusy = true;
    setupError = "";
    try {
      // Creating an AppImage's stable CLI copy happens only on this explicit action.
      config = await cmd<AgentConfiguration>("agent_configuration");
      try {
        await navigator.clipboard.writeText(config[configFormat]);
        toast(t("s.connect.copied"), "ok");
      } catch {
        showConfiguration = true;
        setupError = t("s.connect.copy_failed");
        toast(t("copy.failed"), "warn");
      }
    } catch (e) { setupError = t("s.connect.failed", { error: String(e) }); }
    finally { setupBusy = false; }
  }
  $effect(() => { if (!!cur().shared && st.settingsTab === "agents") loadSetup(); });

  // ---- diagnostics ----
  let diag = $state<any>(null);
  async function loadDiag() { try { diag = await cmd("diagnostics"); } catch (e) { toast(String(e), "danger"); } }
  $effect(() => {
    void st.active;
    if (st.settingsTab !== "diagnostics") return;
    void loadDiag();
    const timer = setInterval(loadDiag, 3000);
    return () => clearInterval(timer);
  });

  function key(e: KeyboardEvent) {
    if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); if (pendingAction) pendingAction = null; else close(); }
    if (e.key === "Tab") {
      const controls = Array.from(dialog!.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), summary, [tabindex="0"]')).filter(el => { const closed = el.closest("details:not([open])"); return el.getClientRects().length > 0 && (!closed || el === closed.querySelector(":scope > summary")); });
      const first = controls[0], last = controls[controls.length - 1];
      if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last?.focus(); }
      else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first?.focus(); }
    }
  }
</script>

<div class="scrim" in:fade={{ duration: 120 }} onclick={close} role="presentation"></div>
<aside bind:this={dialog} class="sheet palette-surface" role="dialog" aria-modal="true" aria-label={t("s.title")} onkeydown={key} tabindex="-1">
  <header>
    <span class="title">{t("s.title")}</span>
    <button class="btn ghost" onclick={close}>{t("close")} <kbd>esc</kbd></button>
  </header>
  {#if pendingAction}<div class="discard" role="alert"><span>{t("s.discardQuestion")}</span><button class="btn ghost" onclick={() => pendingAction = null}>{t("cancel")}</button><button class="btn" onclick={async () => { const action = pendingAction; if (policyDirty) await loadPolicy(); pendingAction = null; profileDirty = false; policyDirty = false; action?.(); }}>{t("s.discard")}</button></div>{/if}
  <div class="body">
    <nav bind:this={navigation} class="category-list" onkeydown={(e) => { if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) return; const buttons = Array.from(e.currentTarget.querySelectorAll("button")); const index = buttons.indexOf(document.activeElement as HTMLButtonElement); e.preventDefault(); const next = e.key === "Home" ? 0 : e.key === "End" ? buttons.length - 1 : (index + (e.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length; buttons[next]?.focus(); }} aria-label={t("s.title")}>
      {#each categories as id}<button class:on={st.settingsTab === id} aria-current={st.settingsTab === id ? "page" : undefined} onclick={() => navigate(id)}>{t(`s.nav.${id}`)}</button>{/each}
    </nav>
    {#key st.settingsTab}
    <section class="settings-content" aria-label={t(`s.nav.${st.settingsTab}`)}>
      <h2>{t(`s.nav.${st.settingsTab}`)}</h2>
      {#if ["agents", "pacing", "appearance"].includes(st.settingsTab)}<p class="apply-note muted">{t("s.immediate")}</p>{/if}
      {#if st.settingsTab === "extensions"}<ExtensionsPane />
      {:else if privateSection}
        <p class="muted">{t("private.settings")}</p>
      {:else if st.settingsTab === "profiles"}
        <ProfilesPane ondirty={dirty => profileDirty = dirty} />
      {:else if st.settingsTab === "automation"}
        <AutomationPane />
      {:else if st.settingsTab === "agents"}
        <details class="advanced"><summary>{t("s.sharing")}</summary><p class="muted">{t("privacy.sharing")}</p></details>
        <AgentConnections />
        <details class="agent-setup"><summary>{t("agents.other")}</summary>
          <h3>{t("s.connect.title")}</h3>
          <p class="muted">{t("s.connect.hint")}</p>
          <label class="config-format">{t("s.connect.format")}
            <select bind:value={configFormat} disabled={setupBusy}>
              <option value="mcpJson">MCP JSON</option>
              <option value="codexToml">Codex TOML</option>
            </select>
          </label>
          <button class="btn primary" disabled={setupBusy || !setupStatus?.canConfigure} onclick={copyAgentConfiguration}>{setupBusy ? t("s.connect.preparing") : t("s.connect.copy")}</button>
          {#if !setupLoading && setupStatus && !setupStatus.canConfigure}<p class="muted">{t("s.connect.missing")}</p>{/if}
          {#if setupError}<p class="setup-error" role="alert">{setupError}</p>{/if}
          {#if !setupLoading && !setupStatus}<button class="btn" onclick={loadSetup}>{t("s.connect.retry")}</button>{/if}
          {#if config}
            <details bind:open={showConfiguration}>
              <summary>{t("s.connect.view")}</summary>
              <textarea readonly value={configText} aria-label={t("s.connect.view")} spellcheck="false" onfocus={(event) => event.currentTarget.select()}></textarea>
            </details>
          {/if}
        </details>
        <h3>{t("mode")}</h3>
        <div class="scope-row"><label>{t("s.applyTo")} <select bind:value={scope}><option value="tab">{t("s.currentTab")}</option><option value="all">{t("s.scope.all")}</option></select></label><details><summary>{t("s.defaults")}</summary><button class="btn ghost" onclick={saveDefaults}>{t("s.defaults.use")}</button></details></div>
        <div class="seg">
          {#each ["observe", "copilot", "autopilot"] as id}
            <button class:on={cur().mode === id} aria-pressed={cur().mode === id} onclick={() => apply("set_mode", { mode: id })}><b>{t(`mode.${id}`)}</b><span>{t(`mode.${id}.desc`)}</span></button>
          {/each}
        </div>
        <label class="row"><input type="checkbox" checked={cur().gate} onchange={(e) => apply("set_control_gate", { ask: (e.target as HTMLInputElement).checked })} /> <span>{t("gate")} <em class="muted">{t("gate.desc")}</em></span></label>
        <label class="row"><input type="checkbox" checked={admissionAsk} onchange={(e) => setAdmission((e.target as HTMLInputElement).checked)} /> <span>{t("admission.setting")} <em class="muted">{t("admission.setting.desc")}</em></span></label>

<details class="advanced"><summary>{t("s.advancedPermissions")}</summary>
        <div class="presets">
          {#each PRESETS as [k, p]}
            <button class="pill" class:on={presetOn(p)} aria-pressed={presetOn(p)} onclick={() => apply("set_affordances", { allow: p })}>{t(k)}</button>
          {/each}
        </div>
        <div class="tools">
          {#each GROUPS as [g, list]}
            <div class="tg">
              <b>{t(g)}</b>
              {#each list as a}
                <label class="row tool"><input type="checkbox" checked={has(a)} onchange={(e) => setMask(a, (e.target as HTMLInputElement).checked)} /><code>{a}</code><span class="muted">{t(`s.tool.${a}`)}</span></label>
              {/each}
            </div>
          {/each}
        </div>

        <h3>{t("s.live")} <em class="muted">{t("s.live.hint")}</em></h3>
        {#if live.length === 0}<p class="muted">{t("s.live.none")}</p>{/if}
        {#each live as a (a.conn)}
          <div class="agent" style:--a={agentColor(a.agentId)}>
            <span class="sw"></span><b>{a.agentId}</b>
            <span class="chips">{#each a.affordances as f}<code>{f}</code>{:else}<span class="muted">{t("center.nothing")}</span>{/each}</span>
          </div>
        {/each}
        {#if cur().allows.length}
          <h3>{t("s.allows")} <em class="muted">{t("s.allows.hint")}</em></h3>
          {#each cur().allows as l}<button class="chip" onclick={() => cmd("revoke_session_allow", { label: l })}>✓ {l} <span class="muted">×</span></button>{/each}
        {/if}

        </details>
      {:else if st.settingsTab === "policy"}
        <h3>{t("s.policy.test")} <em class="muted">{t("s.policy.test.hint")}</em></h3>
        <div class="test">
          <input aria-label={t("s.policy.test")} placeholder={t("s.commandPlaceholder")} bind:value={testCmd} spellcheck="false" />
          {#if verdict}<span class="verdict {verdict.policy}">{verdict.policy}{#if verdict.label} · {verdict.label}{/if}</span>{/if}
        </div>
        {#if recentCmds.length}
          <div class="recent"><span class="muted">{t("s.policy.recent")}</span>{#each recentCmds as c}<button class="pill mono" onclick={() => (testCmd = c)}>{c}</button>{/each}</div>
        {/if}
        {#if verdict && (verdict.segments.length > 1 || verdict.segments.some((s) => s.targets.length || s.opaque))}
          <ul class="segs">
            {#each verdict.segments as s}
              <li class={s.policy}><span class="dot"></span><code>{s.text}</code>{#if s.label}<span class="tag">{s.label}</span>{/if}{#if s.opaque}<span class="tag">{s.opaque}</span>{/if}
                {#each s.targets as tg}<span class="tag" class:danger={tg.protected}>→ {tg.path}{tg.gitRepo ? " · git" : ""}{tg.entries != null ? ` · ${tg.entries}` : ""}{tg.protected ? ` · ${t("hold.protected")}` : ""}</span>{/each}
              </li>
            {/each}
            {#if verdict.isolationViolation}<li class="deny"><span class="dot"></span><span>{t("s.policy.isolation")}</span></li>{/if}
          </ul>
        {/if}

        <h3>{t("s.policy.rules")} <em class="muted">{t("s.policy.rules.hint")}</em></h3>
        {#if rules}
          <ol class="rules">
            {#each rules.rules as r}
              <li class={r.kind} class:hit={matchedLabels.has(r.label)}>
                <span class="kind">{r.kind}</span>
                <details><summary>{r.label}</summary><code>{r.pattern ?? `${r.command}${r.args ? " " + r.args : ""}`}</code></details>
              </li>
            {/each}
          </ol>
          <div class="facts">
            <span class="tag">{t("s.policy.opaque", { v: rules.opaque })}</span>
            {#if rules.isolateDangerous}<span class="tag">{t("s.policy.isolate")}</span>{/if}
            {#if rules.requireIntent}<span class="tag">{t("s.policy.intent")}</span>{/if}
            <span class="tag">{t("s.policy.default", { v: rules.default })}</span>
          </div>
          {#if rules.protected.length}
            <h3>{t("s.policy.protected")}</h3>
            <div class="facts">{#each rules.protected as p}<code class="tag">{p}</code>{/each}</div>
          {/if}
        {/if}

        <button class="disclose" onclick={() => (showSource = !showSource)}>{showSource ? "▾" : "▸"} {t("s.policy.source")} <em class="muted">{policyPath}</em></button>
        {#if showSource}
          <textarea aria-label={t("s.policy.source")} bind:value={policyText} oninput={() => (policyDirty = true)} spellcheck="false"></textarea>
          <div class="actions"><button class="btn ghost" onclick={loadPolicy}>{t("revert")}</button><button class="btn primary" disabled={!policyDirty} onclick={savePolicy}>{t("save")}</button></div>
          <p class="muted">{t("s.policy.source.hint")}</p>
        {/if}

      {:else if st.settingsTab === "pacing"}
        <div class="scope-row"><label>{t("s.applyTo")} <select bind:value={scope}><option value="tab">{t("s.currentTab")}</option><option value="all">{t("s.scope.all")}</option></select></label><details><summary>{t("s.defaults")}</summary><button class="btn ghost" onclick={saveDefaults}>{t("s.defaults.use")}</button></details></div>
        <h3>{t("s.pacing.presets")} {#if !PACE_PRESETS.some(([, patch, gate]) => cur().pacing.enterGraceMs === patch.enterGraceMs && cur().pacing.minWriteIntervalMs === patch.minWriteIntervalMs && (gate === null || cur().gate === gate))}<span class="v">{t("s.custom")}</span>{/if}</h3>
        <div class="seg">
          {#each PACE_PRESETS as [k, patch, gate]}
            <button class:on={(cur().pacing.enterGraceMs === patch.enterGraceMs && cur().pacing.minWriteIntervalMs === patch.minWriteIntervalMs) && (gate === null || cur().gate === gate)} aria-pressed={(cur().pacing.enterGraceMs === patch.enterGraceMs && cur().pacing.minWriteIntervalMs === patch.minWriteIntervalMs) && (gate === null || cur().gate === gate)} onclick={() => pacePreset(patch, gate)}><b>{t(`s.pacing.${k}`)}</b><span>{t(`s.pacing.${k}.desc`)}</span></button>
          {/each}
        </div>
        <h3>{t("s.grace")} <label class="v numeric"><input aria-label={t("s.grace")} type="number" min="0" max="5000" step="1" value={cur().pacing.enterGraceMs} onchange={e => { const n = e.currentTarget.valueAsNumber; if (Number.isFinite(n)) pace({ enterGraceMs: Math.max(0, Math.min(5000, Math.round(n))) }); }} /> ms</label></h3>
        <p class="muted">{t("s.grace.hint")}</p>
        <input aria-label={t("s.grace")} type="range" min="0" max="5000" step="100" value={cur().pacing.enterGraceMs} onchange={(e) => pace({ enterGraceMs: +(e.target as HTMLInputElement).value })} />
        <details class="advanced"><summary>{t("s.advanced")}</summary>
        <h3>{t("s.interval")} <label class="v numeric"><input aria-label={t("s.interval")} type="number" min="0" max="500" step="1" value={cur().pacing.minWriteIntervalMs} onchange={e => { const n = e.currentTarget.valueAsNumber; if (Number.isFinite(n)) pace({ minWriteIntervalMs: Math.max(0, Math.min(500, Math.round(n))) }); }} /> ms</label></h3>
        <p class="muted">{t("s.interval.hint")}</p>
        <input aria-label={t("s.interval")} type="range" min="0" max="500" step="10" value={cur().pacing.minWriteIntervalMs} onchange={(e) => pace({ minWriteIntervalMs: +(e.target as HTMLInputElement).value })} />
        <h3>{t("s.lease")} <label class="v numeric"><input aria-label={t("s.lease")} type="number" min="10" max="600" step="1" value={cur().pacing.leaseTtlSecs} onchange={e => { const n = e.currentTarget.valueAsNumber; if (Number.isFinite(n)) pace({ leaseTtlSecs: Math.max(10, Math.min(600, Math.round(n))) }); }} /> s</label></h3>
        <p class="muted">{t("s.lease.hint")}</p>
        <input aria-label={t("s.lease")} type="range" min="10" max="600" step="10" value={cur().pacing.leaseTtlSecs} onchange={(e) => pace({ leaseTtlSecs: +(e.target as HTMLInputElement).value })} />
        <h3>{t("s.approval")} <label class="v numeric"><input aria-label={t("s.approval")} type="number" min="30" max="900" step="1" value={cur().pacing.approvalTtlSecs} onchange={e => { const n = e.currentTarget.valueAsNumber; if (Number.isFinite(n)) pace({ approvalTtlSecs: Math.max(30, Math.min(900, Math.round(n))) }); }} /> s</label></h3>
        <p class="muted">{t("s.approval.hint")}</p>
        <input aria-label={t("s.approval")} type="range" min="30" max="900" step="30" value={cur().pacing.approvalTtlSecs} onchange={(e) => pace({ approvalTtlSecs: +(e.target as HTMLInputElement).value })} />

        </details>
      {:else if st.settingsTab === "appearance"}
        <h3>{t("s.language")}</h3>
        <div class="presets">
          {#each LANGS as l}<button class="pill" class:on={i18n.lang === l.id} aria-pressed={i18n.lang === l.id} onclick={() => setLang(l.id)}>{l.name}</button>{/each}
        </div>
        <h3>{t("s.theme")}</h3>
        <div class="themes">
          {#each Object.values(THEMES) as th}
            <button class="tcard" class:on={st.theme === th.id} aria-pressed={st.theme === th.id} onclick={() => setTheme(th.id)} style:--tbg={th.tokens.bg} style:--tfg={th.tokens.fg} style:--ts={th.tokens.surface2}>
              <span class="sw"><i></i><i></i><i></i></span><b>{th.name}</b><span class="muted">{t(th.blurb)}</span>
            </button>
          {/each}
        </div>
        <label class="row"><input type="checkbox" checked={st.followSystem} onchange={(e) => { st.followSystem = (e.target as HTMLInputElement).checked; localStorage.setItem("ss:followSystem", st.followSystem ? "1" : "0"); }} /> {t("s.follow")} <em class="muted">{t("s.follow.hint")}</em></label>
        <label class="row"><input type="checkbox" checked={st.effects} onchange={(e) => { st.effects = (e.target as HTMLInputElement).checked; localStorage.setItem("ss:effects", st.effects ? "1" : "0"); }} /> {t("s.effects")}</label>
        <h3>{t("s.font")} <span class="v">{st.fontSize}px</span></h3>
        <input aria-label={t("s.font")} type="range" min="10" max="20" step="1" value={st.fontSize} onchange={(e) => { st.fontSize = +(e.target as HTMLInputElement).value; localStorage.setItem("ss:fontSize", String(st.fontSize)); }} />

      {:else}
        {#if diag}
          <div class="diag-summary"><span>Conn {diag.version}</span><span>{t("s.diag.tabs")}: {diag.sessions}</span><span>{t("s.diag.connected")}: {diag.agentConnections?.length ?? 0}</span></div>
          <DiagnosticConnections compact connections={diag.agentConnections ?? []} />
          <details class="advanced"><summary>{t("s.diag.details")}</summary><dl class="diag">
            <dt>{t("s.diag.version")}</dt><dd>Conn {diag.version}</dd>
            <dt>{t("s.diag.tabs")}</dt><dd>{diag.sessions}</dd>
            <dt>{t("s.diag.socket")}</dt><dd><code>{diag.socket}</code></dd>
            <dt>{t("s.diag.config")}</dt><dd><code>{diag.configDir}</code></dd>
            <dt>{t("s.diag.policy")}</dt><dd><code>{diag.policyPath}</code></dd>
            <dt>{t("s.diag.audit")}</dt><dd><code>{diag.auditPath}</code> <span class="muted">· {t("s.diag.log")}</span></dd>
            <dt>{t("s.diag.cli")}</dt>
            <dd class="rowdd">{#if diag.cli.onPath}<code>{diag.cli.onPath}</code>{:else}<span class="muted">{t("s.diag.cli.none")}</span>{/if}
              {#if diag.cli.canInstall}<button class="btn mini" onclick={async () => { try { toast(t("cli.installed", { path: await cmd<string>("install_cli") }), "ok"); loadDiag(); } catch (e) { toast(String(e), "danger"); } }}>{t("s.diag.cli.install")}</button>{/if}</dd>
            {#if diag.cli.canConfigure}<dt>{t("s.connect.title")}</dt><dd><button class="btn mini" onclick={() => st.settingsTab = "agents"}>{t("agents.title")}</button></dd>{/if}
            <dt>{t("s.diag.plugins")}</dt>
            <dd class="plugins">
              {#each diag.clients ?? [] as client}
                <span class="plug" class:ok={client.state === "configured" || client.state === "external"}><span class="sw" style:background={agentColor(client.id)}></span>{client.name}<em>{t(`agents.state.${client.state}`)}</em></span>
              {/each}
              {#if diag.configError}<span class="muted">{t("s.diag.config.failed")}</span>{/if}
              <p class="muted">{t("s.diag.config.note")}</p>
            </dd>
            <dt>{t("s.diag.connected")}</dt>
            <dd><DiagnosticConnections connections={diag.agentConnections ?? []} /></dd>
          </dl></details>
        {/if}
      {/if}
    </section>
    {/key}
  </div>

</aside>

<style>
  .apply-note { font-size: 11px; margin: -8px 0 14px; }
  .discard { padding: 10px 16px; display: flex; gap: 8px; align-items: center; background: var(--surface2); font-size: 12px; flex-wrap: wrap; }
  .discard span { flex: 1; }
  .numeric { display: inline-flex; align-items: center; gap: 6px; }
  .numeric input { width: 72px; border: 1px solid var(--line); border-radius: 6px; background: var(--bg); color: var(--fg); padding: 4px 6px; font: inherit; }
  .diag-summary { display: flex; gap: 10px 20px; flex-wrap: wrap; padding: 12px 0 20px; font-size: 12px; }

  .scope-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; margin: 12px 0; font-size: 12px; color: var(--muted); }
  .scope-row select { background: var(--surface2); color: var(--fg); border: 1px solid var(--line); border-radius: 7px; padding: 6px; }
  .advanced { border-top: 1px solid var(--line); padding: 12px 0; margin: 12px 0; }
  .advanced > summary, .scope-row summary { cursor: pointer; color: var(--muted); font-size: 12px; }
  .advanced p { line-height: 1.6; font-size: 12px; }
  .rules summary { cursor: pointer; } .rules code { display: block; white-space: pre-wrap; overflow-wrap: anywhere; margin: 6px 0; color: var(--muted); }

  .scrim { position: absolute; inset: 0; z-index: 40; background: rgba(0,0,0,.25); }
  .sheet { z-index: 41; display: flex; flex-direction: column; outline: 0; width: min(820px, 94vw); height: min(660px, calc(100dvh - 88px)); }
  .category-list { padding: 12px 8px; overflow: auto; border-right: 1px solid var(--line); background: color-mix(in srgb, var(--bg) 25%, transparent); }
  .category-list button { display: block; width: 100%; padding: 10px 12px; margin-bottom: 3px; border: 0; border-radius: 9px; background: transparent; color: var(--muted); text-align: left; cursor: pointer; font: 12.5px var(--font-ui); transition: background 160ms, color 160ms; }
  .category-list button:hover, .category-list button.on { background: var(--surface2); color: var(--fg); }
  .category-list button.on { font-weight: 600; }
  .category-list button:focus-visible { outline: 2px solid var(--agent); outline-offset: -2px; }
  .settings-content { animation: settings-content-in 220ms cubic-bezier(.22, 1, .36, 1) both; }
  @keyframes settings-content-in { from { opacity: 0; transform: translateX(8px); } to { opacity: 1; transform: translateX(0); } }
  h2 { margin: 0 0 18px; font-size: 15px; font-weight: 600; }
  @media (prefers-reduced-motion: reduce) { .settings-content { animation: none; } }
  @media (max-width: 560px) { .body { grid-template-columns: 112px minmax(0, 1fr); } .category-list { padding: 8px 4px; } .category-list button { padding: 9px 6px; font-size: 11.5px; } header { flex-wrap: wrap; } .scope { order: 3; flex-basis: 100%; } }
  .title { flex: 1; }
  footer { border-top: 1px solid var(--line); padding: 7px 14px; }
  header { display: flex; align-items: center; gap: 12px; padding: 12px 16px; border-bottom: 1px solid var(--line); }
  .title { font-weight: 700; font-size: 14px; }
  .scope { flex: 1; display: inline-flex; gap: 2px; background: var(--surface2); padding: 3px; border-radius: 10px; max-width: 320px; }
  .scope button { flex: 1; display: inline-flex; align-items: center; justify-content: center; gap: 6px; border: 0; background: transparent; color: var(--muted); padding: 5px 8px; border-radius: 8px; cursor: pointer; font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .scope button.on { background: var(--surface); color: var(--fg); font-weight: 600; box-shadow: 0 1px 4px rgba(0,0,0,.18); }
  .scope .dot { width: 7px; height: 7px; border-radius: 50%; flex: 0 0 7px; }
  .body { display: grid; grid-template-columns: 154px minmax(0, 1fr); flex: 1; min-height: 0; overflow: hidden; }
  @media (max-width: 560px) { .body { grid-template-columns: 112px minmax(0, 1fr); } }
  section { min-width: 0; padding: 14px 18px; overflow: auto; }
  h3 { margin: 18px 0 8px; font-size: 11px; text-transform: uppercase; letter-spacing: .08em; color: var(--muted); display: flex; gap: 8px; align-items: baseline; }
  h3:first-child { margin-top: 4px; }
  h3 em { text-transform: none; letter-spacing: 0; font-style: normal; font-weight: 400; }
  h3 .v { margin-left: auto; color: var(--fg); font-weight: 600; text-transform: none; letter-spacing: 0; font-variant-numeric: tabular-nums; }
  .seg { display: grid; grid-template-columns: 1fr; gap: 2px; }
  .seg button { display: flex; flex-direction: column; gap: 3px; align-items: flex-start; background: transparent; border: 1px solid transparent; border-radius: 9px; padding: 8px 10px; cursor: pointer; text-align: left; }
  .seg button span { color: var(--muted); font-size: 11px; }
  .seg button:hover, .seg button.on { background: var(--surface2); }
  .row { display: flex; gap: 8px; align-items: center; margin: 10px 0; }
  .row em { font-style: normal; margin-left: 6px; }
  .presets { display: flex; gap: 6px; flex-wrap: wrap; margin-bottom: 8px; }
  .pill { background: var(--surface2); border: 1px solid var(--line); border-radius: 999px; padding: 4px 11px; cursor: pointer; font-size: 12px; color: var(--muted); }
  .pill.on { color: var(--fg); border-color: var(--agent); box-shadow: 0 0 0 1px var(--agent) inset; }
  .pill.mono { font-family: var(--font-mono); font-size: 11px; max-width: 220px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tools { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 240px), 1fr)); gap: 6px 16px; }
  .tg { min-width: 0; }
  .tg b { display: block; font-size: 11px; color: var(--muted); margin: 6px 0 2px; }
  .tool { margin: 2px 0; font-size: 12px; gap: 6px; min-width: 0; }
  .tool code { flex: 0 0 auto; }
  .tool span { min-width: 0; flex: 1; font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .agent { display: flex; align-items: center; gap: 8px; padding: 6px 0; }
  .agent b { color: var(--a); }
  .agent .sw { background: var(--a); }
  .chips { display: flex; gap: 3px; flex-wrap: wrap; }
  .chips code { font-size: 10px; padding: 1px 5px; border-radius: 999px; background: var(--surface2); color: var(--muted); }
  .sw { width: 10px; height: 10px; border-radius: 50%; display: inline-block; flex: 0 0 10px; }
  .chip { background: color-mix(in srgb, var(--warn) 15%, var(--surface2)); border: 1px solid color-mix(in srgb, var(--warn) 40%, transparent); border-radius: 999px; padding: 3px 10px; margin: 0 6px 6px 0; cursor: pointer; font-size: 12px; }
  input[type=range] { width: 100%; }
  .test { display: flex; gap: 10px; align-items: center; }
  .test input, textarea { width: 100%; background: var(--bg); border: 1px solid var(--line); border-radius: 8px; color: var(--fg); padding: 8px 10px; font: 12.5px var(--font-mono); outline: 0; }
  .test input:focus, textarea:focus { border-color: var(--agent); }
  .recent { display: flex; gap: 6px; flex-wrap: wrap; align-items: center; margin-top: 8px; font-size: 11px; }
  textarea { min-height: 280px; resize: vertical; margin-top: 8px; }
  .verdict { padding: 3px 10px; border-radius: 999px; font-weight: 700; font-size: 12px; white-space: nowrap; }
  .segs { list-style: none; margin: 8px 0 0; padding: 0; display: grid; gap: 4px; }
  .segs li { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; font-size: 12px; }
  .segs .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--muted); }
  .segs li.confirm .dot { background: var(--warn); } .segs li.deny .dot { background: var(--danger); }
  .tag { font-size: 10.5px; padding: 1px 6px; border-radius: 999px; background: var(--surface2); color: var(--muted); }
  .tag.danger { background: color-mix(in srgb, var(--danger) 20%, transparent); color: var(--danger); }
  .verdict.allow { background: color-mix(in srgb, var(--ok) 20%, transparent); color: var(--ok); }
  .verdict.confirm { background: color-mix(in srgb, var(--warn) 20%, transparent); color: var(--warn); }
  .verdict.deny { background: color-mix(in srgb, var(--danger) 20%, transparent); color: var(--danger); }
  .rules { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; }
  .rules li { display: grid; grid-template-columns: 64px minmax(0, 1fr); gap: 8px; align-items: center; padding: 4px 8px; border-radius: 6px; font-size: 12px; border: 1px solid transparent; }
  .rules li.hit { background: color-mix(in srgb, var(--warn) 12%, transparent); border-color: color-mix(in srgb, var(--warn) 40%, transparent); }
  .rules li.deny.hit { background: color-mix(in srgb, var(--danger) 12%, transparent); border-color: color-mix(in srgb, var(--danger) 40%, transparent); }
  .rules .kind { font-size: 10px; text-transform: uppercase; letter-spacing: .06em; color: var(--warn); }
  .rules li.deny .kind { color: var(--danger); }
  .rules code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rules .lbl { color: var(--muted); font-size: 11px; white-space: nowrap; }
  .facts { display: flex; gap: 6px; flex-wrap: wrap; margin-top: 8px; }
  .disclose { display: block; width: 100%; text-align: left; background: transparent; border: 0; color: var(--fg); padding: 10px 0 4px; margin-top: 14px; cursor: pointer; font-size: 12.5px; border-top: 1px solid var(--line); }
  .disclose em { font-style: normal; font-size: 11px; margin-left: 6px; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 8px; }
  .themes { display: grid; grid-template-columns: repeat(2, 1fr); gap: 10px; }
  .tcard { display: grid; gap: 4px; justify-items: start; background: var(--tbg); color: var(--tfg); border: 1px solid var(--line); border-radius: 12px; padding: 12px; cursor: pointer; text-align: left; }
  .tcard.on { border-color: var(--agent); box-shadow: 0 0 0 2px var(--agent); }
  .tcard .sw { display: flex; gap: 4px; width: auto; height: auto; }
  .tcard .sw i { width: 14px; height: 14px; border-radius: 4px; background: var(--ts); }
  .tcard .sw i:nth-child(2) { background: var(--agent); } .tcard .sw i:nth-child(3) { background: var(--tfg); }
  .agent-setup { margin-bottom: 22px; padding-bottom: 18px; border-bottom: 1px solid var(--line); }
  .agent-setup p { font-size: 12px; line-height: 1.55; }
  .config-format { display: flex; align-items: center; gap: 10px; margin: 12px 0; font-size: 12px; }
  .config-format select { background: var(--bg); color: var(--fg); border: 1px solid var(--line); border-radius: 7px; padding: 6px 9px; }
  .agent-setup details { margin-top: 12px; font-size: 12px; }
  .agent-setup summary { cursor: pointer; color: var(--muted); }
  .agent-setup textarea { min-height: 170px; line-height: 1.5; }
  .agent-setup .setup-error { color: var(--danger); overflow-wrap: anywhere; }
  .agent-setup button:disabled { opacity: .55; cursor: default; }
  .diag { display: grid; grid-template-columns: 110px 1fr; gap: 10px 12px; margin: 4px 0; font-size: 12px; align-items: center; }
  .diag dt { color: var(--muted); align-self: start; padding-top: 2px; }
  .diag dd { margin: 0; min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .diag code { font-size: 11.5px; }
  .rowdd { display: flex; gap: 8px; align-items: center; }
  .btn.mini { padding: 2px 8px; font-size: 11px; }
  .plugins { display: flex; flex-direction: column; gap: 6px; }
  .plug { display: flex; align-items: center; gap: 8px; }
  .plug em { font-style: normal; color: var(--muted); font-size: 11px; }
  .plug.ok em { color: var(--ok); }
</style>
