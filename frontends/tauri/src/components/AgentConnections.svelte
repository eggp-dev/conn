<script lang="ts">
  import { onMount } from "svelte";
  import { cmd } from "../lib/bridge";
  import { t } from "../lib/i18n.svelte";
  type Client = { id: string; name: string; state: string; managed: boolean; connected: boolean; error: string | null; configPath: string; skillPath: string };
  type Catalog = { clients: Client[]; canConfigure: boolean; backupPath: string };
  let catalog = $state<Catalog | null>(null);
  let busy = $state("");
  let error = $state("");
  let notice = $state("");
  let manual = $state<Record<string, string>>({});
  let actionErrors = $state<Record<string, string>>({});
  let refreshing = false;
  let alive = true;
  function message(e: unknown) {
    const code = String(e);
    const key = `agents.error.${code}`;
    const translated = t(key);
    return translated === key ? t("agents.error.unknown") : translated;
  }
  async function refresh() {
    if (refreshing) return;
    refreshing = true;
    try { const next = await cmd<Catalog>("agent_integrations"); if (alive) { catalog = next; error = ""; } }
    catch (e) { if (alive) error = message(e); }
    finally { refreshing = false; }
  }
  onMount(() => { alive = true; void refresh(); const timer = setInterval(() => { if (!busy) void refresh(); }, 3000); return () => { alive = false; clearInterval(timer); }; });
  async function act(client: Client, action: "install" | "remove") {
    busy = client.id; notice = ""; actionErrors[client.id] = "";
    try {
      await cmd(`agent_integration_${action}`, { client: client.id });
      notice = t(action === "install" ? "agents.installed" : "agents.removed", { name: client.name });
      await refresh();
    } catch (e) { actionErrors[client.id] = message(e); }
    finally { busy = ""; }
  }
  async function copy(client: Client) {
    busy = client.id; actionErrors[client.id] = "";
    try {
      const result = await cmd<{ text: string }>("agent_integration_manual", { client: client.id });
      manual[client.id] = result.text;
      try { await navigator.clipboard.writeText(result.text); notice = t("s.connect.copied"); }
      catch { actionErrors[client.id] = t("s.connect.copy_failed"); }
    } catch (e) { actionErrors[client.id] = message(e); }
    finally { busy = ""; }
  }
</script>

<section class="connections" aria-labelledby="connections-title">
  <header><h3 id="connections-title">{t("agents.title")}</h3><button class="link" disabled={!!busy} onclick={refresh}>{t("agents.refresh")}</button></header>
  <p class="intro">{t("agents.intro")}</p>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if notice}<p class="notice" role="status">{notice}</p>{/if}
  {#if !catalog && !error}<p class="intro">{t("agents.loading")}</p>{/if}
  {#if catalog}
    {#if !catalog.canConfigure}<p class="error">{t("s.connect.missing")}</p>{/if}
    <div class="clients">
      {#each catalog.clients as client (client.id)}
        <article class="client" class:connected={client.connected} aria-label={client.name}>
          <div class="client-top">
            <div class="identity"><h4>{client.name}</h4><span class="state" class:attention={client.state === "conflict" || client.state === "needs_update"}>{t(`agents.state.${client.state}`)}</span></div>
            {#if client.connected}<span class="live"><i></i>{t("agents.connected")}</span>{/if}
          </div>
          <p class="client-hint">{t(`agents.hint.${client.id}`)}</p>
          <div class="actions">
            <button class="btn" class:primary={!client.managed} disabled={!!busy || !catalog.canConfigure || client.state === "conflict" || client.state === "external"} onclick={() => act(client, "install")}>
              {busy === client.id ? t("agents.working") : t(client.managed ? "agents.update" : "agents.install")}
            </button>
            {#if client.managed}<button class="link" disabled={!!busy || client.state === "conflict"} onclick={() => act(client, "remove")}>{t("agents.remove")}</button>{/if}
          </div>
          {#if client.error || actionErrors[client.id]}<p class="error" role="alert">{actionErrors[client.id] || message(client.error)}</p>{/if}
          {#if client.state === "external"}<p class="client-hint">{t("agents.external")}</p>{/if}
          <details>
            <summary>{t("agents.details")}</summary>
            <dl><dt>{t("agents.config")}</dt><dd>{client.configPath}</dd><dt>{t("agents.skill")}</dt><dd>{client.skillPath}</dd></dl>
            <p class="client-hint">{t("agents.personal")}</p>
            <button class="link" disabled={!!busy || !catalog.canConfigure} onclick={() => copy(client)}>{t("agents.manual")}</button>
            {#if manual[client.id]}<textarea readonly value={manual[client.id]} aria-label={t("s.connect.view")} spellcheck="false" onfocus={(e) => e.currentTarget.select()}></textarea>{/if}
          </details>
        </article>
      {/each}
    </div>
    <p class="footnote">{t("agents.verify")}</p>
  {/if}
</section>

<style>
  .connections { margin-bottom: 24px; }
  header, .client-top, .identity, .actions { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  header { justify-content: space-between; }
  h3 { margin: 0; font-size: 13px; }
  h4 { margin: 0; font-size: 13px; font-weight: 600; }
  p { line-height: 1.6; }
  .intro, .footnote { color: var(--muted); font-size: 12px; margin: 9px 0 14px; }
  .clients { display: grid; gap: 10px; }
  .client { min-width: 0; border: 1px solid var(--line); border-radius: 12px; padding: 14px; background: var(--bg); transition: border-color 180ms ease; }
  .client.connected { border-color: color-mix(in srgb, var(--ok, #35cba1) 45%, var(--line)); }
  .client-top { justify-content: space-between; gap: 6px; }
  .state { font-size: 10px; color: var(--muted); border: 1px solid var(--line); padding: 2px 6px; border-radius: 5px; }
  .state.attention { color: var(--warn, #ffab47); }
  .live { font-size: 11px; color: var(--ok, #35cba1); display: flex; align-items: center; gap: 5px; }
  .live i { width: 5px; height: 5px; border-radius: 50%; background: currentColor; }
  .client-hint { font-size: 11px; color: var(--muted); margin: 8px 0 10px; }
  .actions { gap: 16px; }
  button { cursor: pointer; font: inherit; font-size: 11px; }
  .btn { color: var(--fg); border: 1px solid var(--line); border-radius: 7px; padding: 7px 10px; background: var(--panel, #171d28); }
  .primary { background: var(--accent, #8877ff); color: #fff; border-color: transparent; }
  .link { color: var(--muted); border: 0; background: none; padding: 4px 0; }
  .link:hover { color: var(--fg); }
  button:disabled { opacity: .45; cursor: default; }
  details { margin-top: 10px; font-size: 11px; }
  summary { cursor: pointer; color: var(--muted); width: fit-content; }
  dl { margin: 12px 0; } dt { color: var(--muted); margin-top: 8px; } dd { margin: 4px 0 0; overflow-wrap: anywhere; font-family: var(--mono); font-size: 10px; }
  textarea { display: block; box-sizing: border-box; width: 100%; min-height: 170px; margin-top: 10px; padding: 10px; background: var(--bg); color: var(--fg); border: 1px solid var(--line); border-radius: 8px; font-family: var(--mono); font-size: 11px; line-height: 1.5; resize: vertical; }
  .error, .notice { font-size: 12px; overflow-wrap: anywhere; margin: 10px 0; }
  .error { color: var(--danger); } .notice { color: var(--ok, #35cba1); }
  :focus-visible { outline: 2px solid var(--accent, #8877ff); outline-offset: 3px; }
  @media (prefers-reduced-motion: reduce) { .client { transition: none; } }
</style>
