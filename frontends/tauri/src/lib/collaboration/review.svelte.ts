import { invoke } from '../transport';
import { requestAction } from './decision.svelte';
export const reviewAction = (session: string, approvalId: string) => requestAction(`approval:${session}:${approvalId}`);
export function resolveReview(session: string, approvalId: string, decision: string) {
  return reviewAction(session, approvalId).run(() => invoke('approve', { session, approvalId, decision }));
}
