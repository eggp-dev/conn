/** Shared UI projection of backend events. The native event stream remains authoritative. */
export type OriginalRequest = { method: string; params: Record<string, unknown> };
export type Outcome = "pending" | "granted" | "scheduled" | "executed" | "denied" | "expired" | "cancelled" | "rejected" | "returned" | "info" | "running" | "completed" | "failed" | "unknown";
export type TimelineStep = { t: number; type: string; text?: string; ms?: number };
export type TimelineItem = {
  id: string; t: number; actor: string; kind: "exec" | "control" | "attention" | "tab";
  status: Outcome; text: string; intent?: string; policy?: string; steps: TimelineStep[];
  commandId?: string; submissionId?: string; cwd?: string; exitCode?: number | null; durationMs?: number;
  originalRequest?: OriginalRequest;
  controlId?: string; requestId?: string; lease?: string; commands?: string[]; saved?: boolean;
};
export type TimelineState = { externalPrivate?: boolean; items: TimelineItem[]; seq: number; activeControl?: string; waiting: Record<string, string>; awaitingResult: Record<string, string>; refs: Record<string, string> };
export const newTimeline = (externalPrivate = false): TimelineState => ({ externalPrivate, items: [], seq: 0, waiting: {}, awaitingResult: {}, refs: {} });
export type TimelineFilter = "all" | "commands" | "collaboration";
export const isCommand = (it: TimelineItem) => it.kind === "exec";
export const isPolicyBlocked = (it: TimelineItem) => it.policy === "deny" || it.policy?.startsWith("deny:") === true;
export const policyBlockLabel = (it: TimelineItem) => isPolicyBlocked(it) ? it.policy?.slice(5) ?? "" : "";
export const isProblem = (it: TimelineItem) => ["denied", "expired", "cancelled", "rejected", "failed"].includes(it.status);
const find = (s: TimelineState, id?: string) => s.items.find(it => it.id === id);
const step = (it: TimelineItem, t: number, type: string, text?: string, ms?: number) => { it.steps.push({ t, type, text, ms }); };
function add(s: TimelineState, actor: string, kind: TimelineItem["kind"], text: string, t: number): TimelineItem {
  const it: TimelineItem = { id: `activity-${++s.seq}`, t, actor, kind, text, status: "pending", steps: [] };
  s.items.push(it);
  // Keep links valid while bounding the in-memory history.
  if (s.items.length > 400) {
    const removed = s.items.shift()!;
    for (const map of [s.refs, s.waiting, s.awaitingResult]) for (const key of Object.keys(map)) if (map[key] === removed.id) delete map[key];
    if (s.activeControl === removed.id) s.activeControl = undefined;
    for (const item of s.items) {
      if (item.controlId === removed.id) {
        item.steps.unshift(...removed.steps.map(detail => ({ ...detail })));
        item.originalRequest ??= removed.originalRequest;
        item.controlId = undefined;
      }
      item.commands = item.commands?.filter(id => id !== removed.id);
    }
  }
  return it;
}
function command(s: TimelineState, actor: string, text: string, t: number, intent?: string): TimelineItem {
  let it = find(s, s.waiting[actor]);
  if (!it || it.text !== text) {
    it = add(s, actor, "exec", text, t);
    const control = find(s, s.activeControl);
    if (control?.actor === actor) { it.controlId = control.id; (control.commands ??= []).push(it.id); }
  }
  if (intent) it.intent = intent;
  s.waiting[actor] = it.id;
  return it;
}
function finish(s: TimelineState, it: TimelineItem, outcome: Outcome, t: number, detail?: string) {
  it.status = outcome;
  step(it, t, outcome, detail);
  if (s.waiting[it.actor] === it.id) delete s.waiting[it.actor];
}
function policyOutcome(policy: string): Outcome {
  if (policy.startsWith("deny") || policy.includes(":denied")) return "denied";
  if (policy.includes(":expired")) return "expired";
  return "executed";
}
export function visibleTimeline(s: TimelineState, filter: TimelineFilter): TimelineItem[] {
  return s.items.filter(it => filter === "commands" ? isCommand(it) : filter === "collaboration" ? !isCommand(it) : !(it.kind === "control" && it.commands?.length))
    .slice().sort((a, b) => a.t - b.t);
}
export function timelineSteps(s: TimelineState, it: TimelineItem): TimelineStep[] {
  const control = find(s, it.controlId);
  return [...(control?.steps ?? []), ...it.steps].sort((a, b) => a.t - b.t);
}
export function controlFor(s: TimelineState, it: TimelineItem) { return find(s, it.controlId); }
export function isExternalAutomation(s: TimelineState, it: TimelineItem): boolean {
  return it.actor.startsWith("AppleScript · ") || (it.originalRequest ?? controlFor(s, it)?.originalRequest)?.params.origin === "external_automation";
}

/** Human acts on this command only. A control grant authorizes input, not execution. */
export function commandDecisions(it: TimelineItem): ("approved" | "accepted" | "cosigned")[] {
  if (!isCommand(it)) return [];
  const types = new Set(it.steps.map(s => s.type));
  return ([ ["approval_granted", "approved"], ["proposal_committed", "accepted"], ["exec_cosigned", "cosigned"] ] as const)
    .filter(([type]) => types.has(type)).map(([, label]) => label);
}

/** Correlate by backend request/approval/proposal/execution IDs, never by rendered labels. */
export function recordTimeline(s: TimelineState, ev: Record<string, any>, t = Date.now()) {
  if (s.externalPrivate || ev.externalPrivate) return;
  switch (ev.event) {
    case "control_requested": {
      const r = ev.request;
      if (s.refs[`control:${r.requestId}`]) return;
      const it = add(s, r.agentId, "control", r.reason ?? "", t);
      it.originalRequest = r.originalRequest;
      it.requestId = r.requestId; s.refs[`control:${r.requestId}`] = it.id;
      step(it, t, "control_requested", r.reason); break;
    }
    case "control_granted": {
      let it = [...s.items].reverse().find(it => it.kind === "control" && it.actor === ev.agentId && it.status === "pending");
      if (!it) it = add(s, ev.agentId, "control", ev.reason ?? "", t);
      it.originalRequest ??= ev.originalRequest;
      it.status = "granted"; it.lease = ev.lease; s.activeControl = it.id;
      step(it, t, "control_granted", ev.reason); break;
    }
    case "control_request_resolved": {
      const it = find(s, s.refs[`control:${ev.requestId}`]);
      if (!it) break;
      if (ev.state !== "granted") finish(s, it, ev.state, t);
      else if (it.status === "pending") { it.status = "granted"; step(it, t, "control_granted"); }
      break;
    }
    case "control_revoked": {
      let it = [...s.items].reverse().find(it => it.lease === ev.lease);
      if (!it) it = add(s, ev.agentId, "control", "", t);
      it.status = "returned"; step(it, t, "control_returned", ev.reason);
      if (s.activeControl === it.id) s.activeControl = undefined;
      break;
    }
    case "approval_requested": {
      const r = ev.request;
      const it = command(s, r.agentId, r.cmd, t, r.intent);
      s.refs[`approval:${r.id}`] = it.id;
      it.status = "pending"; step(it, t, "approval_requested", r.label); break;
    }
    case "approval_resolved": {
      const it = find(s, s.refs[`approval:${ev.approvalId}`]);
      if (!it) break;
      step(it, t, `approval_${ev.state}`, ev.by);
      it.status = ev.state; s.awaitingResult[it.actor] = it.id;
      break;
    }
    case "proposal_changed": {
      const p = ev.proposal;
      // Streaming keystrokes are not timeline entries; record when ready for review.
      if (p.state !== "ready") break;
      const it = find(s, s.refs[`proposal:${p.proposalId}`]) ?? command(s, p.agentId, p.text, t, p.intent);
      if (!s.refs[`proposal:${p.proposalId}`]) step(it, t, "proposal_ready");
      s.refs[`proposal:${p.proposalId}`] = it.id; it.text = p.text; it.intent = p.intent; break;
    }
    case "proposal_resolved": {
      const it = find(s, s.refs[`proposal:${ev.proposalId}`]);
      if (!it) break;
      if (ev.state === "rejected") finish(s, it, "rejected", t);
      else { step(it, t, "proposal_committed"); s.awaitingResult[it.actor] = it.id; }
      break;
    }
    case "exec_scheduled": {
      const it = command(s, ev.agentId, ev.cmd, t, ev.intent);
      s.refs[`exec:${ev.execId}`] = it.id; it.status = "scheduled";
      step(it, t, "scheduled", undefined, ev.graceMs); break;
    }
    case "exec_cancelled": {
      const it = find(s, s.refs[`exec:${ev.execId}`]);
      if (it) finish(s, it, "cancelled", t, ev.reason);
      break;
    }
    case "exec_cosigned": {
      const it = find(s, s.refs[`exec:${ev.execId}`]);
      if (it && !it.steps.some(s => s.type === "exec_cosigned")) step(it, t, "exec_cosigned");
      break;
    }
    case "agent_exec": {
      // ApprovalResolved/ProposalResolved is followed by AgentExec for that command.
      const resolved = find(s, s.awaitingResult[ev.agentId]);
      const it = resolved && resolved.text === ev.cmd ? resolved : command(s, ev.agentId, ev.cmd, t, ev.intent);
      delete s.awaitingResult[ev.agentId];
      it.submissionId = ev.submissionId;
      if (ev.submissionId) s.refs[`submission:${ev.submissionId}`] = it.id;
      it.policy = ev.policy; if (ev.intent) it.intent = ev.intent;
      finish(s, it, policyOutcome(ev.policy ?? ""), t); break;
    }
    case "shell_command_started": {
      if (s.refs[`command:${ev.commandId}`]) break;
      const it = find(s, s.refs[`submission:${ev.submissionId}`]) ?? add(s, ev.actor || "shell", "exec", ev.cmd, t);
      it.commandId = ev.commandId; it.cwd = ev.cwd; it.status = "running";
      s.refs[`command:${ev.commandId}`] = it.id;
      step(it, t, "running"); break;
    }
    case "shell_command_finished": {
      const it = find(s, s.refs[`command:${ev.commandId}`]);
      if (!it) break;
      it.exitCode = ev.exitCode; it.durationMs = ev.durationMs;
      finish(s, it, ev.exitCode == null ? "unknown" : ev.exitCode === 0 ? "completed" : "failed", t); break;
    }
    case "human_exec": {
      const it = add(s, "human", "exec", ev.cmd, t); finish(s, it, "executed", t); break;
    }
    case "attention_requested":
    case "tab_opened":
    case "agent_switched_tab": {
      if (ev.event === "agent_switched_tab" && ev.session !== ev.to) break;
      const it = add(s, ev.agentId, ev.event === "attention_requested" ? "attention" : "tab", ev.reason ?? "", t);
      it.status = "info"; step(it, t, ev.event); break;
    }
  }
}
/** Old audit entries have no reliable tab/lease identity. Label them explicitly, without guessing links. */
export function importSavedActivity(s: TimelineState, entries: Record<string, any>[]) {
  if (s.externalPrivate) return;
  for (const e of entries) {
    if (e.externalPrivate || e.origin === "external_automation" || e.originalRequest?.params?.origin === "external_automation" || String(e.actor ?? "").startsWith("AppleScript · ")) continue;
    if (e.action === "control_request_resolved" && ["denied", "expired"].includes(e.state)) {
      const it = add(s, e.actor, "control", e.reason ?? "", Date.parse(e.ts) || Date.now());
      it.originalRequest = e.originalRequest;
      it.saved = true; it.status = e.state; step(it, it.t, e.state);
    }
    if (e.action === "shell_command_started" || e.action === "shell_command_finished") {
      recordTimeline(s, { ...e, event: e.action }, Date.parse(e.ts) || Date.now());
      const it = find(s, s.refs[`command:${e.commandId}`]);
      if (it) { it.saved = true; if (it.status === "running") it.status = "unknown"; }
      continue;
    }
    if (e.action !== "exec" || !e.cmd) continue;
    const it = add(s, e.actor, "exec", e.cmd, Date.parse(e.ts) || Date.now());
    it.saved = true; it.intent = e.intent; it.submissionId = e.submissionId;
    if (e.submissionId) s.refs[`submission:${e.submissionId}`] = it.id;
    it.policy = `${e.policy ?? "allow"}${e.policy === "deny" && e.label ? `:${e.label}` : e.approval ? `:${e.approval}` : ""}`;
    // Restore explicit audit facts only; never infer approval from policy or a lease.
    if (e.approval === "granted") step(it, it.t, "approval_granted");
    if (e.by === "human_commit") step(it, it.t, "proposal_committed");
    if (e.by === "human_cosign") step(it, it.t, "exec_cosigned");
    it.status = policyOutcome(it.policy); step(it, it.t, it.status);
  }
}
