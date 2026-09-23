import { createLifetime } from './lifetime';
import { createClient } from './client';
import type { ConnPorts } from './ports';
import { createStore } from '../lib/store.svelte';
import { createI18n } from '../lib/i18n.svelte';
import { createBridge } from '../lib/bridge';
import { createThemes, applyTheme } from '../lib/themes';
import { createProfiles } from '../lib/profiles.svelte';
import { createExtensions } from '../lib/extensions.svelte';
import { createLiveAgents } from '../lib/liveAgents.svelte';
import { createUpdates } from '../lib/updates.svelte';
import { createAttention } from '../lib/collaboration/attention';
import { createRequestActions } from '../lib/collaboration/decision.svelte';
import { createCollaborationApi } from '../lib/collaboration/api';
import { createControl } from '../lib/collaboration/control.svelte';
import { createReview } from '../lib/collaboration/review.svelte';

/** One invocation per ConnApp mount. No state, request maps or timers survive disposal. */
export function createConnApp(ports: ConnPorts) {
  const lifetime = createLifetime();
  const storage = ports.storage ?? localStorage;
  const client = createClient(ports.transport, lifetime);
  const store = createStore(storage, lifetime);
  const language = createI18n(storage);
  const bridge = createBridge(client, store, language);
  const THEMES = $state(createThemes());
  const requests = createRequestActions();
  const api = createCollaborationApi(client);
  let root: HTMLElement | undefined;
  lifetime.own(requests.dispose);
  return {
    ...client, ...store, ...language, ...bridge, ...api,
    ...createProfiles(client), ...createExtensions(bridge, store, THEMES, storage),
    ...createLiveAgents(bridge, () => store.st.active, lifetime), ...createUpdates(ports.updates, storage, lifetime),
    ...createAttention(ports.attention, lifetime), ...createControl(client, api, requests), ...createReview(client, requests, () => store.toast(language.t('hold.input_retained'), 'info')),
    requestAction: requests.requestAction,
    forgetSession: requests.forgetSession,
    storage, THEMES, ports,
    openExternal: ports.links?.open ?? ((url: string) => client.invoke<void>('open_release', {url})),
    copyText: ports.clipboard?.writeText ?? ((text: string) => navigator.clipboard.writeText(text)),
    setRoot(element: HTMLElement) { root = element; },
    get root() { return root; },
    applyTheme(theme: Parameters<typeof applyTheme>[0], accent?: string, element = root!) { return applyTheme(theme, accent, element); },
    timeout: lifetime.timeout,
    clearTimeout: lifetime.clearTimeout,
    dispose: lifetime.dispose,
  };
}
export type ConnAppState = ReturnType<typeof createConnApp>;
