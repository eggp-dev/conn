import type { ConnClient } from '../../runtime/client';
import type { createRequestActions } from './decision.svelte';
export function createReview(client: ConnClient, actions: ReturnType<typeof createRequestActions>, onRetained: () => void = () => {}) {
  const {invoke} = client; const {requestAction} = actions;
  const reviewAction = (session: string, approvalId: string) => requestAction(`approval:${session}:${approvalId}`);
  function resolveReview(session: string, approvalId: string, decision: string) {
    return reviewAction(session, approvalId).run(async () => { const result = await invoke('approve', { session, approvalId, decision }); if (result?.inputRetained) onRetained(); });
  }


  return { reviewAction, resolveReview };
}
