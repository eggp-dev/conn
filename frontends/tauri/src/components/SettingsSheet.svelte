<script lang="ts">
  import AutomationPane from "./AutomationPane.svelte";
  import AgentConnections from "./AgentConnections.svelte";
  import DiagnosticConnections from "./DiagnosticConnections.svelte";
  import ProfilesPane from "./ProfilesPane.svelte";
  import { fly } from "svelte/transition";
  import { st, cur, toast, type Analysis } from "../lib/store.svelte";
  import { cmd, changeMode, type Mode } from "../lib/bridge";
  import { THEMES, agentColor } from "../lib/themes";
  import { t, fmtMs, i18n, setLang, LANGS } from "../lib/i18n.svelte";
  let { onclose }: { onclose: () => void } = $props();
  const NAV = ["profiles", "agents", "automation", "policy", "pacing", "appearance", "diagnostics"] as const;

  // ---- scope: this tab or all tabs; plus stored defaults for new tabs ----
  let scope = $state<"tab" | "all">("tab");
  async function apply(name: string, args: Record<string, unknown>) {
    if (name === "set_mode") {
      for (const id of scope === "all" ? st.order : [st.active]) await changeMode(args.mode as Mode, id);
      return;
    }
    if (scope === "all") { await Promise.all(st.order.map((id) => cmd(name, { ...args, session: id }))); }
    else await cmd(name, args);
  }
  async function saveDefaults() {
    const c = cur();
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
  async function poll() { if (st.settingsTab !== "agents") return; try { live = await cmd<Live[]>("agents"); } catch { live = []; } }
  $effect(() => { void st.active; void st.settingsTab; poll(); const i = setInterval(poll, 2000); return () => clearInterval(i); });

  // ---- policy ----
  let policyText = $state("");
  let policyPath = $state("");
  let policyDirty = $state(false);
  let showSource = $state(false);
  let testCmd = $state("");
  let verdict = $state<Analysis | null>(null);
  type Rules = { rules: { kind: string; label: string; pattern?: string; command?: string; args?: string | null }[]; protected: string[]; opaque: string; isolateDangerous: boolean; requireIntent: boolean; unattended: string; default: string };
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
  $effect(() => { if (st.settingsTab === "policy" && !rules) loadPolicy(); });
  $effect(() => {
    const c = testCmd;
    if (!c.trim()) { verdict = null; return; }
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
  function setTheme(id: string) { st.theme = id as any; localStorage.setItem("ss:theme", id); }

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
  $effect(() => { if (st.settingsTab === "agents") loadSetup(); });

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

  function key(e: KeyboardEvent) { if (e.key === "Escape") onclose(); }
</script>

<aside class="sheet" transition:fly={{ x: 40, duration: 220 }} onkeydown={key} tabindex="-1">
  <header>
    <span class="title">{t("s.title")}</span>
    {#if st.settingsTab !== "profiles" && st.settingsTab !== "automation"}<span class="scope">
      <button class:on={scope === "tab"} onclick={() => (scope = "tab")}><span class="dot" style:background={cur().controller.type === "agent" ? agentColor(cur().controller.agentId) : "var(--muted)"}></span>{cur().title}</button>
      <button class:on={scope === "all"} onclick={() => (scope = "all")}>{t("s.scope.all")} · {st.order.length}</button>
    </span>{/if}
    <button class="btn ghost" onclick={onclose}>{t("close")} <kbd>esc</kbd></button>
  </header>
  <div class="body">
    <nav>
      {#each NAV as id}
        <button class:on={st.settingsTab === id} onclick={() => (st.settingsTab = id)}>{t(`s.nav.${id}`)}</button>
      {/each}
      <span class="spacer"></span>
      {#if st.settingsTab !== "profiles" && st.settingsTab !== "automation"}<p class="muted small">{t("s.scope.hint")}</p>
      <button class="link" onclick={saveDefaults}>{t("s.defaults.use")}</button>{/if}
    </nav>
    <section>
      {#if st.settingsTab === "profiles"}
        <ProfilesPane />
      {:else if st.settingsTab === "automation"}
        <AutomationPane />
      {:else if st.settingsTab === "agents"}
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
        <div class="seg">
          {#each ["observe", "copilot", "autopilot"] as id}
            <button class:on={cur().mode === id} onclick={() => apply("set_mode", { mode: id })}><b>{t(`mode.${id}`)}</b><span>{t(`mode.${id}.desc`)}</span></button>
          {/each}
        </div>
        <label class="row"><input type="checkbox" checked={cur().gate} onchange={(e) => apply("set_control_gate", { ask: (e.target as HTMLInputElement).checked })} /> <span>{t("gate")} <em class="muted">{t("gate.desc")}</em></span></label>

        <h3>{t("s.tools")} <em class="muted">{t("s.tools.hint")}</em></h3>
        <div class="presets">
          {#each PRESETS as [k, p]}
            <button class="pill" class:on={presetOn(p)} onclick={() => apply("set_affordances", { allow: p })}>{t(k)}</button>
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

      {:else if st.settingsTab === "policy"}
        <h3>{t("s.policy.test")} <em class="muted">{t("s.policy.test.hint")}</em></h3>
        <div class="test">
          <input placeholder="cd .. && rm -rf -- hello-mel" bind:value={testCmd} spellcheck="false" />
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
                <code>{r.pattern ?? `${r.command}${r.args ? " " + r.args : ""}`}</code>
                <span class="lbl">{r.label}</span>
              </li>
            {/each}
          </ol>
          <div class="facts">
            <span class="tag">{t("s.policy.opaque", { v: rules.opaque })}</span>
            {#if rules.isolateDangerous}<span class="tag">{t("s.policy.isolate")}</span>{/if}
            {#if rules.requireIntent}<span class="tag">{t("s.policy.intent")}</span>{/if}
            <span class="tag">{t("s.policy.unattended", { v: rules.unattended })}</span>
            <span class="tag">{t("s.policy.default", { v: rules.default })}</span>
          </div>
          {#if rules.protected.length}
            <h3>{t("s.policy.protected")}</h3>
            <div class="facts">{#each rules.protected as p}<code class="tag">{p}</code>{/each}</div>
          {/if}
        {/if}

        <button class="disclose" onclick={() => (showSource = !showSource)}>{showSource ? "▾" : "▸"} {t("s.policy.source")} <em class="muted">{policyPath}</em></button>
        {#if showSource}
          <textarea bind:value={policyText} oninput={() => (policyDirty = true)} spellcheck="false"></textarea>
          <div class="actions"><button class="btn ghost" onclick={loadPolicy}>{t("revert")}</button><button class="btn primary" disabled={!policyDirty} onclick={savePolicy}>{t("save")}</button></div>
          <p class="muted">{t("s.policy.source.hint")}</p>
        {/if}

      {:else if st.settingsTab === "pacing"}
        <h3>{t("s.pacing.presets")}</h3>
        <div class="seg">
          {#each PACE_PRESETS as [k, patch, gate]}
            <button onclick={() => pacePreset(patch, gate)}><b>{t(`s.pacing.${k}`)}</b><span>{t(`s.pacing.${k}.desc`)}</span></button>
          {/each}
        </div>
        <h3>{t("s.grace")} <span class="v">{fmtMs(cur().pacing.enterGraceMs)}</span></h3>
        <p class="muted">{t("s.grace.hint")}</p>
        <input type="range" min="0" max="5000" step="100" value={cur().pacing.enterGraceMs} onchange={(e) => pace({ enterGraceMs: +(e.target as HTMLInputElement).value })} />
        <div class="gpreview" style:--g="{Math.max(300, cur().pacing.enterGraceMs)}ms"><span class="fill"></span></div>
        <h3>{t("s.interval")} <span class="v">{cur().pacing.minWriteIntervalMs} ms</span></h3>
        <p class="muted">{t("s.interval.hint")}</p>
        <input type="range" min="0" max="500" step="10" value={cur().pacing.minWriteIntervalMs} onchange={(e) => pace({ minWriteIntervalMs: +(e.target as HTMLInputElement).value })} />
        <div class="preview" style:--iv="{Math.max(20, cur().pacing.minWriteIntervalMs)}ms"><span class="cur">$</span> <span class="typed">kubectl get pods -n prod</span></div>
        <h3>{t("s.lease")} <span class="v">{cur().pacing.leaseTtlSecs} s</span></h3>
        <p class="muted">{t("s.lease.hint")}</p>
        <input type="range" min="10" max="600" step="10" value={cur().pacing.leaseTtlSecs} onchange={(e) => pace({ leaseTtlSecs: +(e.target as HTMLInputElement).value })} />
        <h3>{t("s.approval")} <span class="v">{cur().pacing.approvalTtlSecs} s</span></h3>
        <p class="muted">{t("s.approval.hint")}</p>
        <input type="range" min="30" max="900" step="30" value={cur().pacing.approvalTtlSecs} onchange={(e) => pace({ approvalTtlSecs: +(e.target as HTMLInputElement).value })} />

      {:else if st.settingsTab === "appearance"}
        <h3>{t("s.language")}</h3>
        <div class="presets">
          {#each LANGS as l}<button class="pill" class:on={i18n.lang === l.id} onclick={() => setLang(l.id)}>{l.name}</button>{/each}
        </div>
        <h3>{t("s.theme")}</h3>
        <div class="themes">
          {#each Object.values(THEMES) as th}
            <button class="tcard" class:on={st.theme === th.id} onclick={() => setTheme(th.id)} style:--tbg={th.tokens.bg} style:--tfg={th.tokens.fg} style:--ts={th.tokens.surface2}>
              <span class="sw"><i></i><i></i><i></i></span><b>{th.name}</b><span class="muted">{t(th.blurb)}</span>
            </button>
          {/each}
        </div>
        <label class="row"><input type="checkbox" checked={st.followSystem} onchange={(e) => { st.followSystem = (e.target as HTMLInputElement).checked; localStorage.setItem("ss:followSystem", st.followSystem ? "1" : "0"); }} /> {t("s.follow")} <em class="muted">{t("s.follow.hint")}</em></label>
        <label class="row"><input type="checkbox" checked={st.effects} onchange={(e) => { st.effects = (e.target as HTMLInputElement).checked; localStorage.setItem("ss:effects", st.effects ? "1" : "0"); }} /> {t("s.effects")}</label>
        <h3>{t("s.font")} <span class="v">{st.fontSize}px</span></h3>
        <input type="range" min="10" max="20" step="1" value={st.fontSize} onchange={(e) => { st.fontSize = +(e.target as HTMLInputElement).value; localStorage.setItem("ss:fontSize", String(st.fontSize)); }} />

      {:else}
        {#if diag}
          <dl class="diag">
            <dt>{t("s.diag.version")}</dt><dd>Conn {diag.version}</dd>
            <dt>{t("s.diag.tabs")}</dt><dd>{diag.sessions}</dd>
            <dt>{t("s.diag.socket")}</dt><dd><code>{diag.socket}</code></dd>
            <dt>{t("s.diag.config")}</dt><dd><code>{diag.configDir}</code></dd>
            <dt>{t("s.diag.policy")}</dt><dd><code>{diag.policyPath}</code></dd>
            <dt>{t("s.diag.audit")}</dt><dd><code>{diag.auditPath}</code> <span class="muted">· {t("s.diag.log")}</span></dd>
            <dt>{t("s.diag.cli")}</dt>
            <dd class="rowdd">{#if diag.cli.onPath}<code>{diag.cli.onPath}</code>{:else}<span class="muted">{t("s.diag.cli.none")}</span>{/if}
              {#if diag.cli.canInstall}<button class="btn mini" onclick={async () => { try { toast(t("cli.installed", { path: await cmd<string>("install_cli") }), "ok"); loadDiag(); } catch (e) { toast(String(e), "danger"); } }}>{t("s.diag.cli.install")}</button>{/if}</dd>
            {#if diag.cli.canConfigure}<dt>{t("s.connect.title")}</dt><dd><button class="btn mini" onclick={() => st.settingsTab = "agents"}>{t("s.connect.copy")}</button></dd>{/if}
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
          </dl>
        {/if}
      {/if}
    </section>
  </div>
</aside>

<style>
  .sheet { position: absolute; top: 0; right: 0; bottom: 0; width: 600px; max-width: 92vw; z-index: 31; background: var(--surface); border-left: 1px solid var(--line); box-shadow: var(--shadow); display: flex; flex-direction: column; outline: 0; }
  header { display: flex; align-items: center; gap: 12px; padding: 12px 16px; border-bottom: 1px solid var(--line); }
  .title { font-weight: 700; font-size: 14px; }
  .scope { flex: 1; display: inline-flex; gap: 2px; background: var(--surface2); padding: 3px; border-radius: 10px; max-width: 320px; }
  .scope button { flex: 1; display: inline-flex; align-items: center; justify-content: center; gap: 6px; border: 0; background: transparent; color: var(--muted); padding: 5px 8px; border-radius: 8px; cursor: pointer; font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .scope button.on { background: var(--surface); color: var(--fg); font-weight: 600; box-shadow: 0 1px 4px rgba(0,0,0,.18); }
  .scope .dot { width: 7px; height: 7px; border-radius: 50%; flex: 0 0 7px; }
  .body { display: grid; grid-template-columns: 130px minmax(0, 1fr); flex: 1; min-height: 0; }
  nav { display: flex; flex-direction: column; gap: 2px; padding: 10px; border-right: 1px solid var(--line); }
  nav button { text-align: left; background: transparent; border: 0; padding: 8px 10px; border-radius: 8px; cursor: pointer; color: var(--muted); font-size: 12.5px; }
  nav button.on { background: var(--surface2); color: var(--fg); font-weight: 600; }
  nav .spacer { flex: 1; }
  nav .small { font-size: 10.5px; margin: 0 4px 6px; line-height: 1.4; }
  nav .link { color: var(--agent); font-size: 11.5px; padding: 6px 8px; }
  section { min-width: 0; padding: 14px 18px; overflow: auto; }
  h3 { margin: 18px 0 8px; font-size: 11px; text-transform: uppercase; letter-spacing: .08em; color: var(--muted); display: flex; gap: 8px; align-items: baseline; }
  h3:first-child { margin-top: 4px; }
  h3 em { text-transform: none; letter-spacing: 0; font-style: normal; font-weight: 400; }
  h3 .v { margin-left: auto; color: var(--fg); font-weight: 600; text-transform: none; letter-spacing: 0; font-variant-numeric: tabular-nums; }
  .seg { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; }
  .seg button { display: flex; flex-direction: column; gap: 3px; align-items: flex-start; background: var(--surface2); border: 1px solid var(--line); border-radius: 10px; padding: 10px; cursor: pointer; text-align: left; }
  .seg button span { color: var(--muted); font-size: 11px; }
  .seg button.on { border-color: var(--agent); box-shadow: 0 0 0 1px var(--agent) inset; }
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
  .preview { margin: 8px 0 4px; padding: 8px 10px; background: var(--bg); border: 1px solid var(--line); border-radius: 8px; font-family: var(--font-mono); font-size: 12px; }
  .preview .typed { display: inline-block; overflow: hidden; white-space: nowrap; vertical-align: bottom; width: 0; animation: type calc(var(--iv) * 24) steps(24) infinite; }
  @keyframes type { 0% { width: 0; } 85%, 100% { width: 24ch; } }
  .gpreview { height: 3px; margin: 8px 0 2px; background: var(--surface2); border-radius: 2px; overflow: hidden; }
  .gpreview .fill { display: block; height: 100%; background: var(--warn); transform-origin: left; animation: drain var(--g) linear infinite; }
  @keyframes drain { 0% { transform: scaleX(1); } 100% { transform: scaleX(0); } }
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
  .rules li { display: grid; grid-template-columns: 52px 1fr auto; gap: 8px; align-items: center; padding: 4px 8px; border-radius: 6px; font-size: 12px; border: 1px solid transparent; }
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
