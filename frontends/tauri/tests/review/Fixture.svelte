<script lang="ts">
  import Timeline from '../../src/components/Timeline.svelte';
  import SettingsSheet from '../../src/components/SettingsSheet.svelte';
  import Toasts from '../../src/components/Toasts.svelte';
  import { st } from '../../src/lib/store.svelte';
  import { setLang } from '../../src/lib/i18n.svelte';
  import { changeMode } from '../../src/lib/bridge';
</script>
<main>
  <nav aria-label="Test fixture">
    <button onclick={() => { st.settingsOpen = false; st.timelineOpen = true; }}>Timeline fixture</button>
    <button onclick={() => { st.settingsTab = 'diagnostics'; st.settingsOpen = true; st.timelineOpen = false; }}>Diagnostics fixture</button>
    <button onclick={() => { st.settingsTab = 'automation'; st.settingsOpen = true; st.timelineOpen = false; }}>Automation fixture</button>
    <button onclick={() => setLang('en')}>English</button>
    <button onclick={() => setLang('ko')}>한국어</button>
    <button onclick={() => changeMode('copilot')}>Blocked mode switch</button>
  </nav>
  <Timeline />
  <Toasts />
  {#if st.settingsOpen}<SettingsSheet onclose={() => st.settingsOpen = false} />{/if}
</main>
<style>
  main { position: relative; width: 100vw; height: 100vh; overflow: hidden; }
  nav { padding: 16px; display: flex; flex-wrap: wrap; gap: 8px; max-width: 420px; }
  button { color: var(--fg); background: var(--surface2); border: 1px solid var(--line); border-radius: 6px; padding: 6px 10px; cursor: pointer; }
</style>
