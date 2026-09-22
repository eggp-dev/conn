<script lang="ts">
  import { onMount } from 'svelte';
  import { ConnApp, HostConnection, translate, initialLanguage } from '@conn/ui';
  import type { ConnPorts } from '@conn/ui/runtime';
  import { WebTransport } from './transport';

  let ports = $state<ConnPorts>();
  let busy = $state(false), error = $state(''), occupied = $state(false), reconnect = $state(false);
  const lang = initialLanguage(localStorage);
  let transport: WebTransport | undefined, disposed = false, established = false;
  function problem(kind: 'authentication' | 'occupied' | 'offline' | 'version' | 'reconnect') {
    if (disposed || kind === 'offline') return;
    ports = undefined; occupied = kind === 'occupied'; reconnect = kind === 'reconnect';
    error = kind === 'authentication' ? (established ? translate(lang, 'host.authentication') : '')
      : kind === 'version' ? translate(lang, 'host.versionMismatch')
      : kind === 'reconnect' ? translate(lang, 'connection.uncertain') : '';
  }
  async function connect(token = '', takeover = false) {
    if (busy || disposed) return;
    busy = true; error = '';
    transport?.dispose();
    try {
      if (token) {
        const response = await fetch('/api/bootstrap', { method: 'POST', headers: { 'Content-Type': 'application/json' }, credentials: 'same-origin', body: JSON.stringify({ token }) });
        if (!response.ok) throw new Error(translate(lang, 'host.invalidCode'));
      }
      const candidate = transport = new WebTransport(problem);
      await candidate.connect(takeover);
      const info = await (await fetch('/api/info', { cache: 'no-store' })).json();
      if (disposed) { candidate.dispose(); return; }
      ports = {
        transport: candidate,
        attention: { isFocused: async () => document.hasFocus(), request: async () => {} },
        updates: {
          supported: false,
          reason: translate(lang, 'host.updateServer'),
          invoke: async () => ({ phase: 'unsupported', current: info.buildVersion, notes: '', preview: false, supported: false, downloaded: 0 }),
        },
        links: { open: async url => { window.open(url, '_blank', 'noopener,noreferrer'); } },
        storage: localStorage,
        platform: /Mac/i.test(navigator.platform) ? 'macos' : /Win/i.test(navigator.platform) ? 'windows' : 'linux',
      };
      established = true;
    } catch (cause: any) {
      if (!disposed && !['authentication', 'owner_attached'].includes(cause.code)) error = String(cause.message ?? cause);
    } finally { if (!disposed) busy = false; }
  }
  onMount(() => {
    const token = new URLSearchParams(location.hash.slice(1)).get('token') ?? '';
    if (token) history.replaceState(null, '', location.pathname + location.search);
    void connect(token);
    return () => { disposed = true; transport?.dispose(); };
  });
</script>

{#if ports}<ConnApp {ports} />{:else}<HostConnection mode={reconnect ? 'local' : 'code'} {lang} {busy} {error} {occupied} onconnect={connect} />{/if}
