import type { TabState } from '../store.svelte';
export type ActivityConnection = { connId: number; agentId: string; session: string | null; preparation: {id:number;reason:string|null} | null };
export type ActivitySnapshot = { revision: number; connections: ActivityConnection[] };
export type NoticeTarget = { session: string; requestId?: string; kind?: 'control' | 'review' | 'proposal' } | { connId: number; preparationId?: number };
export function pendingKind(t: TabState) {
  if (!t.shared || !t.processAlive) return null;
  return t.approval ? 'review' : t.ctlReq ? 'control' : t.proposal?.ready ? 'proposal' : t.attention ? 'attention' : null;
}
export function requestStillPending(t: TabState, target: Extract<NoticeTarget,{session:string}>) {
  if (!target.requestId) return true;
  return (target.kind === 'control' ? t.ctlReq?.id : target.kind === 'review' ? t.approval?.id : t.proposal?.id) === target.requestId;
}
export function sessionLabel(order: string[], tabs: Record<string, TabState>, id: string) {
  const index=order.indexOf(id);return index < 0 ? '' : `${index+1} · ${tabs[id]?.title ?? ''}`;
}
