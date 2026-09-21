import type { TabState } from '../store.svelte';
import type { Mode, Pacing } from '../bridge';
import type { Approval, ControlRequest, Proposal, SessionEvent } from './events';
export type SessionSnapshot = {
  shared: boolean; externalOrigin: boolean; surfaceAvailable: boolean; inputPending: boolean;
  externalStarting?: boolean; externalInputAvailable: boolean; processAlive: boolean; attended: boolean;
  profileId: string | null; profileName: string | null; reviewRequired: boolean;
  mode: Mode; effectiveMode: Mode; controlGate: boolean; pacing: Pacing;
  sessionAllows: string[]; affordanceMask: string[] | null; connectedAgents: string[];
  controller: TabState['controller']; attentionRequest: {agentId: string; reason?: string | null} | null; openedBy: string | null;
  lastAgent: TabState['lastAgent'];
  scheduled?: { execId: string; cmd: string; intent?: string } | null; scheduledRemainingMs?: number | null;
  pending: Approval[];
  proposal: Proposal | null;
  controlRequests: ControlRequest[];
};
const approvalState = (a: Approval, at = Date.now()): NonNullable<TabState['approval']> => ({ id: a.id, agentId: a.agentId, cmd: a.cmd, label: a.label, at, intent: a.intent ?? undefined, analysis: a.analysis ?? undefined });
const proposalState = (p: Proposal): NonNullable<TabState['proposal']> => ({ id: p.proposalId, agentId: p.agentId, text: p.text, ready: p.state === 'ready', intent: p.intent ?? undefined });
const controlState = (c: ControlRequest): NonNullable<TabState['ctlReq']> => ({ id: c.requestId, agentId: c.agentId, reason: c.reason ?? undefined, originalRequest: c.originalRequest ?? undefined });
function clearDecisions(t: TabState) {
  t.approval = null; t.ctlReq = null; t.proposal = null; t.grace = null; t.handback = false;
}
export function applyController(t: TabState, controller: TabState['controller']) {
  t.controller = controller;
  if (controller.type === 'agent') t.handback = false;
  else { t.grace = null; t.proposal = null; }
}

/** State changes only. Notifications, focus and motion belong to the view. */
export function applySessionEvent(t: TabState, ev: SessionEvent) {
  if (ev.session !== t.id) return;
  if (ev.event === 'sharing_changed') {
    t.shared = ev.shared; t.surfaceAvailable = false; clearDecisions(t);
    t.timeline.recording = ev.shared; t.agents = []; t.lastAgent = null; t.attention = null; t.typing = false;
    applyController(t, { type: 'human' });
    return;
  }
  if (ev.event === 'process_exited') { t.processAlive = false; clearDecisions(t); applyController(t, { type: 'human' }); }
  if (ev.event === 'attention_changed') { t.attended = ev.attended; if (ev.attended) t.attention = null; }
  if (!t.shared) return;
  switch (ev.event) {
    case 'control_granted': {
      const previous = t.lastAgent;
      t.lastAgent = { agentId: ev.agentId, connected: true, lastCmd: previous?.agentId === ev.agentId ? previous.lastCmd : undefined };
      applyController(t, { type: 'agent', agentId: ev.agentId }); t.ctlReq = null; break;
    }
    case 'control_revoked': applyController(t, { type: 'human' }); break;
    case 'control_requested': t.ctlReq = controlState(ev.request); break;
    case 'control_request_resolved': if (t.ctlReq?.id === ev.requestId) t.ctlReq = null; break;
    case 'attention_requested': t.attention = { agentId: ev.agentId, reason: ev.reason ?? undefined }; break;
    case 'tab_opened': t.openedBy = ev.agentId; t.attention = { agentId: ev.agentId, reason: ev.reason ?? undefined }; break;
    case 'proposal_changed': t.proposal = proposalState(ev.proposal); break;
    case 'proposal_resolved':
      if (t.proposal?.id === ev.proposalId) t.proposal = null;
      if (ev.state === 'executed' && t.lastAgent) t.lastAgent.lastCmd = ev.cmd;
      break;
    case 'approval_requested': t.approval = approvalState(ev.request, t.approval?.id === ev.request.id ? t.approval.at : undefined); break;
    case 'approval_resolved': if (t.approval?.id === ev.approvalId) t.approval = null; break;
    case 'session_allows_changed': t.allows = ev.allows; break;
    case 'exec_scheduled': t.grace = { execId: ev.execId, cmd: ev.cmd, ms: ev.graceMs, start: performance.now(), intent: ev.intent ?? undefined }; break;
    case 'exec_cancelled': if (t.grace?.execId === ev.execId) t.grace = null; break;
    case 'agent_exec': t.grace = null; if (t.lastAgent) t.lastAgent.lastCmd = ev.cmd; break;
    case 'pacing_changed': t.pacing = ev.pacing; break;
    case 'mode_changed': t.mode = ev.mode; t.effectiveMode = ev.effectiveMode; break;
    case 'control_gate_changed': t.gate = ev.ask; break;
    case 'affordance_mask_changed': t.mask = ev.allow; break;
  }
}
export function applySessionSnapshot(t: TabState, s: SessionSnapshot, privateTitle: string) {
    t.shared = s.shared === true;
    t.externalOrigin = s.externalOrigin === true;
    t.surfaceAvailable = s.surfaceAvailable === true;
    t.inputPending = s.inputPending === true;
    t.externalStarting = s.externalStarting === true;
    t.externalInputAvailable = s.externalInputAvailable === true;
    if (!t.shared) {
      t.profileId = s.profileId ?? null; t.profileName = s.profileName ?? null;
      t.title = s.profileName || (t.externalOrigin ? privateTitle : t.title);
      t.processAlive = !!s.processAlive; t.attended = !!s.attended;
      t.timeline.recording = false; t.controller = { type: "human" };
      t.agents = []; t.lastAgent = null; t.openedBy = null;
      clearDecisions(t); t.attention = null;
      t.handback = false; t.typing = false;
      t.statusReady = true;
      return;
    }
    t.timeline.recording = true;
    if (!t.profileId && s.profileName) t.title = s.profileName;
    t.profileId = s.profileId ?? null; t.profileName = s.profileName ?? null; t.reviewRequired = !!s.reviewRequired;
    t.mode = s.mode; t.effectiveMode = s.effectiveMode ?? s.mode; t.gate = !!s.controlGate; t.allows = s.sessionAllows ?? []; t.mask = s.affordanceMask ?? null; t.agents = s.connectedAgents ?? [];
    t.pacing = s.pacing; t.attended = !!s.attended; t.processAlive = !!s.processAlive;
    t.attention = s.attentionRequest ? { agentId: s.attentionRequest.agentId, reason: s.attentionRequest.reason ?? undefined } : null;
    t.openedBy = s.openedBy ?? null;
    t.lastAgent = s.lastAgent ? { agentId: s.lastAgent.agentId, connected: s.lastAgent.connected, lastCmd: s.lastAgent.lastCmd ?? undefined } : null;
    t.controller = s.controller.type === "agent" ? { type: "agent", agentId: s.controller.agentId } : { type: "human" };
    t.approval = s.pending?.length ? approvalState(s.pending[0], t.approval?.id === s.pending[0].id ? t.approval.at : undefined) : null;
    t.proposal = s.proposal ? proposalState(s.proposal) : null;
    t.ctlReq = s.controlRequests?.length ? controlState(s.controlRequests[0]) : null;
    if (!s.scheduled) t.grace = null;
    else if (t.grace?.execId !== s.scheduled.execId) t.grace = { execId: s.scheduled.execId, cmd: s.scheduled.cmd, intent: s.scheduled.intent, ms: Math.max(1, s.scheduledRemainingMs ?? 0), start: performance.now() };
    if (t.controller.type === 'agent') t.handback = false;
    t.statusReady = true;
}
