import type { ConnClient } from '../../runtime/client';
import type { createCollaborationApi } from './api';
import type { createRequestActions } from './decision.svelte';
import type { DecisionTarget } from './contracts';

export function createControl(client: ConnClient, api: ReturnType<typeof createCollaborationApi>, actions: ReturnType<typeof createRequestActions>) {
  const {invoke} = client; const {decideControl} = api; const {requestAction} = actions;
  const controlAction = (target: DecisionTarget) => requestAction(`control:${target.session}:${target.requestId}`);
  function resolveControl(target: DecisionTarget, decision: 'grant' | 'copilot' | 'deny') {
    return controlAction(target).run(async () => {
      if (decision === 'copilot') await invoke('set_mode', { session: target.session, mode: 'copilot' });
      await decideControl(target, decision !== 'deny');
    });
  }


  return { controlAction, resolveControl };
}
