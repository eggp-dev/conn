// Contract fixture for the production App. Never imported by the application entrypoint.
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { emit } from '@tauri-apps/api/event';
import { st } from '../../src/lib/store.svelte';
import { mount } from 'svelte';
import App from '../../src/App.svelte';
import '../../src/app.css';
import { setLang } from '../../src/lib/i18n.svelte';

const options = new URLSearchParams(location.search);
setLang(options.get('lang') === 'ko' ? 'ko' : 'en');
if (options.has('settings')) { st.settingsOpen = true; st.settingsTab = 'agents'; }
let shared = false, admitted = false, admissionFailures = options.has('fail-admission') ? 1 : 0;
let revision = 1;
let staleFailures = options.has('fail-stale') ? 1 : 0;
let waiting = !options.has('delayed');
window.addEventListener('fixture:request', () => { waiting = true; revision++; void emit('ss:admission', {connId: 7, agentId: 'copilot', state: 'pending', revision}); });
const profile = { id: 'fixture', name: 'External shell', enabled: true, backend: 'local', shell: 'posix', program: '/bin/bash', args: [], cwd: null, env: {}, target: null, port: null };
const status = () => ({ shared, externalOrigin: true, externalStarting: options.has('preparing'), externalInputAvailable: !shared,
  surfaceAvailable: shared, inputPending: false, processAlive: true, attended: true, profileId: profile.id, profileName: profile.name,
  mode: 'autopilot', effectiveMode: 'autopilot', controller: {type: 'human'}, controlGate: true, pending: [], controlRequests: [],
  sessionAllows: [], affordanceMask: null, connectedAgents: admitted ? ['copilot'] : [], reviewRequired: true,
  pacing: {minWriteIntervalMs: 0, enterGraceMs: 0, leaseTtlSecs: 60, approvalTtlSecs: 300}, revision });
mockWindows('main');
mockIPC((command, raw: any) => {
  if (command !== 'dispatch') return null;
  const { name, args } = raw;
  switch (name) {
    case 'profiles_catalog': return {config: {version: 1, revision: 0, defaultProfile: profile.id, profiles: [profile]}, availability: {[profile.id]: {available: true, message: 'Ready'}}};
    case 'extensions_status': return {extensions: []};
    case 'start': return {socket: 'fixture', session: options.has('empty') ? null : 'private-1', sessions: options.has('empty') ? [] : ['private-1'], externalPending: options.has('preparing')};
    case 'status': return status();
    case 'pending_admissions': return admitted || !waiting ? [] : [{connId: 7, agentId: 'copilot'}];
    case 'admission_snapshot': return {revision, pending: admitted || !waiting ? [] : [{connId: 7, agentId: 'copilot'}]};
    case 'admission_policy': return {ask: true};
    case 'decide_admission':
      if (admissionFailures-- > 0) throw new Error('transport_unavailable');
      admitted = true; revision++;
      void emit('ss:admission', {connId: 7, agentId: 'copilot', state: args.allow ? 'granted' : 'denied', revision});
      return {decided: true};
    case 'sharing_participants': return admitted ? [{connId: 7, agentId: 'copilot', selected: shared}] : [];
    case 'sharing_state': return {revision, participants: admitted ? [{connId: 7, agentId: 'copilot', selected: shared}] : []};
    case 'set_sharing':
      if (staleFailures-- > 0) { revision++; throw new Error('sharing_changed'); }
      if (options.has('fail-sharing')) throw new Error('input_pending');
      shared = args.shared; revision++;
      void emit('ss:event', {session: 'private-1', event: 'sharing_changed', shared, generation: revision});
      return status();
    case 'agent_clients': return [];
    case 'audit_tail': return [];
    case 'log': return null;
    default: return null;
  }
}, {shouldMockEvents: true});
mount(App, {target: document.getElementById('app')!});
