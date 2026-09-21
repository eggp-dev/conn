import { invoke } from '../transport';
import { decideControl } from './api';
import type { DecisionTarget } from './contracts';
import { requestAction } from './decision.svelte';

export const controlAction = (target: DecisionTarget) => requestAction(`control:${target.session}:${target.requestId}`);
export function resolveControl(target: DecisionTarget, decision: 'grant' | 'copilot' | 'deny') {
  return controlAction(target).run(async () => {
    if (decision === 'copilot') await invoke('set_mode', { session: target.session, mode: 'copilot' });
    await decideControl(target, decision !== 'deny');
  });
}
