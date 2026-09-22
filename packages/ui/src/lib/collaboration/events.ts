import type { Mode, Pacing } from '../bridge';
import type { Analysis } from '../store.svelte';
import type { OriginalRequest } from '../timeline';

export type ReviewContext = { reason: "command_policy" | "unverified_shell"; allowSession: boolean; remote: boolean };
export type Approval = { id: string; agentId: string; cmd: string; label: string; review?: ReviewContext; intent?: string | null; analysis?: Analysis | null };
export type ControlRequest = { requestId: string; agentId: string; reason?: string | null; originalRequest?: OriginalRequest | null };
export type Proposal = { proposalId: string; agentId: string; text: string; state: 'drafting' | 'ready' | 'executed' | 'rejected' | 'denied'; intent?: string | null };
type Resolution = 'pending' | 'granted' | 'denied' | 'expired';

/** Owner event stream, scoped to a session by the native or browser transport. */
export type SessionEvent = { session: string } & (
  | { event: 'control_granted'; lease: string; agentId: string; reason?: string | null; originalRequest?: OriginalRequest | null }
  | { event: 'control_revoked'; lease: string; agentId: string; reason: string }
  | { event: 'control_handed_back'; lease: string; agentId: string; lastCmd: string | null }
  | { event: 'control_requested'; request: ControlRequest }
  | { event: 'control_request_resolved'; requestId: string; state: Resolution }
  | { event: 'approval_requested'; request: Approval }
  | { event: 'approval_resolved'; approvalId: string; state: Resolution; by: string }
  | { event: 'agent_input'; agentId: string; len: number }
  | { event: 'exec_scheduled'; execId: string; agentId: string; cmd: string; intent?: string | null; graceMs: number }
  | { event: 'exec_cosigned'; execId: string; agentId: string }
  | { event: 'exec_cancelled'; execId: string; reason: string }
  | { event: 'agent_exec'; agentId: string; cmd: string; policy: string; intent?: string | null; submissionId?: string }
  | { event: 'proposal_changed'; proposal: Proposal }
  | { event: 'proposal_resolved'; proposalId: string; state: Proposal['state']; cmd: string; policy: string | null }
  | { event: 'sharing_changed'; shared: boolean; generation: number }
  | { event: 'screen_changed'; revision: number }
  | { event: 'process_exited'; exitCode: number | null }
  | { event: 'pacing_changed'; pacing: Pacing }
  | { event: 'affordance_mask_changed'; allow: string[] | null }
  | { event: 'mode_changed'; mode: Mode; effectiveMode: Mode }
  | { event: 'control_gate_changed'; ask: boolean }
  | { event: 'attention_changed'; attended: boolean }
  | { event: 'attention_requested' | 'tab_opened'; agentId: string; reason?: string | null }
  | { event: 'agent_switched_tab'; agentId: string; from: string; to: string }
  | { event: 'session_allows_changed'; allows: string[] }
  | { event: 'shell_integration_changed'; status: { state: string; shell?: string | null; reason?: string | null } }
  | { event: 'shell_command_started'; commandId: string; submissionId: string | null; actor: string; cmd: string; cwd: string }
  | { event: 'shell_command_finished'; commandId: string; exitCode: number | null; durationMs: number }
  | { event: 'tools_changed' }
);
