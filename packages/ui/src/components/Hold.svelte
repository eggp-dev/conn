<script lang="ts">
  import { useConnApp } from '../runtime/context';
  const app = useConnApp();
  const { t, reviewAction, resolveReview, cur, st } = app;

  import DecisionCard from "./DecisionCard.svelte";
  const action = $derived(reviewAction(st.active, cur().approval?.id ?? "none"));
  function decide(value: string) { return resolveReview(st.active, a.id, value); }
  const a = $derived(cur().approval!);
  const targets = $derived(a.analysis?.segments.flatMap((s) => s.targets) ?? []);
  const multi = $derived((a.analysis?.segments.length ?? 0) > 1);
</script>


{#if cur().approval}
  <DecisionCard requestKey={`${st.active}:review:${a.id}`} agent={a.agentId} label={a.label} busy={action.busy} error={action.error}>
    <p class="reason">{a.intent || t("hold.no_intent")}</p>
    {#if targets.length}
      <ul class="targets">
        {#each targets as tg}
          <li class:protected={tg.protected} class:missing={!tg.exists}>
            <span class="k">{t("hold.target")}</span><code>{tg.path}</code>
            {#if !tg.exists}<span class="tag">{t("hold.missing")}</span>{/if}
            {#if tg.gitRepo}<span class="tag git">{t("hold.git")}</span>{/if}
            {#if tg.entries != null}<span class="tag">{t("hold.entries", { n: tg.entries >= 2000 ? "2000+" : tg.entries })}</span>{/if}
            {#if tg.protected}<span class="tag danger">{t("hold.protected")}</span>{/if}
          </li>
        {/each}
      </ul>
    {/if}
    {#if multi}
      <ul class="segs">
        {#each a.analysis!.segments as s}
          <li class={s.policy}><span class="dot"></span><code>{s.text}</code>{#if s.label}<span class="tag">{s.label}</span>{/if}</li>
        {/each}
      </ul>
    {:else}
      <code class="raw">{a.cmd}</code>
    {/if}
    {#snippet actions()}
      {#if a.analysis?.cwd}<span class="muted cwd">cwd {a.analysis.cwd}</span>{/if}
      <span class="spacer"></span>
      <button class="btn ok" disabled={action.busy} onclick={() => decide("grant")}><kbd>a</kbd> {t("hold.approve")}</button>
      <button class="btn danger" disabled={action.busy} onclick={() => decide("deny")}><kbd>d</kbd> {t("hold.deny")}</button>
      {#if cur().reviewRequired}
        <span class="muted review-note">{t('hold.review_required')}</span>
      {:else}
        <button class="btn" disabled={action.busy} title={t('hold.allow_session_hint',{label:a.label})} onclick={() => decide("allow_session")}><kbd>A</kbd> {t("hold.allow_session")}</button>
      {/if}
    {/snippet}
  </DecisionCard>
{/if}

<style>
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 3px; }
  .targets li { min-width:0; flex-wrap:wrap; display: flex; gap: 8px; align-items: center; }
  .targets .k { color: var(--muted); font-size: 11px; width: 28px; }
  .targets code { overflow-wrap:anywhere; min-width:0; font-size: 12.5px; }
  .targets li.protected code { color: var(--danger); }
  .tag { font-size: 10.5px; padding: 1px 6px; border-radius: 999px; background: var(--surface2); color: var(--muted); white-space: nowrap; }
  .tag.git { color: var(--agent); }
  .tag.danger { background: color-mix(in srgb, var(--danger) 20%, transparent); color: var(--danger); }
  .segs li { flex-wrap:wrap; display: flex; gap: 8px; align-items: center; }
  .segs .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--muted); }
  .segs li.confirm .dot { background: var(--warn); } .segs li.deny .dot { background: var(--danger); }
  .segs li.allow code { color: var(--muted); }
  .raw { color: var(--muted); white-space: pre-wrap; word-break: break-all; }
  .spacer { flex: 1; }
  .cwd { font-size: 11px; }
  .review-note { flex-basis:100%; text-align:right; font-size:11px; }
</style>
