<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import { groupConnections, type AgentConnection } from "../lib/connections";
  let { connections, compact = false }: { connections: AgentConnection[]; compact?: boolean } = $props();
  const groups = $derived(groupConnections(connections));
</script>

<div class="connections">
  {#each groups as group (group.agentId)}
    <details>
      <summary><span class="chevron" aria-hidden="true">›</span><b>{group.agentId}</b><span>{t(group.connections.length === 1 ? "s.diag.connection" : "s.diag.connections", { n: group.connections.length })}</span></summary>
      <ul>
        {#each group.connections as connection (connection.connId)}
          <li><span>{t("s.diag.connection.id", { id: connection.connId })} · {t("s.diag.idle", { seconds: connection.idleSecs })}</span><small>{t("s.diag.connection.tabs", { tabs: connection.sessions.join(", ") })}</small></li>
        {/each}
      </ul>
    </details>
  {:else}<span class="muted">{t("center.no_agents")}</span>{/each}
  {#if !compact}<p>{t("s.diag.connections.note")}</p>{/if}
</div>

<style>
  .connections { min-width: 0; }
  details { margin: 4px 0 8px; }
  summary { display: flex; flex-wrap: wrap; gap: 6px 12px; cursor: pointer; overflow-wrap: anywhere; }
  summary span, small, p { color: var(--muted); }
  .chevron { display: inline-block; transition: transform 120ms; }
  details[open] .chevron { transform: rotate(90deg); }
  @media (prefers-reduced-motion: reduce) { .chevron { transition: none; } }
  summary:focus-visible { outline: 2px solid var(--agent); outline-offset: 3px; }
  ul { padding-left: 14px; margin: 8px 0; }
  li { margin: 8px 0; overflow-wrap: anywhere; }
  small { display: block; margin-top: 4px; }
  p { font-size: 11px; line-height: 1.6; margin: 8px 0 0; }
</style>
