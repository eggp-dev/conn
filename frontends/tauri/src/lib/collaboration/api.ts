import { invoke } from '../transport';
import type { AdmissionSnapshot, SharingSnapshot, SharingStatus, DecisionTarget } from './contracts';
export const admissionSnapshot = () => invoke<AdmissionSnapshot>('admission_snapshot');
export const decideAdmission = (connId: number, allow: boolean) => invoke('decide_admission', { connId, allow });
export const sharingSnapshot = (session: string) => invoke<SharingSnapshot>('sharing_state', { session });
export const setSharing = (session: string, shared: boolean, connectionIds: number[], expectedRevision: number) => invoke<SharingStatus>('set_sharing', { session, shared, connectionIds, expectedRevision });
export const decideControl = (target: DecisionTarget, grant: boolean) => invoke('decide_control', { ...target, grant });
export function failureKey(error: unknown): string {
  const message = String(error);
  if (message.includes('sharing_changed')) return 'sharing.changed';
  if (message.includes('history_unavailable')) return 'sharing.historyUnavailable';
  if (message.includes('input_pending') || message.includes('shell input is pending')) return 'sharing.pendingInput';
  if (message.includes('connection_gone') || message.includes('disconnected')) return 'sharing.connectionGone';
  return 'decision.failed';
}
