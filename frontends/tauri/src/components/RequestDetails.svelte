<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import type { OriginalRequest } from "../lib/timeline";
  let { request }: { request?: OriginalRequest } = $props();
  const command = $derived(typeof request?.params?.command === "string" ? request.params.command : undefined);
</script>

<details class="request-original">
  <summary>{t("request.original")}</summary>
  {#if request}
    <p>{t("request.planned")}</p>
    {#if command !== undefined}<pre><code>{command}</code></pre>{:else}<p class="muted">{t("request.noCommand")}</p>{/if}
    <p>{t("request.parameters")}</p>
    <pre><code>{JSON.stringify(request, null, 2)}</code></pre>
  {:else}<p class="muted">{t("request.unavailable")}</p>{/if}
</details>

<style>
  .request-original { min-width: 0; border: 1px solid var(--line); border-radius: 8px; margin: 6px 0; padding: 10px 12px; }
  summary { cursor: pointer; color: var(--fg); font-size: 12px; }
  summary:focus-visible { outline: 2px solid var(--agent); outline-offset: 3px; }
  p { font-size: 11px; color: var(--muted); margin: 10px 0 6px; }
  pre { margin: 0; padding: 10px; background: var(--bg); border-radius: 6px; white-space: pre-wrap; overflow-wrap: anywhere; max-height: 22vh; overflow: auto; font-size: 12px; line-height: 1.55; }
</style>
