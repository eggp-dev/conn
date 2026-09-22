import type { ConnClient } from '../../runtime/client';
import type { createRequestActions } from './decision.svelte';
export function createReview(client: ConnClient, actions: ReturnType<typeof createRequestActions>) {
  const {invoke} = client; const {requestAction} = actions;
  const reviewAction = (session: string, approvalId: string) => requestAction(`approval:${session}:${approvalId}`);
  function resolveReview(session: string, approvalId: string, decision: string) {
    return reviewAction(session, approvalId).run(() => invoke('approve', { session, approvalId, decision }));
  }


  return { reviewAction, resolveReview };
}
