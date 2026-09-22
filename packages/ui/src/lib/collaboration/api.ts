import type { ConnClient } from '../../runtime/client';
import type { AdmissionSnapshot, SharingSnapshot, SharingStatus, DecisionTarget } from './contracts';
export function failureKey(error: unknown): string {
  const message = String(error);
  const code = typeof error === 'object' && error !== null && 'code' in error ? String(error.code) : '';
  if (['outcome_unknown', 'outcome_pending'].includes(code) || /outcome_(unknown|pending)/.test(message)) return 'decision.outcomeUnknown';
  if (message.includes('review_context_changed')) return 'decision.contextChanged';
  if (message.includes('review_scope_unavailable')) return 'decision.scopeUnavailable';
  if (message.includes('sharing_changed')) return 'sharing.changed';
  if (message.includes('history_unavailable')) return 'sharing.historyUnavailable';
  if (message.includes('input_pending') || message.includes('shell input is pending')) return 'sharing.pendingInput';
  if (message.includes('connection_gone') || message.includes('disconnected')) return 'sharing.connectionGone';
  return 'decision.failed';
}

export function createCollaborationApi(client: ConnClient) {
  const {invoke} = client;
  const admissionSnapshot = () => invoke<AdmissionSnapshot>('admission_snapshot');
  const decideAdmission = (connId: number, allow: boolean) => invoke('decide_admission', { connId, allow });
  const sharingSnapshot = (session: string) => invoke<SharingSnapshot>('sharing_state', { session });
  const setSharing = (session: string, shared: boolean, connectionIds: number[], expectedRevision: number) => invoke<SharingStatus>('set_sharing', { session, shared, connectionIds, expectedRevision });
  const decideControl = (target: DecisionTarget, grant: boolean) => invoke('decide_control', { ...target, grant });

  return { admissionSnapshot, decideAdmission, sharingSnapshot, setSharing, decideControl };
}
