<script lang="ts">
  import { onMount } from 'svelte';
  import { ConnApp, HostConnection, translate, initialLanguage } from '@conn/ui';
  import type { ConnPorts } from '@conn/ui/runtime';
  import { createNativePorts } from './nativePorts';

  let ports = $state<ConnPorts>();
  let busy = $state(true), error = $state('');
  const lang = initialLanguage(localStorage);
  let disposed = false, connecting = false;
  function disconnected(cause: unknown, uncertain = false) {
    if (disposed) return;
    ports?.transport.dispose?.(); ports = undefined;
    error = uncertain ? translate(lang, 'connection.uncertain') : String(cause) === 'attachment_fenced'
      ? translate(lang, 'host.connectionChanged')
      : String(cause instanceof Error ? cause.message : cause);
  }
  async function connect() {
    if (connecting || disposed) return;
    connecting = true; busy = true; error = '';
    try {
      const next = await createNativePorts(disconnected);
      if (disposed) { next.transport.dispose?.(); return; }
      ports = next;
    } catch (cause) { disconnected(cause); }
    finally { connecting = false; if (!disposed) busy = false; }
  }
  onMount(() => {
    void connect();
    return () => { disposed = true; ports?.transport.dispose?.(); };
  });
</script>

{#if ports}<ConnApp {ports} />{:else}<HostConnection mode="local" {lang} {busy} {error} onconnect={connect} />{/if}
