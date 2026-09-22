<script lang="ts">
  import { useConnApp } from '../runtime/context';
  const app = useConnApp();
  const { invoke, t, profileState, refreshProfiles } = app;

  let { ondirty = (_dirty: boolean) => {} } = $props<{ ondirty?: (dirty: boolean) => void }>();
  $effect(() => ondirty(dirty));
  import { onMount } from "svelte";
  import { type ProfileConfig, type Profile, type Backend, type Catalog, type Availability } from '../lib/profiles.svelte';
  let config = $state<ProfileConfig | null>(null);
  let selected = $state("");
  let busy = $state(false);
  let dirty = $state(false);
  let error = $state("");
  let notice = $state("");
  let argsText = $state("");
  let envText = $state("");
  const current = $derived(config?.profiles.find(p => p.id === selected));
  function select(id: string) {
    if (current && !commitFields()) return;
    selected = id;
    const p = config?.profiles.find(p => p.id === id);
    argsText = p?.args.join("\n") ?? "";
    envText = Object.entries(p?.env ?? {}).map(([k,v]) => `${k}=${v}`).join("\n");
    notice = ""; error = "";
  }
  function commitFields() {
    if (!current) return true;
    const env: Record<string,string> = {};
    for (const line of envText.split("\n").filter(s => s.trim())) {
      const at = line.indexOf("=");
      if (at < 1) { error = t("profiles.envError"); return false; }
      const key = line.slice(0,at).trim();
      if (!key || Object.prototype.hasOwnProperty.call(env,key)) { error = t("profiles.envError"); return false; }
      Object.defineProperty(env,key,{value:line.slice(at+1),enumerable:true,writable:true,configurable:true});
    }
    current.args = argsText ? argsText.split("\n") : [];
    current.env = env;
    return true;
  }
  async function load() {
    busy = true; error = "";
    try {
      const catalog = await refreshProfiles();
      selected = "";
      config = structuredClone($state.snapshot(catalog.config));
      select(config.defaultProfile); dirty = false;
    } catch (e) { error = String(e); }
    finally { busy = false; }
  }
  onMount(load);
  function add(backend: Backend = "local") {
    if (!config || !commitFields()) return;
    const local = config.profiles.find(p => p.backend === "local");
    const p: Profile = { id: `profile-${crypto.randomUUID()}`, name: t("profiles.new"), enabled:true,backend,
      shell:local?.shell ?? "posix",program:local?.program ?? "/bin/sh",args:local?.args.slice() ?? ["-l"],cwd:null,env:{},target:null,port:null };
    config.profiles.push(p);select(p.id);dirty = true;
  }
  function changeBackend(backend: Backend) {
    if (!current) return;
    const local = config?.profiles.find(p => p.backend === "local" && p.id !== selected)
      ?? profileState.catalog?.config.profiles.find(p => p.backend === "local");
    current.backend = backend;current.target = null;current.port = null;current.cwd = null;current.env = {};envText = "";
    current.program = backend === "ssh" ? "" : backend === "local" ? (local?.program ?? "/bin/sh") : "/bin/sh";
    current.shell = backend === "ssh" ? "custom" : backend === "local" ? (local?.shell ?? "posix") : "posix";
    current.args = backend === "local" ? (local?.args.slice() ?? ["-l"]) : [];argsText = current.args.join("\n");dirty = true;notice = "";
  }
  async function discover() {
    if (!config || !commitFields()) return;
    busy = true;error = "";
    try {
      const found = await invoke<Profile[]>("profiles_discover");
      let n = 0;
      for (const p of found) {
        if (config.profiles.some(old => old.backend === p.backend && old.program === p.program && old.target === p.target)) continue;
        if (config.profiles.some(old=>old.id===p.id)) p.id=`profile-${crypto.randomUUID()}`;
        config.profiles.push(p);n++;
      }
      dirty ||= n > 0;notice = t("profiles.found",{n});
    } catch(e) {error=String(e);} finally {busy=false;}
  }
  async function save() {
    if (!config || !commitFields()) return;
    busy=true;error="";notice="";
    try {
      const catalog=await invoke<Catalog>("profiles_save",{config:$state.snapshot(config)});
      profileState.catalog=catalog;config=structuredClone($state.snapshot(catalog.config));dirty=false;notice=t("profiles.saved");
    } catch(e) {error=String(e);} finally {busy=false;}
  }
  async function test() {
    if (!current || !commitFields()) return;
    busy=true;error="";notice="";
    try {
      const result=await invoke<Availability>("profiles_test",{profile:$state.snapshot(current)});
      if (result.available) notice=result.message;else error=result.message;
    } catch(e) {error=String(e);} finally {busy=false;}
  }
  function remove() {
    if (!config || !current || current.id===config.defaultProfile) return;
    config.profiles=config.profiles.filter(p=>p.id!==selected);selected="";select(config.defaultProfile);dirty=true;
  }
</script>

<div class="profiles">

  <p class="muted">{t("profiles.description")}</p>
  <div class="actions">
    <button class="btn" disabled={busy || !config} onclick={()=>add()}>{t("profiles.add")}</button>
    <button class="btn ghost" disabled={busy || !config} onclick={discover}>{t("profiles.detect")}</button>
    <button class="btn ghost" disabled={busy} onclick={load}>{t("profiles.reload")}</button>
  </div>
  {#if config}
    <div class="profile-list" aria-label={t("profiles.title")}>
      {#each config.profiles as p (p.id)}
        {@const availability=profileState.catalog?.availability[p.id]}
        <button class="profile-row" class:selected={selected===p.id} aria-pressed={selected===p.id} onclick={()=>select(p.id)} disabled={busy}>
          <span class="profile-name">{p.name}{p.id===config.defaultProfile ? ` · ${t("profiles.defaultBadge")}` : ""}</span>
          <small>{p.backend} · {p.shell}{!p.enabled ? ` · ${t("profiles.disabled")}` : availability && !availability.available ? ` · ${t("profiles.unavailable")}` : ""}</small>
        </button>
      {/each}
    </div>
    {#if current}
      <fieldset disabled={busy} oninput={()=>{dirty=true;notice="";}}>
        <legend>{t("profiles.edit")}</legend>
        <label class="field">{t("profiles.name")}<input bind:value={current.name} maxlength="128" /></label>
        <div class="pair">
          <label class="field">{t("profiles.backend")}<select value={current.backend} onchange={e=>changeBackend(e.currentTarget.value as Backend)}>
            <option value="local">{t("profiles.local")}</option><option value="wsl">WSL</option><option value="ssh">SSH</option><option value="docker">Docker</option>
          </select></label>
          <label class="field">{t("profiles.shell")}<select bind:value={current.shell}>
            <option value="posix">bash / zsh / sh</option><option value="fish">fish</option><option value="power_shell">PowerShell</option><option value="cmd">cmd</option><option value="custom">{t("profiles.custom")}</option>
          </select></label>
        </div>
        {#if current.backend!=="local"}
          <label class="field">{t(`profiles.target.${current.backend}`)}<input bind:value={current.target} placeholder={current.backend==="ssh" ? "user@host / SSH alias" : current.backend==="wsl" ? "Ubuntu" : "container-name"} /></label>
        {/if}
        {#if current.backend==="ssh"}
          <label class="field">{t("profiles.port")}<input type="number" min="1" max="65535" value={current.port ?? ""} oninput={e=>{current!.port=e.currentTarget.value ? Number(e.currentTarget.value) : null;}} placeholder="22" /></label>
          <p class="muted small">{t("profiles.sshHint")}</p>
        {/if}
        <label class="field">{t("profiles.cwd")}<input bind:value={current.cwd} placeholder={t("profiles.inherit")} /></label>
        <details class="advanced"><summary>{t("s.advanced")}</summary>
        <label class="field">{t("profiles.program")}<input bind:value={current.program} placeholder={current.backend==="ssh" ? t("profiles.serverDefault") : "pwsh.exe / /bin/bash"} /></label>
        <label class="field">{t("profiles.args")}<textarea bind:value={argsText} rows="3" spellcheck="false" placeholder="-l"></textarea></label>

        <label class="field">{t("profiles.env")}<textarea bind:value={envText} rows="3" spellcheck="false" placeholder="LANG=ko_KR.UTF-8"></textarea></label>
        </details>
        <label class="enabled"><input type="checkbox" bind:checked={current.enabled} disabled={current.id===config.defaultProfile} />{t("profiles.enabled")}</label>
        <label class="enabled"><input type="checkbox" checked={current.id===config.defaultProfile} disabled={!current.enabled || current.id===config.defaultProfile} onchange={() => { config!.defaultProfile = current!.id; dirty = true; }} />{t("profiles.makeDefault")}</label>
        {#if current.backend!=="local" || current.shell!=="posix"}<p class="review-note">{t("profiles.review")}</p>{/if}
        {#if !dirty && profileState.catalog?.availability[current.id]?.available===false}<p class="error">{profileState.catalog.availability[current.id].message}</p>{/if}
        <div class="actions">
          <button class="btn" onclick={test}>{t("profiles.test")}</button>
          <button class="btn ghost" disabled={current.id===config.defaultProfile} onclick={remove}>{t("profiles.remove")}</button>
        </div>
      </fieldset>
    {/if}
    {#if dirty}<p class="muted">{t("s.unsaved")}</p><button class="btn save" disabled={busy || !dirty} onclick={save}>{busy ? t("profiles.working") : t("profiles.save")}</button>{/if}
  {/if}
  <div aria-live="polite">{#if error}<p class="error" role="alert">{error}</p>{/if}{#if notice}<p class="notice">{notice}</p>{/if}</div>
</div>

<style>
 .advanced summary { cursor: pointer; color: var(--muted); font-size: 12px; } .advanced .field { margin-top: 12px; } .save { position: sticky; bottom: 0; z-index: 1; }
  .profiles{display:grid;gap:14px;min-width:0}.profiles h3,.profiles p{margin:0}.profiles p{line-height:1.5;font-size:12px}.actions{display:flex;gap:8px;flex-wrap:wrap}.field{display:grid;gap:6px;font-size:12px;min-width:0}input,select,textarea{width:100%;box-sizing:border-box;background:var(--bg);color:var(--fg);border:1px solid var(--line);border-radius:6px;padding:8px;font:inherit}textarea{resize:vertical;min-height:60px}.pair{display:grid;grid-template-columns:1fr 1fr;gap:12px}.profile-list{display:grid;gap:4px;max-height:185px;overflow:auto}.profile-row{text-align:left;display:grid;gap:4px;background:var(--surface);color:var(--fg);border:1px solid var(--line);border-radius:8px;padding:10px;cursor:pointer}.profile-row.selected{border-color:var(--agent,#8b7cff);background:var(--surface2)}.profile-name{overflow-wrap:anywhere}small,.muted{color:var(--muted)}fieldset{border:1px solid var(--line);border-radius:10px;padding:14px;display:grid;gap:12px;min-width:0}legend{padding:0 6px;font-size:12px}.enabled{display:flex;align-items:center;gap:8px;font-size:12px}.enabled input{width:auto}.error{color:var(--danger);overflow-wrap:anywhere}.notice{color:var(--fg)}.review-note{border-left:2px solid var(--warn);padding-left:10px;color:var(--muted)}.save{justify-self:start}button:focus-visible,input:focus-visible,textarea:focus-visible,select:focus-visible{outline:2px solid var(--agent,#8b7cff);outline-offset:2px}button:disabled{opacity:.5;cursor:default}
</style>
