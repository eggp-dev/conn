/** Projection of the active tab; never grants control or runs a command. */
export type ConnMarkState = 'human' | 'request' | 'agent' | 'grace' | 'blocked' | 'paused' | 'offline';
export type MarkInput = {
  processAlive: boolean; policyBlockedUntil: number; paused: boolean;
  controller: { type: 'human' | 'agent' }; ctlReq: unknown; approval: unknown;
  attention: unknown; proposal: { ready: boolean } | null;
  grace: { start: number; ms: number } | null;
};
export function connMarkState(tab: MarkInput, online: boolean, now: number): ConnMarkState {
  if (!online || !tab.processAlive) return 'offline';
  if (tab.policyBlockedUntil > now) return 'blocked';
  if (tab.ctlReq || tab.approval || tab.proposal?.ready || tab.attention) return 'request';
  if (tab.paused) return 'paused';
  if (tab.grace) return 'grace';
  return tab.controller.type === 'agent' ? 'agent' : 'human';
}
export function graceProgress(grace: MarkInput['grace'], now: number): number {
  return grace ? grace.ms <= 0 ? 1 : Math.min(1, Math.max(0, (now - grace.start) / grace.ms)) : 0;
}
