<script lang="ts">
  import { translate, type Lang } from '../lib/i18n.svelte';
  let { lang = 'en', busy = false, error = '', occupied = false, mode = 'code', onconnect }: {
    lang?: Lang; busy?: boolean; error?: string; occupied?: boolean;
    mode?: 'code' | 'local';
    onconnect: (token: string, takeover: boolean) => void;
  } = $props();
  let token = $state('');
  const t = (key: string) => translate(lang, key);
</script>

<main class="host-connection">
  <form onsubmit={event => { event.preventDefault(); onconnect(token, occupied); }}>
    <div class="wordmark">Conn</div>
    <h1>{t(occupied ? 'host.occupied' : 'host.connect')}</h1>
    <p>{t(occupied ? 'host.occupiedHint' : mode === 'local' ? 'host.localHint' : 'host.codeHint')}</p>
    {#if !occupied && mode === 'code'}
      <label for="connection-token">{t('host.code')}</label>
      <input id="connection-token" type="password" bind:value={token} autocomplete="off" required disabled={busy} />
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <button class="btn primary" type="submit" disabled={busy}>{t(busy ? 'host.connecting' : occupied ? 'host.continue' : mode === 'local' ? 'host.reconnect' : 'host.connectAction')}</button>
  </form>
</main>

<style>
  .host-connection { min-height: 100dvh; display: grid; place-items: center; padding: 28px; background: var(--bg, #0c0e13); color: var(--text, #edf0f6); }
  form { width: min(100%, 400px); display: flex; flex-direction: column; gap: 14px; }
  .wordmark { color: var(--agent, #59d3df); font-size: 16px; font-weight: 650; }
  h1 { margin: 0; font-size: 23px; letter-spacing: -.02em; }
  p { margin: 0; line-height: 1.6; color: var(--muted, #a0a8b7); }
  label { margin-top: 12px; font-size: 13px; }
  input { width: 100%; box-sizing: border-box; border: 1px solid var(--line, #303641); border-radius: 9px; padding: 12px; background: var(--surface, #151922); color: inherit; font: inherit; }
  input:focus-visible { outline: 2px solid var(--agent, #59d3df); outline-offset: 2px; }
  button { margin-top: 6px; padding: 12px; }
  .error { color: var(--danger, #ff8a91); overflow-wrap: anywhere; }
</style>
