//! Session core. Owns every piece of state and enforces the invariants:
//!
//! 1. One writer at a time; the human by default.
//! 2. Human input revokes the agent lease *before* the bytes reach the PTY.
//! 3. Policy is checked at ENTER, on the command line the shell will actually run.
//!
//! This module knows nothing about MCP, the CLI, sockets, or any UI toolkit. It
//! talks to the outside world through two byte sinks (PTY in, human output out) and
//! `EventSink`s registered per connection.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{MasterPty, PtySize};
use serde::Serialize;
use serde_json::json;

use crate::affordance::{affordances_for, Actor, Affordance, AffordanceState};
use crate::approval::{ApprovalQueue, ApprovalRequest, ApprovalState, Decision as ApprovalDecision};
use crate::audit::Audit;
use crate::authority::{Authority, AuthorityError, ConnId, Controller, Lease, RevokeReason};
use crate::config::Pacing;
use crate::input::{self, InputTracker};
use crate::policy::{Decision as PolicyDecision, PolicyStore, Reload};
use crate::screen::{Projection, ScreenModel, Size, SurfaceFrame};

/// Something that can deliver server-push events to a connected client.
pub trait EventSink: Send {
    fn send(&self, event: ServerEvent);
    fn send_scoped(&self, event: ServerEvent, _generation: u64) { self.send(event); }
}

impl<F: Fn(ServerEvent) + Send> EventSink for F {
    fn send(&self, event: ServerEvent) {
        self(event)
    }
}

/// Events. Agents receive the ones that concern them; frontends receive all of them.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ServerEvent {
    ControlGranted { lease: String, #[serde(rename = "agentId")] agent_id: String, reason: Option<String>, #[serde(rename = "originalRequest")] original_request: Option<serde_json::Value> },
    ControlRevoked { lease: String, #[serde(rename = "agentId")] agent_id: String, reason: RevokeReason },
    /// The set of tools available to the receiving agent changed.
    ToolsChanged,
    ApprovalRequested { request: ApprovalRequest },
    ApprovalResolved { #[serde(rename = "approvalId")] approval_id: String, state: ApprovalState, by: String },
    /// An agent wrote bytes to the input line (for typing indicators).
    AgentInput { #[serde(rename = "agentId")] agent_id: String, len: usize },
    /// An agent's ENTER passed policy and was scheduled after `enter_grace_ms`.
    ExecScheduled { #[serde(rename = "execId")] exec_id: String, #[serde(rename = "agentId")] agent_id: String, cmd: String, intent: Option<String>, #[serde(rename = "graceMs")] grace_ms: u64 },
    ExecCosigned { #[serde(rename = "execId")] exec_id: String, #[serde(rename = "agentId")] agent_id: String },
    ExecCancelled { #[serde(rename = "execId")] exec_id: String, reason: String },
    /// An agent command reached the shell (or was denied).
    AgentExec { #[serde(rename = "agentId")] agent_id: String, cmd: String, policy: String, intent: Option<String>, #[serde(rename = "submissionId", skip_serializing_if = "Option::is_none")] submission_id: Option<String> },
    ShellIntegrationChanged { status: crate::shell_integration::IntegrationStatus },
    ShellCommandStarted { #[serde(rename = "commandId")] command_id: String, #[serde(rename = "submissionId")] submission_id: Option<String>, actor: String, cmd: String, cwd: String },
    ShellCommandFinished { #[serde(rename = "commandId")] command_id: String, #[serde(rename = "exitCode")] exit_code: Option<i32>, #[serde(rename = "durationMs")] duration_ms: u64 },
    /// The visible screen changed (coalesced by the tick).
    ScreenChanged { revision: u64 },
    SharingChanged { shared: bool, generation: u64 },
    ProcessExited { #[serde(rename = "exitCode")] exit_code: Option<u32> },
    PacingChanged { pacing: Pacing },
    AffordanceMaskChanged { allow: Option<Vec<Affordance>> },
    ModeChanged { mode: AgentMode, #[serde(rename = "effectiveMode")] effective_mode: AgentMode },
    ControlGateChanged { ask: bool },
    /// An agent asked for control and the gate is on; the human decides.
    ControlRequested { request: ControlRequest },
    ControlRequestResolved { #[serde(rename = "requestId")] request_id: String, state: ControlRequestState },
    /// Copilot mode: the proposed command line changed (ghost text).
    ProposalChanged { proposal: Proposal },
    ProposalResolved { #[serde(rename = "proposalId")] proposal_id: String, state: ProposalState, cmd: String, policy: Option<String> },
    /// The human started or stopped looking at this session.
    AttentionChanged { attended: bool },
    /// An agent asks the human to come back to this session.
    AttentionRequested { #[serde(rename = "agentId")] agent_id: String, reason: Option<String> },
    /// An agent opened this session as a new tab. It starts unattended.
    TabOpened { #[serde(rename = "agentId")] agent_id: String, reason: Option<String> },
    /// An agent's connection moved between tabs. Sent to both sessions.
    AgentSwitchedTab { #[serde(rename = "agentId")] agent_id: String, from: String, to: String },
    /// The human explicitly returned control to the previous agent.
    ControlHandedBack { lease: String, #[serde(rename = "agentId")] agent_id: String, #[serde(rename = "lastCmd")] last_cmd: Option<String> },
    SessionAllowsChanged { allows: Vec<String> },
}

/// How much an agent may do. Set by the frontend; narrows the state-derived table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    /// Snapshot only.
    Observe,
    /// The agent proposes a command line (ghost text); the human commits it with ENTER.
    Copilot,
    /// The agent holds a lease and executes; policy decides what to ask.
    #[default]
    Autopilot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalState {
    /// Being typed by the agent.
    Drafting,
    /// The agent pressed ENTER; waiting for the human to commit or reject.
    Ready,
    Executed,
    Rejected,
    Denied,
}

#[derive(Debug, Clone, Serialize)]
pub struct Proposal {
    #[serde(rename = "proposalId")]
    pub proposal_id: String,
    #[serde(skip)]
    pub conn: ConnId,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    pub text: String,
    pub state: ProposalState,
    /// What the agent says the proposed command is for (set at ENTER).
    pub intent: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ControlRequestState {
    Pending,
    Granted,
    Denied,
    Expired,
}

fn params_command_changed(previous: &serde_json::Value, next: &serde_json::Value) -> bool {
    next["params"]["command"].is_string() && previous["params"]["command"] != next["params"]["command"]
}

/// A `request_control` held for the human when the control gate is on.
#[derive(Debug, Clone, Serialize)]
pub struct ControlRequest {
    #[serde(rename = "requestId")]
    pub request_id: String,
    #[serde(skip)]
    pub conn: ConnId,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    pub reason: Option<String>,
    #[serde(rename = "originalRequest")]
    pub original_request: serde_json::Value,
    pub state: ControlRequestState,
    #[serde(skip)]
    pub created: Instant,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum ControlOutcome {
    Granted {
        #[serde(flatten)]
        lease: LeaseInfo,
    },
    Pending {
        #[serde(rename = "requestId")]
        request_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnKind {
    Agent,
    Frontend,
}

struct ConnInfo {
    kind: ConnKind,
    name: String,
    sink: Box<dyn EventSink>,
    last_activity: Instant,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error(transparent)]
    Authority(#[from] AuthorityError),
    #[error("the shell process has exited")]
    ProcessExited,
    #[error("shell input is pending; clear or cancel it before changing mode")]
    InputPending,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("an approval is pending ({0}); wait for it or call interrupt")]
    ApprovalPending(String),
    #[error("an execution is scheduled ({0}); wait for it")]
    ExecPending(String),
    #[error("rate limited; retry after {retry_after_ms} ms")]
    RateLimited { retry_after_ms: u64 },
    #[error("affordance '{0}' is disabled for agents by the frontend")]
    Masked(String),
    #[error("not available in {0:?} mode")]
    WrongMode(AgentMode),
    #[error("a proposal is pending ({0}); the human must commit or reject it")]
    ProposalPending(String),
    #[error("intent is required: say in one line what this command does and changes")]
    IntentRequired,
    #[error("'{0}' is not available right now")]
    NotAvailable(String),
    #[error("terminal screen revision is no longer current")]
    SurfaceUnavailable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ControllerInfo {
    Human,
    Agent {
        #[serde(rename = "agentId")]
        agent_id: String,
        #[serde(rename = "leaseId")]
        lease_id: String,
        #[serde(rename = "expiresInSecs")]
        expires_in_secs: u64,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub mode: AgentMode,
    #[serde(rename = "effectiveMode")]
    pub effective_mode: AgentMode,
    #[serde(flatten)]
    pub projection: Projection,
    pub controller: ControllerInfo,
    #[serde(rename = "processAlive")]
    pub process_alive: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LeaseInfo {
    #[serde(rename = "leaseId")]
    pub lease_id: String,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    #[serde(rename = "ttlSecs")]
    pub ttl_secs: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum KeyResult {
    Sent,
    Executed { cmd: String },
    /// Allowed, but held for `enter_grace_ms`. Poll `exec_state` or wait for the event.
    Scheduled { #[serde(rename = "execId")] exec_id: String, cmd: String, #[serde(rename = "graceMs")] grace_ms: u64 },
    Cancelled { cmd: String, reason: String },
    /// Copilot mode: the line is proposed; waiting for the human to commit.
    Proposed { #[serde(rename = "proposalId")] proposal_id: String, cmd: String },
    Rejected { cmd: String },
    Denied { cmd: String, label: String },
    Pending {
        #[serde(rename = "approvalId")]
        approval_id: String,
        cmd: String,
        label: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecState {
    Scheduled,
    Executed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScheduledExec {
    #[serde(rename = "execId")]
    pub exec_id: String,
    #[serde(skip)]
    pub conn: ConnId,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    pub cmd: String,
    pub intent: Option<String>,
    #[serde(skip)]
    pub due: Instant,
    pub state: ExecState,
    #[serde(skip)]
    pub cancel_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApprovalInfo {
    #[serde(rename = "approvalId")]
    pub approval_id: String,
    pub state: ApprovalState,
    pub cmd: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConnection {
    pub conn_id: ConnId,
    pub agent_id: String,
    pub idle_secs: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    #[serde(rename = "shellIntegration")]
    pub shell_integration: crate::shell_integration::IntegrationStatus,
    pub shared: bool,
    #[serde(rename = "externalOrigin")]
    pub external_origin: bool,
    #[serde(rename = "surfaceGeneration")]
    pub surface_generation: u64,
    #[serde(rename = "outputSeq")]
    pub output_seq: u64,
    #[serde(rename = "surfaceAvailable")]
    pub surface_available: bool,
    #[serde(rename = "completionPromptReady")]
    pub completion_prompt_ready: bool,
    #[serde(rename = "inputPending")]
    pub input_pending: bool,
    #[serde(rename = "externalInputAvailable")]
    pub external_input_available: bool,
    #[serde(rename = "profileId")]
    pub profile_id: Option<String>,
    #[serde(rename = "profileName")]
    pub profile_name: Option<String>,
    #[serde(rename = "reviewRequired")]
    pub review_required: bool,
    pub controller: ControllerInfo,
    #[serde(rename = "processAlive")]
    pub process_alive: bool,
    pub revision: u64,
    pub size: Size,
    pub pending: Vec<ApprovalRequest>,
    pub scheduled: Option<ScheduledExec>,
    #[serde(rename = "sessionAllows")]
    pub session_allows: Vec<String>,
    #[serde(rename = "policyPath")]
    pub policy_path: Option<String>,
    #[serde(rename = "connectedAgents")]
    pub connected_agents: Vec<String>,
    #[serde(rename = "agentConnections")]
    pub agent_connections: Vec<AgentConnection>,
    pub pacing: Pacing,
    #[serde(rename = "affordanceMask")]
    pub affordance_mask: Option<Vec<Affordance>>,
    pub mode: AgentMode,
    /// Effective mode; unobserved sessions have no input authority.
    #[serde(rename = "effectiveMode")]
    pub effective_mode: AgentMode,
    pub attended: bool,
    #[serde(rename = "attentionRequest")]
    pub attention_request: Option<AttentionRequest>,
    /// The agent that opened this session as a tab, if any.
    #[serde(rename = "openedBy")]
    pub opened_by: Option<String>,
    #[serde(rename = "controlGate")]
    pub control_gate: bool,
    #[serde(rename = "controlRequests")]
    pub control_requests: Vec<ControlRequest>,
    pub proposal: Option<Proposal>,
    #[serde(rename = "lastAgent")]
    pub last_agent: Option<LastAgent>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttentionRequest {
    #[serde(rename = "agentId")]
    pub agent_id: String,
    pub reason: Option<String>,
    pub at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LastAgent {
    #[serde(rename = "agentId")]
    pub agent_id: String,
    pub connected: bool,
    #[serde(rename = "lastCmd")]
    pub last_cmd: Option<String>,
}

/// Owner-only rendered output delivery, installed before the PTY reader starts.
#[derive(Debug, Clone)]
pub struct OutputFrame {
    pub data: Vec<u8>,
    pub output_seq: u64,
    pub generation: u64,
}
pub type OutputFrameSink = Box<dyn Fn(&OutputFrame) + Send + Sync>;

pub struct SessionConfig {
    pub rows: u16,
    pub cols: u16,
    pub audit: Audit,
    pub policy: PolicyStore,
    pub pty_writer: Box<dyn Write + Send>,
    pub master: Option<Box<dyn MasterPty + Send>>,
    pub pacing: Pacing,
    /// Pid of the shell, used to read its cwd (`lsof -d cwd`) when analysing a command.
    pub shell_pid: Option<u32>,
}

/// None means every connected agent was affected (the previous audience was unrestricted).
type ParticipationListener = Box<dyn Fn(Option<&HashSet<ConnId>>) + Send + Sync>;

pub struct Session {
    // Origin survives participation transitions. Only native owners change sharing.
    external_origin: bool,
    shared: bool,
    participants: Option<HashSet<ConnId>>,
    participation_listener: Option<ParticipationListener>,
    surface_generation: u64,
    participation_generation: u64,
    output_seq: u64,
    surface_id: String,
    external_writer_active: bool,
    execution_profile: Option<crate::backend::Profile>,
    authority: Authority,
    policy: PolicyStore,
    approvals: ApprovalQueue,
    screen: ScreenModel,
    input: InputTracker,
    human_input_pending: bool,
    shell_integration: crate::shell_integration::IntegrationStatus,
    shell_command: Option<crate::shell_integration::RunningCommand>,
    // Content-free state closes the input-to-async-shell-hook interval.
    shell_prompt_confirmed: bool,
    shell_submission_inflight: bool,
    shell_foreground_sequence: Option<u64>,
    shell_recording_ready: bool,
    shell_recording_armed: bool,
    shell_submissions: Vec<crate::shell_integration::Submission>,
    shell_human_input: bool,
    shell_agent_input: bool,
    audit: Audit,
    pty: Box<dyn Write + Send>,
    output_frame_sink: Option<OutputFrameSink>,
    master: Option<Box<dyn MasterPty + Send>>,
    pacing: Pacing,
    process_alive: bool,
    conns: HashMap<ConnId, ConnInfo>,
    next_local_conn: ConnId,
    affordance_mask: Option<HashSet<Affordance>>,
    last_agent_write: Option<Instant>,
    scheduled: Option<ScheduledExec>,
    exec_seq: u64,
    screen_dirty: bool,
    mode: AgentMode,
    control_gate: bool,
    control_requests: Vec<ControlRequest>,
    control_seq: u64,
    proposal: Option<Proposal>,
    proposal_seq: u64,
    /// The agent that most recently held control, for hand-back.
    last_agent: Option<(ConnId, String)>,
    last_agent_cmd: Option<String>,
    shell_pid: Option<u32>,
    cwd_override: Option<std::path::PathBuf>,
    attended: bool,
    attention_request: Option<AttentionRequest>,
    opened_by: Option<String>,
    tabs_supported: bool,
    trace: Option<Arc<Mutex<Vec<String>>>>,
}

pub type SharedSession = Arc<parking_lot::Mutex<Session>>;

fn key_bytes(key: &str) -> Option<&'static [u8]> {
    Some(match key.to_ascii_uppercase().as_str() {
        "ENTER" => b"\r",
        "TAB" => b"\t",
        "ESC" | "ESCAPE" => b"\x1b",
        "BACKSPACE" => b"\x7f",
        "UP" => b"\x1b[A",
        "DOWN" => b"\x1b[B",
        "LEFT" => b"\x1b[D",
        "RIGHT" => b"\x1b[C",
        "CTRL_C" => b"\x03",
        "CTRL_D" => b"\x04",
        _ => return None,
    })
}

pub const SUPPORTED_KEYS: &[&str] = &[
    "ENTER", "TAB", "ESC", "BACKSPACE", "UP", "DOWN", "LEFT", "RIGHT", "CTRL_C", "CTRL_D",
];

/// Connection ids handed out by the socket server start at 1. Local subscribers (embedders)
/// get ids from a separate high range so the two never collide.
const LOCAL_CONN_BASE: ConnId = 1 << 40;

impl Session {
    pub fn new(cfg: SessionConfig) -> Self {
        Self::with_origin(cfg, false)
    }

    /// Construct a private native session with activity recording disabled.
    pub fn new_external_private(cfg: SessionConfig) -> Self {
        cfg.audit.set_enabled(false);
        Self::with_origin(cfg, true)
    }

    fn with_origin(cfg: SessionConfig, external_private: bool) -> Self {
        Self {
            external_origin: external_private,
            shared: !external_private,
            participants: if external_private { Some(HashSet::new()) } else { None },
            participation_listener: None,
            surface_generation: 1,
            participation_generation: 1,
            output_seq: 0,
            surface_id: uuid::Uuid::new_v4().to_string(),
            external_writer_active: external_private,
            authority: Authority::new(Duration::from_secs(cfg.pacing.lease_ttl_secs)),
            policy: cfg.policy,
            approvals: ApprovalQueue::new(Duration::from_secs(cfg.pacing.approval_ttl_secs)),
            screen: ScreenModel::new(cfg.rows, cfg.cols),
            input: InputTracker::new(),
            human_input_pending: false,
            shell_integration: crate::shell_integration::IntegrationStatus::unavailable(if external_private { "private_session" } else { "unsupported_shell" }),
            shell_command: None,
            shell_prompt_confirmed: false,
            shell_submission_inflight: false,
            shell_foreground_sequence: None,
            shell_recording_ready: false,
            shell_recording_armed: false,
            shell_submissions: Vec::new(),
            shell_human_input: false,
            shell_agent_input: false,
            audit: cfg.audit,
            pty: cfg.pty_writer,
            output_frame_sink: None,
            master: cfg.master,
            pacing: cfg.pacing,
            process_alive: true,
            conns: HashMap::new(),
            next_local_conn: LOCAL_CONN_BASE,
            affordance_mask: None,
            last_agent_write: None,
            scheduled: None,
            exec_seq: 0,
            screen_dirty: false,
            mode: AgentMode::Autopilot,
            control_gate: false,
            control_requests: Vec::new(),
            control_seq: 0,
            proposal: None,
            proposal_seq: 0,
            last_agent: None,
            last_agent_cmd: None,
            execution_profile: None,
            shell_pid: cfg.shell_pid,
            cwd_override: None,
            attended: true,
            attention_request: None,
            opened_by: None,
            tabs_supported: false,
            trace: None,
        }
    }

    pub fn is_private(&self) -> bool { !self.shared }
    pub fn external_origin(&self) -> bool { self.external_origin }
    pub fn surface_generation(&self) -> u64 { self.surface_generation }
    pub fn participation_generation(&self) -> u64 { self.participation_generation }
    pub fn output_seq(&self) -> u64 { self.output_seq }
    /// Native owner installs a log at the moment sharing is enabled, never at external startup.
    pub fn activate_shared_audit(&mut self, audit: Audit) { audit.set_enabled(self.shared); self.audit = audit; }

    pub fn set_output_frame_sink(&mut self, sink: Option<OutputFrameSink>) { self.output_frame_sink = sink; }
    pub fn participant_allowed(&self, conn: ConnId) -> bool {
        self.shared && self.participants.as_ref().is_none_or(|ids| ids.contains(&conn))
    }

    pub fn review_required(&self) -> bool {
        self.external_origin || self.execution_profile.as_ref().is_some_and(|profile| profile.needs_review())
            || (self.shell_integration.state == "active" && (!self.shell_prompt_confirmed || self.shell_submission_inflight))
    }

    pub fn completion_prompt_ready(&self) -> bool {
        self.shared && self.process_alive && self.shell_integration.state == "active" && self.shell_command.is_none()
            && self.shell_prompt_confirmed && !self.shell_submission_inflight
            && !self.external_origin && self.authority.controller().is_human()
    }

    /// Completion insertion is a human acceptance, not a second unchecked writer.
    pub fn accept_completion(&mut self, surface_id: &str, generation: u64, revision: u64, text: &str) -> Result<(), SessionError> {
        let frame = self.authoritative_surface()?;
        if frame.surface_id != surface_id || frame.generation != generation || frame.revision != revision {
            return Err(SessionError::SurfaceUnavailable);
        }
        if text.len() > 8192 || text.chars().any(char::is_control) {
            return Err(SessionError::InvalidInput("completion must be bounded text without execution controls".into()));
        }
        self.human_input(text.as_bytes());
        Ok(())
    }

    /// Current terminal grid derived exclusively from PTY output. Window focus,
    /// renderer heartbeats and scrollback position never authorize participation.
    pub fn authoritative_surface(&self) -> Result<SurfaceFrame, SessionError> {
        self.require_shared()?;
        let size = self.screen.size();
        Ok(SurfaceFrame {
            surface_id: self.surface_id.clone(), generation: self.surface_generation,
            revision: self.screen.revision(), output_seq: self.output_seq,
            rows: size.rows, cols: size.cols,
            cursor: self.screen.visible_cursor(), screen: self.screen.rows(),
            alternate_screen: self.screen.alternate_screen(),
        })
    }

    /// Only the local owner may add/remove agent participation. The PTY is untouched.
    pub fn set_shared(&mut self, shared: bool) -> Result<(), SessionError> {
        self.set_shared_with_agents(shared, Vec::new())
    }

    /// Host-only catalog invalidation. The listener must not lock any session.
    pub(crate) fn set_participation_listener(&mut self, listener: ParticipationListener) {
        self.participation_listener = Some(listener);
    }

    pub fn set_shared_with_agents(&mut self, shared: bool, agents: Vec<ConnId>) -> Result<(), SessionError> {
        if agents.len() > 64 { return Err(SessionError::InvalidInput("at most 64 participants".into())); }
        let selected: HashSet<_> = agents.into_iter().collect();
        if !self.process_alive { return Err(SessionError::ProcessExited); }
        if self.shared == shared && self.participants.as_ref() == Some(&selected) { return Ok(()); }
        if shared && (self.input.has_pending() || self.human_input_pending) { return Err(SessionError::InputPending); }
        let affected = if self.shared && self.participants.is_none() {
            None
        } else {
            let mut ids = if self.shared { self.participants.clone().unwrap_or_default() } else { HashSet::new() };
            if shared { ids.extend(selected.iter().copied()); }
            Some(ids)
        };
        // Revocation cannot be held hostage by a partial agent write. Preserve the
        // physical input for the owner instead of guessing a clear key in a TUI.
        let unfinished_agent_input = self.input.has_pending();
        self.revoke_external();
        self.cancel_agent_work("sharing_changed");
        self.human_input_pending |= unfinished_agent_input && self.input.has_pending();
        if !shared { self.audit.record("human", "sharing_stopped", json!({ "generation": self.surface_generation })); }
        self.audit.set_enabled(shared);
        self.shared = shared;
        self.participation_generation = self.participation_generation.saturating_add(1);
        self.participants = Some(selected);
        self.surface_generation = self.surface_generation.saturating_add(1);
        self.input.reset();
        self.shell_submissions.clear();
        self.shell_command = None;
        self.shell_prompt_confirmed = false;
        self.shell_recording_ready = false;
        self.shell_recording_armed = false;
        self.shell_human_input = false;
        self.shell_agent_input = false;
        self.last_agent = None;
        self.last_agent_cmd = None;
        self.attention_request = None;
        if shared { self.audit.record("human", "sharing_started", json!({ "generation": self.surface_generation })); }
        self.broadcast(ServerEvent::SharingChanged { shared, generation: self.surface_generation });
        self.notify_tools_changed();
        // New participants have no session subscription yet; removed participants'
        // session events are deliberately discarded by the transport's ACL guard.
        if let Some(listener) = &self.participation_listener { listener(affected.as_ref()); }
        Ok(())
    }

    fn cancel_agent_work(&mut self, reason: &str) {
        self.cancel_scheduled_if(|_| true, reason);
        self.reject_proposal_if(|_| true, reason);
        let ids: Vec<_> = self.approvals.pending().into_iter().map(|a| a.id.clone()).collect();
        for id in ids { self.finish_approval(&id, ApprovalState::Denied, reason); }
        let ids: Vec<_> = self.control_requests.iter().filter(|r| r.state == ControlRequestState::Pending).map(|r| r.request_id.clone()).collect();
        for id in ids { self.finish_control_request(&id, ControlRequestState::Denied); }
        if let Some(lease) = self.authority.revoke() {
            let ev = ServerEvent::ControlRevoked { lease: lease.label(), agent_id: lease.agent_id, reason: RevokeReason::Taken };
            self.notify(lease.conn, ev.clone());
            self.broadcast(ev);
        }
    }

    pub(crate) fn shell_integration_starting(&mut self) {
        if self.is_private() { return; }
        self.shell_prompt_confirmed = false;
        self.shell_integration.state = "starting".into();
        self.shell_integration.reason = None;
    }

    pub(crate) fn shell_integration_lost(&mut self, reason: &str) {
        if self.is_private() { return; }
        self.finish_shell_command(None);
        self.shell_prompt_confirmed = false;
        self.shell_integration.state = "unavailable".into();
        self.shell_integration.reason = Some(reason.into());
        self.shell_submissions.clear();
        self.broadcast(ServerEvent::ShellIntegrationChanged { status: self.shell_integration.clone() });
    }

    fn shell_submission(&mut self, actor: &str, command: &str) -> Option<String> {
        self.shell_agent_input = true;
        self.shell_recording_armed |= self.shared && self.shell_recording_ready && self.shell_prompt_confirmed;
        if self.is_private() || self.shell_integration.state != "active" || self.shell_command.is_some() { return None; }
        let id = uuid::Uuid::new_v4().to_string();
        if self.shell_submissions.len() >= 16 { self.shell_submissions.remove(0); }
        self.shell_submissions.push(crate::shell_integration::Submission { id: id.clone(), actor: actor.into(), command: command.into(), at: Instant::now() });
        Some(id)
    }

    pub(crate) fn shell_event(&mut self, sequence: u64, kind: &str, text: &str, cwd: &str, result: &str) {
        if !self.process_alive { return; }
        // Lifecycle sequence alone is retained privately. It identifies the outer
        // shell's foreground program without retaining its command or arguments.
        let matching_end = kind == "end" && self.shell_foreground_sequence.is_some()
            && self.shell_foreground_sequence == text.parse::<u64>().ok();
        let record_start = self.shared && self.shell_recording_ready && self.shell_recording_armed;
        match kind {
            "ready" => {
                self.shell_prompt_confirmed = !self.shell_submission_inflight;
                self.shell_integration.state = "active".into();
                self.shell_integration.shell = Some(text.into());
                self.shell_integration.reason = None;
                self.shell_recording_ready = self.shared && self.shell_prompt_confirmed;
            }
            "start" => {
                self.shell_foreground_sequence = Some(sequence);
                self.shell_prompt_confirmed = false;
                self.shell_recording_ready = false;
                self.shell_recording_armed = false;
            }
            "end" if matching_end => {
                self.shell_foreground_sequence = None;
                self.shell_submission_inflight = false;
                self.shell_prompt_confirmed = true;
                self.shell_recording_ready = self.shared && self.shell_integration.state == "active";
                self.shell_recording_armed = false;
            }
            "gap" | "unavailable" => {
                self.shell_prompt_confirmed = false;
                self.shell_recording_ready = false;
                self.shell_recording_armed = false;
                self.shell_foreground_sequence = None;
            }
            _ => {}
        }
        if self.is_private() { return; }
        match kind {
            "ready" => self.broadcast(ServerEvent::ShellIntegrationChanged { status: self.shell_integration.clone() }),
            "gap" | "unavailable" => self.shell_integration_lost(if kind == "gap" { "event_gap" } else { "hook_conflict" }),
            "start" if self.shell_integration.state == "active" && !text.is_empty() => {
                self.finish_shell_command(None);
                let pending = std::mem::take(&mut self.shell_submissions);
                // A shared prompt followed by new local input is required after a
                // sharing boundary. Delayed private mailbox payloads are discarded.
                if !record_start { return; }
                let matched = if pending.len() == 1 && pending[0].command.trim() == text.trim() && pending[0].at.elapsed() < Duration::from_secs(5) { pending.into_iter().next() } else { None };
                let actor = matched.as_ref().map(|p| p.actor.clone()).unwrap_or_else(|| if self.shell_human_input && !self.shell_agent_input { "human".into() } else { "shell".into() });
                let submission_id = matched.map(|p| p.id);
                let id = uuid::Uuid::new_v4().to_string();
                self.shell_command = Some(crate::shell_integration::RunningCommand { id: id.clone(), actor: actor.clone(), started: Instant::now() });
                self.shell_human_input = false;
                self.shell_agent_input = false;
                self.audit.record(&actor, "shell_command_started", json!({ "commandId": id, "submissionId": submission_id, "cmd": text, "cwd": cwd }));
                self.broadcast(ServerEvent::ShellCommandStarted { command_id: id, submission_id, actor, cmd: text.into(), cwd: cwd.into() });
            }
            "end" if matching_end => {
                self.finish_shell_command(result.parse::<i32>().ok().filter(|c| (0..=255).contains(c)));
                self.shell_submissions.clear();
                self.shell_human_input = false;
                self.shell_agent_input = false;
            }
            _ => {}
        }
    }

    fn finish_shell_command(&mut self, exit_code: Option<i32>) {
        let Some(command) = self.shell_command.take() else { return; };
        let duration_ms = command.started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        self.audit.record(&command.actor, "shell_command_finished", json!({ "commandId": command.id, "exitCode": exit_code, "durationMs": duration_ms }));
        self.broadcast(ServerEvent::ShellCommandFinished { command_id: command.id, exit_code, duration_ms });
    }

    pub fn external_writer_active(&self) -> bool { self.is_private() && self.external_writer_active && self.process_alive }

    /// Revocation is permanent for this session. Native ownership is checked by the caller.
    pub fn revoke_external(&mut self) { self.external_writer_active = false; }

    /// A bounded native delivery lets human input preempt between chunks.
    pub fn write_external(&mut self, bytes: &[u8]) -> Result<(), SessionError> {
        if !self.external_writer_active() { return Err(SessionError::NotAvailable("session unavailable".into())); }
        self.write_terminal_response(bytes)?;
        if !bytes.is_empty() { self.human_input_pending = !matches!(bytes.last(), Some(b'\r' | b'\n' | 3 | 21)); }
        Ok(())
    }

    /// In-process terminal protocol replies are not human keystrokes or external
    /// automation requests. Native UI ownership must be checked before calling.
    /// They keep working after human takeover without restoring external access.
    pub fn write_terminal_response(&mut self, bytes: &[u8]) -> Result<(), SessionError> {
        if !self.process_alive { return Err(SessionError::NotAvailable("session unavailable".into())); }
        if bytes.len() > 1024 { return Err(SessionError::InvalidInput("external input chunk is too large".into())); }
        if !self.write_external_chunk(bytes) {
            self.revoke_external();
            return Err(SessionError::NotAvailable("external input failed".into()));
        }
        Ok(())
    }

    fn write_external_chunk(&mut self, bytes: &[u8]) -> bool {
        #[cfg(unix)]
        if let Some(fd) = self.master.as_ref().and_then(|m| m.as_raw_fd()) {
            // The session lock serializes writers. The reader tolerates the brief
            // nonblocking flag; backpressure never holds the lock or retries bytes.
            // SAFETY: master owns fd throughout this call; bytes is valid for len.
            unsafe {
                let flags = libc::fcntl(fd, libc::F_GETFL);
                if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 { return false; }
                let written = libc::write(fd, bytes.as_ptr().cast(), bytes.len());
                let restored = libc::fcntl(fd, libc::F_SETFL, flags) == 0;
                return restored && written == bytes.len() as isize;
            }
        }
        // In-memory embedders may supply a non-PTY writer. Platform adapters
        // must provide equivalent bounded delivery before exposing this path.
        self.pty.write_all(bytes).and_then(|_| self.pty.flush()).is_ok()
    }

    fn require_participant(&self, conn: ConnId) -> Result<(), SessionError> {
        if self.participant_allowed(conn) && self.conn_kind(conn) == Some(ConnKind::Agent) { Ok(()) } else { Err(SessionError::NotAvailable("participation not granted".into())) }
    }

    fn require_shared(&self) -> Result<(), SessionError> {
        if self.is_private() { Err(SessionError::NotAvailable("session unavailable".into())) } else { Ok(()) }
    }

    /// Test hook: record the order of internal effects.
    pub fn set_trace(&mut self, trace: Arc<Mutex<Vec<String>>>) {
        if !self.is_private() { self.trace = Some(trace); }
    }

    /// Test hook / embedder override: the shell's cwd for target resolution.
    pub fn set_cwd_override(&mut self, cwd: Option<std::path::PathBuf>) {
        self.cwd_override = cwd;
    }

    /// The shell's current directory, read from the process (`lsof -d cwd`) so
    /// `cd` history does not have to be tracked. None when unknown.
    pub fn set_execution_profile(&mut self, profile: crate::backend::Profile) { self.execution_profile = Some(profile); }

    pub fn shell_cwd(&self) -> Option<std::path::PathBuf> {
        if self.execution_profile.as_ref().is_some_and(|p|p.backend != crate::backend::BackendKind::Local) { return None; }
        if let Some(c) = &self.cwd_override {
            return Some(c.clone());
        }
        let pid = self.shell_pid?;
        #[cfg(target_os="linux")] { return std::fs::read_link(format!("/proc/{pid}/cwd")).ok(); }
        #[cfg(windows)] { let _ = pid; return None; }
        #[cfg(not(any(target_os="linux", windows)))]
        let out = std::process::Command::new("lsof").args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"]).output().ok()?;
        #[cfg(not(any(target_os="linux", windows)))]
        let text = String::from_utf8_lossy(&out.stdout);
        #[cfg(not(any(target_os="linux", windows)))]
        text.lines().find_map(|l| l.strip_prefix('n').map(|p| std::path::PathBuf::from(p)))
    }

    fn analyse(&self, cmd: &str) -> crate::policy::LineAnalysis {
        if self.review_required() {
            let shell = self.execution_profile.as_ref().filter(|p| p.needs_review()).map_or(crate::backend::ShellKind::Custom, |p| p.shell);
            return self.policy.analyse_external(cmd, shell);
        }
        let cwd = self.shell_cwd();
        let home = dirs::home_dir();
        self.policy.analyse(cmd, cwd.as_deref(), home.as_deref())
    }

    fn trace(&self, what: impl Into<String>) {
        if let Some(t) = &self.trace {
            if let Ok(mut t) = t.lock() {
                t.push(what.into());
            }
        }
    }

    // ----- plumbing -------------------------------------------------------

    fn write_pty(&mut self, bytes: &[u8]) {
        if bytes.iter().any(|byte| matches!(byte, b'\r' | b'\n' | 3 | 4)) {
            self.shell_prompt_confirmed = false;
            self.shell_submission_inflight = true;
        }
        if !self.is_private() { self.trace("pty_write"); }
        let _ = self.pty.write_all(bytes);
        let _ = self.pty.flush();
    }

    fn write_output(&mut self, bytes: &[u8]) {
        if bytes.is_empty() { return; }
        self.output_seq = self.output_seq.saturating_add(1);
        if let Some(sink) = &self.output_frame_sink {
            sink(&OutputFrame { data: bytes.to_vec(), output_seq: self.output_seq, generation: self.surface_generation });
        }
    }

    fn notify(&self, conn: ConnId, ev: ServerEvent) {
        if let Some(c) = self.conns.get(&conn) {
            if c.kind == ConnKind::Agent && !self.participant_allowed(conn) { return; }
            c.sink.send_scoped(ev, self.participation_generation);
        }
    }

    fn notify_tools_changed(&self) {
        for c in self.conns.values() {
            if c.kind == ConnKind::Agent {
                c.sink.send_scoped(ServerEvent::ToolsChanged, self.participation_generation);
            }
        }
    }

    /// Deliver to every frontend.
    fn broadcast(&self, ev: ServerEvent) {
        for c in self.conns.values() {
            if c.kind == ConnKind::Frontend {
                c.sink.send_scoped(ev.clone(), self.participation_generation);
            }
        }
    }

    fn agent_name(&self, conn: ConnId) -> String {
        self.conns
            .get(&conn)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| format!("conn-{conn}"))
    }

    fn controller_info(&self) -> ControllerInfo {
        match self.authority.controller() {
            Controller::Human => ControllerInfo::Human,
            Controller::Agent(l) => ControllerInfo::Agent {
                agent_id: l.agent_id.clone(),
                lease_id: l.label(),
                expires_in_secs: l.expires_at.saturating_duration_since(Instant::now()).as_secs(),
            },
        }
    }

    // ----- connections ----------------------------------------------------

    pub fn register_conn(&mut self, conn: ConnId, kind: ConnKind, name: &str, sink: Box<dyn EventSink>) {
        if self.is_private() { return; }
        self.conns.insert(conn, ConnInfo { kind, name: name.to_string(), sink, last_activity: Instant::now() });
        if kind == ConnKind::Agent {
            self.audit.record(name, "connect", json!({ "conn": conn }));
        }
    }

    /// Register a frontend.
    pub fn register_frontend(&mut self, conn: ConnId, name: &str, sink: Box<dyn EventSink>) {
        if conn < LOCAL_CONN_BASE { return; }
        self.conns.insert(conn, ConnInfo { kind: ConnKind::Frontend, name: name.to_string(), sink, last_activity: Instant::now() });
    }

    /// Subscribe an in-process frontend (embedders). Returns its connection id.
    pub fn subscribe(&mut self, name: &str, sink: Box<dyn EventSink>) -> ConnId {
        let id = self.next_local_conn;
        self.next_local_conn += 1;
        self.register_frontend(id, name, sink);
        id
    }

    pub fn conn_kind(&self, conn: ConnId) -> Option<ConnKind> {
        self.conns.get(&conn).map(|c| c.kind)
    }

    /// Activity describes socket liveness only; it does not renew write authority.
    pub fn note_connection_activity(&mut self, conn: ConnId) {
        if let Some(c) = self.conns.get_mut(&conn) { c.last_activity = Instant::now(); }
    }

    pub fn connection_closed(&mut self, conn: ConnId) {
        let name = self.agent_name(conn);
        if let Some(lease) = self.authority.revoke_if_held_by(conn) {
            self.audit.record(&name, "disconnect", json!({ "revoked": lease.label() }));
            self.broadcast(ServerEvent::ControlRevoked { lease: lease.label(), agent_id: lease.agent_id, reason: RevokeReason::Disconnected });
            self.notify_tools_changed();
        }
        self.cancel_scheduled_if(|s| s.conn == conn, "agent_disconnected");
        self.reject_proposal_if(|p| p.conn == conn, "agent_disconnected");
        for id in self.control_requests.iter().filter(|r| r.conn == conn && r.state == ControlRequestState::Pending).map(|r| r.request_id.clone()).collect::<Vec<_>>() {
            self.finish_control_request(&id, ControlRequestState::Denied);
        }
        for id in self.approvals.pending_for_conn(conn) {
            self.finish_approval(&id, ApprovalState::Denied, "agent_disconnected");
        }
        if let Some(c) = self.conns.remove(&conn) {
            if c.kind == ConnKind::Agent {
                self.audit.record(&name, "disconnect", json!({ "conn": conn }));
            }
        }
    }

    // ----- human path -----------------------------------------------------

    /// Bytes from the human. This is the preemptive takeover path.
    pub fn human_input(&mut self, bytes: &[u8]) {
        if !bytes.is_empty() { self.shell_human_input = true; }
        if self.is_private() {
            if !bytes.is_empty() {
                self.revoke_external();
                self.human_input_pending = !matches!(bytes.last(), Some(b'\r' | b'\n' | 3 | 21));
            }
            if self.process_alive { self.write_pty(bytes); }
            return;
        }
        self.shell_recording_armed |= !bytes.is_empty() && self.shell_recording_ready && self.shell_prompt_confirmed;
        // 1. revoke lease  2. controller = Human   (both inside `revoke`)
        let revoked = self.authority.revoke();
        if revoked.is_some() {
            self.trace("revoke");
        }
        // A scheduled agent execution never survives human input.
        self.cancel_scheduled_if(|_| true, "human_input");
        // Nor does a proposal: the human is typing something else.
        self.reject_proposal_if(|_| true, "human_input");
        // Nor does a pending approval. Its command is already typed on the line, so the
        // human's bytes would be appended to it and a later "approve" would run a line
        // nobody reviewed. Denying clears that line before the human's input arrives.
        if !bytes.is_empty() {
            for id in self.approvals.pending().into_iter().map(|a| a.id.clone()).collect::<Vec<_>>() {
                self.finish_approval(&id, ApprovalState::Denied, "human_input");
            }
        }
        // 3. forward the bytes
        self.write_pty(bytes);
        if let Some(lease) = revoked {
            // 4. tell the agent  5. audit
            let ev = ServerEvent::ControlRevoked { lease: lease.label(), agent_id: lease.agent_id.clone(), reason: RevokeReason::HumanInput };
            self.notify(lease.conn, ev.clone());
            self.trace("event");
            self.broadcast(ev);
            self.notify_tools_changed();
            self.audit
                .record("human", "takeover", json!({ "revoked": lease.label(), "agent": lease.agent_id }));
            self.trace("audit");
        }
        // Human bytes may be a password, an editor buffer, or a shell command.
        // Retain only whether a line may be unfinished, never its contents.
        if !bytes.is_empty() {
            self.input.reset();
            self.human_input_pending = !matches!(bytes.last(), Some(b'\r' | b'\n' | 3 | 21));
        }
    }

    /// Revoke the agent lease without typing anything (`conn take`, frontend button).
    pub fn human_take(&mut self) -> Option<String> {
        self.revoke_external();
        self.cancel_scheduled_if(|_| true, "taken");
        let lease = self.authority.revoke()?;
        let ev = ServerEvent::ControlRevoked { lease: lease.label(), agent_id: lease.agent_id.clone(), reason: RevokeReason::Taken };
        self.notify(lease.conn, ev.clone());
        self.broadcast(ev);
        self.notify_tools_changed();
        self.audit.record("human", "takeover", json!({ "revoked": lease.label(), "agent": lease.agent_id, "via": "cli" }));
        Some(lease.label())
    }

    /// Output from the PTY, passed through untouched.
    pub fn pty_output(&mut self, bytes: &[u8]) {
        if bytes.is_empty() { return; }
        self.output_seq = self.output_seq.saturating_add(1);
        if self.screen.process(bytes) {
            self.screen_dirty = true;
        }
        self.write_output(bytes);
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.screen.resize(rows, cols);
        self.screen_dirty = true;
        if let Some(m) = &self.master {
            let _ = m.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
        }
    }

    pub fn process_exited(&mut self, exit_code: Option<u32>) {
        self.finish_shell_command(None);
        self.revoke_external();
        self.process_alive = false;
        self.cancel_scheduled_if(|_| true, "process_exited");
        if let Some(lease) = self.authority.revoke() {
            let ev = ServerEvent::ControlRevoked { lease: lease.label(), agent_id: lease.agent_id, reason: RevokeReason::ProcessExited };
            self.notify(lease.conn, ev.clone());
            self.broadcast(ev);
        }
        for a in self.approvals.pending().iter().map(|a| a.id.clone()).collect::<Vec<_>>() {
            self.finish_approval(&a, ApprovalState::Denied, "process_exited");
        }
        self.notify_tools_changed();
        self.broadcast(ServerEvent::ProcessExited { exit_code });
        self.audit.record("system", "session_end", json!({ "exitCode": exit_code }));
    }

    pub fn process_alive(&self) -> bool {
        self.process_alive
    }

    // ----- attention ------------------------------------------------------

    pub fn attended(&self) -> bool {
        self.attended
    }


    pub fn effective_mode(&self) -> AgentMode { self.mode }

    pub fn set_attended(&mut self, attended: bool) {
        if self.attended == attended { return; }
        self.attended = attended;
        self.audit.record("human", if attended { "attend" } else { "leave" }, json!({}));
        self.broadcast(ServerEvent::AttentionChanged { attended });
        if attended { self.attention_request = None; }
        self.notify_tools_changed();
    }

    /// Agent: ask the human to look at this session.
    pub fn agent_request_attention(&mut self, conn: ConnId, reason: Option<String>) -> Result<(), SessionError> {
        self.require_participant(conn)?;
        let agent_id = self.agent_name(conn);
        let req = AttentionRequest { agent_id: agent_id.clone(), reason: reason.clone(), at: chrono::Local::now().format("%H:%M:%S").to_string() };
        self.attention_request = Some(req);
        self.audit.record(&agent_id, "attention_requested", json!({ "reason": reason }));
        self.broadcast(ServerEvent::AttentionRequested { agent_id, reason });
        Ok(())
    }

    // ----- tabs -----------------------------------------------------------

    /// The host can open / switch tabs. Set by the hub when an opener is installed.
    pub fn set_tabs_supported(&mut self, on: bool) {
        if self.tabs_supported != on {
            self.tabs_supported = on;
            self.notify_tools_changed();
        }
    }

    /// May this agent do `a` right now? Mask first, then the state table.
    pub fn agent_can(&self, conn: ConnId, a: Affordance) -> Result<(), SessionError> {
        self.require_participant(conn)?;
        self.check_mask(a)?;
        if self.affordances(Actor::Agent { conn }).contains(&a) {
            return Ok(());
        }
        Err(SessionError::NotAvailable(a.name().into()))
    }

    /// Destination authorization for explicit connection navigation. Independent
    /// of shell liveness and owner attention; it grants neither reads nor writes.
    pub fn agent_can_navigate(&self, conn: ConnId) -> Result<(), SessionError> {
        // The public transport has authenticated the connection; destination
        // registration is intentionally lazy and happens after this ACL check.
        if !self.participant_allowed(conn) {
            return Err(SessionError::NotAvailable("participation not granted".into()));
        }
        self.check_mask(Affordance::SwitchTab)?;
        if !self.tabs_supported || self.mode == AgentMode::Observe {
            return Err(SessionError::NotAvailable("switch_tab".into()));
        }
        Ok(())
    }

    /// A live, participating source may pin its agents: an owner mask without
    /// `switch_tab` holds. A source that is gone, private, exited or no longer
    /// open to `conn` never blocks recovery.
    pub fn blocks_navigation_from(&self, conn: ConnId) -> bool {
        self.process_alive && self.participant_allowed(conn) && self.check_mask(Affordance::SwitchTab).is_err()
    }

    /// `conn` moved its binding elsewhere. Like a lock, the lease belongs to the
    /// tab: nothing stays held or queued here by an agent that has left.
    pub fn agent_left_tab(&mut self, conn: ConnId) {
        if self.is_private() { return; }
        self.cancel_scheduled_if(|s| s.conn == conn, "agent_left_tab");
        self.reject_proposal_if(|p| p.conn == conn, "agent_left_tab");
        for id in self.control_requests.iter().filter(|r| r.conn == conn && r.state == ControlRequestState::Pending).map(|r| r.request_id.clone()).collect::<Vec<_>>() {
            self.finish_control_request(&id, ControlRequestState::Denied);
        }
        for id in self.approvals.pending_for_conn(conn) {
            self.finish_approval(&id, ApprovalState::Denied, "agent_left_tab");
        }
        if let Some(lease) = self.authority.revoke_if_held_by(conn) {
            self.audit.record(&lease.agent_id, "lease_released", json!({ "lease": lease.label(), "reason": "left_tab" }));
            self.broadcast(ServerEvent::ControlRevoked { lease: lease.label(), agent_id: lease.agent_id, reason: RevokeReason::Released });
            self.notify_tools_changed();
        }
    }

    /// This session was just opened as a tab by `conn`. It starts unattended and
    /// knocks with the agent's reason so the human can decide to look.
    pub fn agent_opened_tab(&mut self, conn: ConnId, reason: Option<String>) {
        if self.is_private() { return; }
        let agent_id = self.agent_name(conn);
        self.opened_by = Some(agent_id.clone());
        self.attention_request = Some(AttentionRequest { agent_id: agent_id.clone(), reason: reason.clone(), at: chrono::Local::now().format("%H:%M:%S").to_string() });
        self.audit.record(&agent_id, "tab_opened", json!({ "reason": reason }));
        self.broadcast(ServerEvent::TabOpened { agent_id, reason });
    }

    /// Tell this session's listeners that `conn` moved between tabs.
    pub fn agent_switched_tab(&mut self, conn: ConnId, from: &str, to: &str) {
        if self.is_private() { return; }
        let agent_id = self.agent_name(conn);
        self.audit.record(&agent_id, "tab_switched", json!({ "from": from, "to": to }));
        self.broadcast(ServerEvent::AgentSwitchedTab { agent_id, from: from.into(), to: to.into() });
    }

    // ----- frontend controls ---------------------------------------------

    pub fn pacing(&self) -> &Pacing {
        &self.pacing
    }

    /// Set lease / approval TTLs directly (sub-second granularity; used by tests).
    pub fn set_ttls(&mut self, lease: Duration, approval: Duration) {
        self.authority.set_ttl(lease);
        self.approvals.set_ttl(approval);
    }

    pub fn set_pacing(&mut self, pacing: Pacing) {
        self.authority.set_ttl(Duration::from_secs(pacing.lease_ttl_secs));
        self.approvals.set_ttl(Duration::from_secs(pacing.approval_ttl_secs));
        self.pacing = pacing.clone();
        self.audit.record("frontend", "pacing_changed", serde_json::to_value(&pacing).unwrap());
        self.broadcast(ServerEvent::PacingChanged { pacing });
    }

    /// Restrict what agents may do, on top of the state-derived table. `None` = no
    /// restriction. An empty set makes agents observers only if it contains `Snapshot`.
    pub fn set_affordance_mask(&mut self, allow: Option<HashSet<Affordance>>) {
        let list = allow.as_ref().map(|s| {
            let mut v: Vec<_> = s.iter().copied().collect();
            v.sort_by_key(|a| a.name());
            v
        });
        self.affordance_mask = allow;
        // A running agent may have just lost its write tools.
        if let Some(mask) = &self.affordance_mask {
            if !mask.contains(&Affordance::Type) && !mask.contains(&Affordance::SendKey) {
                self.cancel_scheduled_if(|_| true, "masked");
            }
        }
        self.audit.record("frontend", "affordance_mask", json!({ "allow": list }));
        self.broadcast(ServerEvent::AffordanceMaskChanged { allow: list });
        self.notify_tools_changed();
    }

    pub fn affordance_mask(&self) -> Option<Vec<Affordance>> {
        self.affordance_mask.as_ref().map(|s| {
            let mut v: Vec<_> = s.iter().copied().collect();
            v.sort_by_key(|a| a.name());
            v
        })
    }

    // ----- agent path -----------------------------------------------------

    /// Every connected agent with what it may do right now.
    pub fn agent_affordances(&self) -> Vec<(ConnId, String, Vec<Affordance>)> {
        let mut out: Vec<_> = self
            .conns
            .iter()
            .filter(|(_, c)| c.kind == ConnKind::Agent)
            .map(|(conn, c)| (*conn, c.name.clone(), self.affordances(Actor::Agent { conn: *conn })))
            .collect();
        out.sort_by_key(|(c, _, _)| *c);
        out
    }

    pub fn policy_description(&self) -> serde_json::Value {
        self.policy.policy().describe()
    }

    pub fn affordances(&self, actor: Actor) -> Vec<Affordance> {
        if let Actor::Agent { conn } = actor {
            if !self.participant_allowed(conn) { return vec![]; }
        }
        let has_pending = match actor {
            Actor::Agent { conn } => self.approvals.has_pending_for(conn),
            Actor::Human => false,
        };
        let state = AffordanceState {
            controller: self.authority.controller(),
            process_alive: self.process_alive,
            has_pending_approval: has_pending,
            any_pending_approval: self.approvals.first_pending().is_some(),
            attended: self.attended,
            tabs: self.tabs_supported,
        };
        let mut out = affordances_for(actor, &state);
        if let (Actor::Agent { .. }, Some(mask)) = (actor, &self.affordance_mask) {
            out.retain(|a| mask.contains(a));
        }
        if matches!(actor, Actor::Agent { .. }) && self.mode == AgentMode::Observe {
            out.retain(|a| *a == Affordance::Snapshot);
        }
        out
    }

    fn check_mask(&self, a: Affordance) -> Result<(), SessionError> {
        match &self.affordance_mask {
            Some(m) if !m.contains(&a) => Err(SessionError::Masked(a.name().into())),
            _ => Ok(()),
        }
    }

    pub fn snapshot(&self, actor: Actor) -> Result<Snapshot, SessionError> {
        if let Actor::Agent { conn } = actor {
            self.require_participant(conn)?;
            self.check_mask(Affordance::Snapshot)?;
        }
        let frame = self.authoritative_surface()?;
        let projection = frame.projection();
        if let Actor::Agent { conn } = actor {
            self.audit.record(&self.agent_name(conn), "observe", json!({ "rev": projection.revision }));
        }
        Ok(Snapshot { mode: self.mode, effective_mode: self.effective_mode(), projection, controller: self.controller_info(), process_alive: self.process_alive })
    }

    /// `request_control`. With the control gate on, the request is held for the human
    /// (`Pending`); otherwise it is granted immediately. The submitted parameters are
    /// preserved before any human decision. Metadata does not authorize execution.
    pub fn agent_request_control_original(&mut self, conn: ConnId, params: serde_json::Value) -> Result<ControlOutcome, SessionError> {
        self.require_participant(conn)?;
        if !params.is_object() || params.to_string().len() > 65536 {
            return Err(SessionError::InvalidInput("request parameters must be an object of at most 64 KiB".into()));
        }
        for key in ["reason", "command"] {
            if params.get(key).is_some_and(|v| !v.is_null() && !v.is_string()) {
                return Err(SessionError::InvalidInput(format!("{key} must be a string")));
            }
        }
        let reason = params.get("reason").and_then(|v| v.as_str()).filter(|v| !v.trim().is_empty()).map(str::to_owned);
        let original_request = json!({ "method": "request_control", "params": params });
        if !self.process_alive {
            return Err(SessionError::ProcessExited);
        }
        self.check_mask(Affordance::RequestControl)?;
        self.require_shared()?;
        if self.mode == AgentMode::Observe {
            return Err(SessionError::WrongMode(self.mode));
        }
        let agent_id = self.agent_name(conn);
        if self.control_gate && !self.authority.holds(conn) {
            if let Some(r) = self.control_requests.iter().find(|r| r.conn == conn && r.state == ControlRequestState::Pending) {
                if params_command_changed(&r.original_request, &original_request) {
                    return Err(SessionError::InvalidInput("a different control request is already pending".into()));
                }
                return Ok(ControlOutcome::Pending { request_id: r.request_id.clone() });
            }
            if let Controller::Agent(l) = self.authority.controller() {
                if l.expires_at > Instant::now() {
                    return Err(AuthorityError::Busy { agent_id: l.agent_id.clone() }.into());
                }
            }
            self.control_seq += 1;
            let req = ControlRequest {
                request_id: format!("ctl-{}", self.control_seq),
                conn,
                agent_id: agent_id.clone(),
                reason: reason.clone(),
                original_request: original_request.clone(),
                state: ControlRequestState::Pending,
                created: Instant::now(),
            };
            self.control_requests.push(req.clone());
            self.audit.record(&agent_id, "control_requested", json!({ "request": req.request_id, "reason": reason, "originalRequest": original_request }));
            self.broadcast(ServerEvent::ControlRequested { request: req.clone() });
            return Ok(ControlOutcome::Pending { request_id: req.request_id });
        }
        Ok(ControlOutcome::Granted { lease: self.grant_original(conn, reason, "request", Some(original_request))? })
    }

    fn grant_original(&mut self, conn: ConnId, reason: Option<String>, via: &str, original_request: Option<serde_json::Value>) -> Result<LeaseInfo, SessionError> {
        self.require_participant(conn)?;
        self.require_shared()?;
        let agent_id = self.agent_name(conn);
        let lease = self.authority.request(&agent_id, conn, Instant::now())?;
        self.last_agent = Some((conn, agent_id.clone()));
        self.audit.record(&agent_id, "lease_granted", json!({ "lease": lease.label(), "reason": reason, "via": via, "originalRequest": original_request }));
        self.broadcast(ServerEvent::ControlGranted { lease: lease.label(), agent_id: agent_id.clone(), reason, original_request });
        self.notify_tools_changed();
        Ok(LeaseInfo { lease_id: lease.label(), agent_id, ttl_secs: self.authority.ttl().as_secs() })
    }

    /// Human decision on a gated control request.
    pub fn decide_control(&mut self, request_id: &str, grant: bool) -> Result<ControlRequest, SessionError> {
        let Some(req) = self.control_requests.iter().find(|r| r.request_id == request_id).cloned() else {
            return Err(SessionError::NotFound(request_id.to_string()));
        };
        if req.state != ControlRequestState::Pending {
            return Err(SessionError::InvalidInput(format!("{request_id} is not pending")));
        }
        if grant {
            self.grant_original(req.conn, req.reason.clone(), "gate", Some(req.original_request.clone()))?;
        }
        Ok(self.finish_control_request(request_id, if grant { ControlRequestState::Granted } else { ControlRequestState::Denied }).unwrap())
    }

    fn finish_control_request(&mut self, request_id: &str, state: ControlRequestState) -> Option<ControlRequest> {
        let r = self.control_requests.iter_mut().find(|r| r.request_id == request_id && r.state == ControlRequestState::Pending)?;
        r.state = state;
        let r = r.clone();
        self.audit.record(&r.agent_id, "control_request_resolved", json!({ "request": r.request_id, "state": state, "reason": r.reason, "originalRequest": r.original_request }));
        self.notify(r.conn, ServerEvent::ControlRequestResolved { request_id: r.request_id.clone(), state });
        self.broadcast(ServerEvent::ControlRequestResolved { request_id: r.request_id.clone(), state });
        Some(r)
    }

    pub fn control_request_state(&self, request_id: &str) -> Option<ControlRequestState> {
        self.control_requests.iter().find(|r| r.request_id == request_id).map(|r| r.state)
    }

    /// Give control back to the agent that last held it (human action).
    pub fn hand_back(&mut self) -> Result<LeaseInfo, SessionError> {
        let (conn, agent_id) = self.last_agent.clone().ok_or_else(|| SessionError::NotFound("no previous agent".into()))?;
        if !self.conns.contains_key(&conn) {
            return Err(SessionError::NotFound(format!("{agent_id} is no longer connected")));
        }
        if self.mode == AgentMode::Observe {
            return Err(SessionError::WrongMode(self.mode));
        }
        let info = self.grant_original(conn, Some("hand_back".into()), "hand_back", None)?;
        let ev = ServerEvent::ControlHandedBack { lease: info.lease_id.clone(), agent_id: agent_id.clone(), last_cmd: self.last_agent_cmd.clone() };
        self.notify(conn, ev.clone());
        self.broadcast(ev);
        self.audit.record("human", "hand_back", json!({ "agent": agent_id, "lease": info.lease_id }));
        Ok(info)
    }

    pub fn set_mode(&mut self, mode: AgentMode) -> Result<(), SessionError> {
        if self.mode == mode { return Ok(()); }
        // A mode switch must not leave physical shell input behind a ghost proposal.
        // Refuse rather than assume Ctrl-U has the same meaning in every shell/TUI.
        if self.human_input_pending || self.input.has_pending() { return Err(SessionError::InputPending); }
        self.mode = mode;
        match mode {
            AgentMode::Observe => {
                self.reject_proposal_if(|_| true, "mode_changed");
                self.cancel_scheduled_if(|_| true, "mode_changed");
                if let Some(lease) = self.authority.revoke() {
                    let ev = ServerEvent::ControlRevoked { lease: lease.label(), agent_id: lease.agent_id, reason: RevokeReason::Taken };
                    self.notify(lease.conn, ev.clone());
                    self.broadcast(ev);
                }
                for id in self.control_requests.iter().filter(|r| r.state == ControlRequestState::Pending).map(|r| r.request_id.clone()).collect::<Vec<_>>() {
                    self.finish_control_request(&id, ControlRequestState::Denied);
                }
            }
            AgentMode::Copilot => {
                self.cancel_scheduled_if(|_| true, "mode_changed");
            }
            AgentMode::Autopilot => {
                self.reject_proposal_if(|_| true, "mode_changed");
            }
        }
        self.audit.record("frontend", "mode_changed", json!({ "mode": mode }));
        self.notify_mode_changed();
        self.notify_tools_changed();
        Ok(())
    }

    fn notify_mode_changed(&self) {
        let ev = ServerEvent::ModeChanged { mode: self.mode, effective_mode: self.effective_mode() };
        self.broadcast(ev.clone());
        for c in self.conns.values().filter(|c| c.kind == ConnKind::Agent) { c.sink.send_scoped(ev.clone(), self.participation_generation); }
    }

    pub fn mode(&self) -> AgentMode {
        self.mode
    }

    pub fn set_control_gate(&mut self, ask: bool) {
        self.control_gate = ask;
        self.audit.record("frontend", "control_gate", json!({ "ask": ask }));
        self.broadcast(ServerEvent::ControlGateChanged { ask });
    }

    /// Full structural analysis of a line against the current policy and shell cwd.
    pub fn analyse_line(&self, cmd: &str) -> crate::policy::LineAnalysis {
        self.analyse(cmd)
    }

    pub fn policy_path(&self) -> Option<std::path::PathBuf> {
        self.policy.path().map(|p| p.to_path_buf())
    }

    pub fn revoke_session_allow(&mut self, label: &str) -> bool {
        let removed = self.policy.disallow_for_session(label);
        if removed {
            self.audit.record("human", "session_allow_revoked", json!({ "label": label }));
            self.broadcast(ServerEvent::SessionAllowsChanged { allows: self.policy.session_allows() });
        }
        removed
    }

    // ----- copilot proposals ---------------------------------------------

    fn proposal_mut(&mut self, conn: ConnId) -> &mut Proposal {
        let needs_new = !matches!(&self.proposal, Some(p) if p.conn == conn && p.state == ProposalState::Drafting);
        if needs_new {
            self.reject_proposal_if(|_| true, "superseded");
            self.proposal_seq += 1;
            self.proposal = Some(Proposal {
                proposal_id: format!("prop-{}", self.proposal_seq),
                conn,
                agent_id: self.agent_name(conn),
                text: String::new(),
                state: ProposalState::Drafting,
                intent: None,
            });
        }
        self.proposal.as_mut().unwrap()
    }

    fn broadcast_proposal(&self) {
        if let Some(p) = &self.proposal {
            self.broadcast(ServerEvent::ProposalChanged { proposal: p.clone() });
        }
    }

    fn reject_proposal_if(&mut self, pred: impl Fn(&Proposal) -> bool, reason: &str) {
        if let Some(p) = &mut self.proposal {
            if matches!(p.state, ProposalState::Drafting | ProposalState::Ready) && pred(p) {
                p.state = ProposalState::Rejected;
                let (id, agent, cmd, conn) = (p.proposal_id.clone(), p.agent_id.clone(), p.text.clone(), p.conn);
                self.audit.record(&agent, "proposal_rejected", json!({ "proposal": id, "cmd": cmd, "reason": reason }));
                let ev = ServerEvent::ProposalResolved { proposal_id: id, state: ProposalState::Rejected, cmd, policy: None };
                self.notify(conn, ev.clone());
                self.broadcast(ev);
            }
        }
    }

    /// Human commits the proposed line: it is typed into the shell and ENTER runs
    /// the policy. `confirm` is treated as granted — the human just read and committed
    /// it — and recorded as such; `deny` still blocks.
    pub fn accept_proposal(&mut self, proposal_id: &str) -> Result<KeyResult, SessionError> {
        self.require_shared()?;
        let Some(p) = self.proposal.clone() else { return Err(SessionError::NotFound(proposal_id.into())) };
        if p.proposal_id != proposal_id {
            return Err(SessionError::NotFound(proposal_id.into()));
        }
        if p.state != ProposalState::Ready {
            return Err(SessionError::InvalidInput(format!("{proposal_id} is {:?}", p.state)));
        }
        // Attention changes can impose a Co-pilot cap without an explicit mode
        // switch. Never append a proposal to pre-existing physical shell input.
        if self.human_input_pending || self.input.has_pending() { return Err(SessionError::InputPending); }
        let cmd = p.text.trim().to_string();
        let col = self.screen.cursor().col;
        self.input.feed(p.text.as_bytes(), col);
        self.write_pty(p.text.as_bytes());
        let decision = self.analyse(&cmd).decision;
        let (state, result, policy) = match decision {
            PolicyDecision::Deny { label } => {
                self.write_pty(b"\x15");
                self.input.reset();
                (ProposalState::Denied, KeyResult::Denied { cmd: cmd.clone(), label: label.clone() }, format!("deny:{label}"))
            }
            PolicyDecision::Allow => {
                self.write_pty(b"\r");
                self.input.reset();
                (ProposalState::Executed, KeyResult::Executed { cmd: cmd.clone() }, "allow".into())
            }
            PolicyDecision::Confirm { label } => {
                self.write_pty(b"\r");
                self.input.reset();
                (ProposalState::Executed, KeyResult::Executed { cmd: cmd.clone() }, format!("confirm:{label}"))
            }
        };
        if let Some(pp) = &mut self.proposal {
            pp.state = state;
        }
        let submission_id = if state == ProposalState::Executed { self.shell_submission(&p.agent_id, &cmd) } else { None };
        self.last_agent_cmd = Some(cmd.clone());
        self.audit.record(&p.agent_id, "exec", json!({ "cmd": cmd, "policy": policy, "by": "human_commit", "proposal": p.proposal_id, "intent": p.intent, "submissionId": submission_id }));
        let ev = ServerEvent::ProposalResolved { proposal_id: p.proposal_id.clone(), state, cmd: cmd.clone(), policy: Some(policy.clone()) };
        self.notify(p.conn, ev.clone());
        self.broadcast(ev);
        self.broadcast(ServerEvent::AgentExec { agent_id: p.agent_id, cmd, policy, intent: p.intent, submission_id });
        Ok(result)
    }

    pub fn reject_proposal(&mut self, proposal_id: &str) -> Result<(), SessionError> {
        match &self.proposal {
            Some(p) if p.proposal_id == proposal_id && matches!(p.state, ProposalState::Drafting | ProposalState::Ready) => {
                self.reject_proposal_if(|_| true, "rejected");
                Ok(())
            }
            Some(p) if p.proposal_id == proposal_id => Err(SessionError::InvalidInput(format!("{proposal_id} is {:?}", p.state))),
            _ => Err(SessionError::NotFound(proposal_id.into())),
        }
    }

    pub fn proposal_state(&self, proposal_id: &str) -> Option<ProposalState> {
        self.proposal.as_ref().filter(|p| p.proposal_id == proposal_id).map(|p| p.state)
    }

    pub fn proposal(&self) -> Option<&Proposal> {
        self.proposal.as_ref().filter(|p| matches!(p.state, ProposalState::Drafting | ProposalState::Ready))
    }

    pub fn agent_release_control(&mut self, conn: ConnId) -> Result<(), SessionError> {
        self.require_participant(conn)?;
        let lease = self.authority.revoke_if_held_by(conn).ok_or(AuthorityError::NotController)?;
        self.cancel_scheduled_if(|s| s.conn == conn, "released");
        self.audit.record(&lease.agent_id, "lease_released", json!({ "lease": lease.label() }));
        self.broadcast(ServerEvent::ControlRevoked { lease: lease.label(), agent_id: lease.agent_id, reason: RevokeReason::Released });
        self.notify_tools_changed();
        Ok(())
    }

    fn check_writer(&mut self, conn: ConnId, a: Affordance) -> Result<(), SessionError> {
        self.require_participant(conn)?;
        if !self.process_alive {
            return Err(SessionError::ProcessExited);
        }
        self.check_mask(a)?;
        match self.authority.check_and_touch(conn, Instant::now()) {
            Ok(()) => {}
            Err(AuthorityError::Expired) => {
                let name = self.agent_name(conn);
                self.audit.record(&name, "lease_expired", json!({}));
                self.notify_tools_changed();
                return Err(AuthorityError::Expired.into());
            }
            Err(e) => return Err(e.into()),
        }
        self.require_shared()?;
        if let Some(id) = self.approvals.pending_for_conn(conn).first() {
            return Err(SessionError::ApprovalPending(id.clone()));
        }
        if let Some(s) = &self.scheduled {
            if s.state == ExecState::Scheduled {
                return Err(SessionError::ExecPending(s.exec_id.clone()));
            }
        }
        if let Some(p) = &self.proposal {
            if p.state == ProposalState::Ready {
                return Err(SessionError::ProposalPending(p.proposal_id.clone()));
            }
        }
        if self.pacing.min_write_interval_ms > 0 {
            if let Some(last) = self.last_agent_write {
                let min = Duration::from_millis(self.pacing.min_write_interval_ms);
                let elapsed = last.elapsed();
                if elapsed < min {
                    return Err(SessionError::RateLimited { retry_after_ms: (min - elapsed).as_millis() as u64 });
                }
            }
        }
        Ok(())
    }

    fn note_agent_write(&mut self, conn: ConnId, len: usize) {
        self.shell_agent_input = true;
        self.last_agent_write = Some(Instant::now());
        self.broadcast(ServerEvent::AgentInput { agent_id: self.agent_name(conn), len });
    }

    /// Append text to the input line. Never executes.
    pub fn agent_type(&mut self, conn: ConnId, text: &str) -> Result<(), SessionError> {
        if text.chars().any(|c| c == '\r' || c == '\n') {
            return Err(SessionError::InvalidInput(
                "text must not contain newlines; use send_key(ENTER) to execute".into(),
            ));
        }
        if text.chars().any(|c| c.is_control() && c != '\t') {
            return Err(SessionError::InvalidInput("text must not contain control characters".into()));
        }
        self.check_writer(conn, Affordance::Type)?;
        if self.human_input_pending { return Err(SessionError::InputPending); }
        if self.effective_mode() == AgentMode::Copilot {
            self.proposal_mut(conn).text.push_str(text);
            self.last_agent_write = Some(Instant::now());
            self.broadcast_proposal();
            return Ok(());
        }
        self.shell_recording_armed |= self.shell_recording_ready && self.shell_prompt_confirmed;
        let col = self.screen.cursor().col;
        self.input.feed(text.as_bytes(), col);
        self.write_pty(text.as_bytes());
        self.note_agent_write(conn, text.len());
        Ok(())
    }

    /// Send a named key. ENTER runs the policy check and carries the agent's stated
    /// intent (required when the policy says so). The intent is shown to the human
    /// and recorded in the audit log.
    pub fn agent_send_key_with(&mut self, conn: ConnId, key: &str, intent: Option<String>) -> Result<KeyResult, SessionError> {
        let intent = intent.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let bytes = key_bytes(key).ok_or_else(|| {
            SessionError::InvalidInput(format!("unsupported key '{key}'; supported: {}", SUPPORTED_KEYS.join(" ")))
        })?;
        if key.eq_ignore_ascii_case("CTRL_C") {
            return self.agent_interrupt(conn).map(|_| KeyResult::Sent);
        }
        self.check_writer(conn, Affordance::SendKey)?;
        if self.human_input_pending { return Err(SessionError::InputPending); }
        if self.effective_mode() == AgentMode::Observe {
            return Err(SessionError::WrongMode(AgentMode::Observe));
        }
        if self.effective_mode() == AgentMode::Copilot {
            return match key.to_ascii_uppercase().as_str() {
                "BACKSPACE" => {
                    self.proposal_mut(conn).text.pop();
                    self.broadcast_proposal();
                    Ok(KeyResult::Sent)
                }
                "ENTER" => {
                    if intent.is_none() && self.policy.require_intent() {
                        return Err(SessionError::IntentRequired);
                    }
                    let p = self.proposal_mut(conn);
                    if p.text.trim().is_empty() {
                        return Err(SessionError::InvalidInput("nothing proposed; type first".into()));
                    }
                    p.state = ProposalState::Ready;
                    p.intent = intent.clone();
                    let (id, cmd, agent) = (p.proposal_id.clone(), p.text.trim().to_string(), p.agent_id.clone());
                    self.last_agent_write = Some(Instant::now());
                    self.audit.record(&agent, "proposal", json!({ "proposal": id, "cmd": cmd, "intent": intent }));
                    self.broadcast_proposal();
                    Ok(KeyResult::Proposed { proposal_id: id, cmd })
                }
                _ => Err(SessionError::InvalidInput("copilot mode: only ENTER and BACKSPACE (and CTRL_C) are meaningful".into())),
            };
        }
        if !key.eq_ignore_ascii_case("ENTER") {
            let col = self.screen.cursor().col;
            self.input.feed(bytes, col);
            self.write_pty(bytes);
            self.note_agent_write(conn, bytes.len());
            return Ok(KeyResult::Sent);
        }
        if intent.is_none() && self.policy.require_intent() {
            return Err(SessionError::IntentRequired);
        }
        // The resolved command is echoed back to the agent and may come from the
        // raw cursor row. An agent never submits a line it cannot fully observe.
        if self.screen.cursor_line_hidden() {
            return Err(SessionError::InvalidInput("the cursor line contains concealed text; only the human can submit it".into()));
        }
        let agent_id = self.agent_name(conn);
        // The cursor row is only a visual fragment when a command wraps (or
        // scrolls beyond the screen). Keep the full tracked input authoritative
        // unless completion/history/cursor editing made it unreliable.
        let cmd = input::resolve_command(
            &self.input.line(),
            self.input.is_dirty(),
            &self.screen.cursor_line(),
            self.input.prompt_col(),
        );
        self.last_agent_write = Some(Instant::now());
        let analysis = self.analyse(&cmd);
        match analysis.decision.clone() {
            PolicyDecision::Allow => {
                if self.pacing.enter_grace_ms > 0 {
                    self.exec_seq += 1;
                    let exec_id = format!("exec-{}", self.exec_seq);
                    let grace_ms = self.pacing.enter_grace_ms;
                    self.scheduled = Some(ScheduledExec {
                        exec_id: exec_id.clone(),
                        conn,
                        agent_id: agent_id.clone(),
                        cmd: cmd.clone(),
                        intent: intent.clone(),
                        due: Instant::now() + Duration::from_millis(grace_ms),
                        state: ExecState::Scheduled,
                        cancel_reason: None,
                    });
                    self.audit.record(&agent_id, "exec_scheduled", json!({ "cmd": cmd, "exec": exec_id, "graceMs": grace_ms, "intent": intent }));
                    self.broadcast(ServerEvent::ExecScheduled { exec_id: exec_id.clone(), agent_id: agent_id.clone(), cmd: cmd.clone(), intent, grace_ms });
                    // Grace is the human's chance to object; a tab nobody is looking at must knock.
                    if !self.attended {
                        let reason = Some(format!("grace: {cmd}"));
                        self.attention_request = Some(AttentionRequest { agent_id: agent_id.clone(), reason: reason.clone(), at: chrono::Local::now().format("%H:%M:%S").to_string() });
                        self.broadcast(ServerEvent::AttentionRequested { agent_id, reason });
                    }
                    return Ok(KeyResult::Scheduled { exec_id, cmd, grace_ms });
                }
                self.execute_with_source(&agent_id, &cmd, intent, None);
                Ok(KeyResult::Executed { cmd })
            }
            PolicyDecision::Deny { label } => {
                self.write_pty(b"\x15");
                self.input.reset();
                self.last_agent_cmd = Some(cmd.clone());
                self.audit.record(&agent_id, "exec", json!({ "cmd": cmd, "policy": "deny", "label": label, "intent": intent, "isolation": analysis.isolation_violation }));
                self.broadcast(ServerEvent::AgentExec { agent_id, cmd: cmd.clone(), policy: format!("deny:{label}"), intent, submission_id: None });
                Ok(KeyResult::Denied { cmd, label })
            }
            PolicyDecision::Confirm { label } => {
                self.last_agent_cmd = Some(cmd.clone());
                let req = self.approvals.create_with(&agent_id, conn, &cmd, &label, Instant::now(), intent.clone(), Some(analysis)).clone();
                self.audit.record(
                    &agent_id,
                    "approval_requested",
                    json!({ "approval": req.id, "cmd": cmd, "label": label, "intent": intent }),
                );
                self.broadcast(ServerEvent::ApprovalRequested { request: req.clone() });
                self.notify_tools_changed();
                Ok(KeyResult::Pending { approval_id: req.id, cmd, label })
            }
        }
    }

    fn execute_with_source(&mut self, agent_id: &str, cmd: &str, intent: Option<String>, by: Option<&str>) {
        let submission_id = self.shell_submission(agent_id, cmd);
        self.last_agent_cmd = Some(cmd.to_string());
        self.write_pty(b"\r");
        self.input.reset();
        self.audit.record(agent_id, "exec", json!({ "cmd": cmd, "policy": "allow", "intent": intent, "by": by, "submissionId": submission_id }));
        self.broadcast(ServerEvent::AgentExec { agent_id: agent_id.to_string(), cmd: cmd.to_string(), policy: "allow".into(), intent, submission_id });
    }

    fn cancel_scheduled_if(&mut self, pred: impl Fn(&ScheduledExec) -> bool, reason: &str) {
        if let Some(s) = &mut self.scheduled {
            if s.state == ExecState::Scheduled && pred(s) {
                s.state = ExecState::Cancelled;
                s.cancel_reason = Some(reason.to_string());
                let (id, agent, cmd) = (s.exec_id.clone(), s.agent_id.clone(), s.cmd.clone());
                // Cancellation leaves bytes in the shell, so retain their tracking.
                self.audit.record(&agent, "exec_cancelled", json!({ "exec": id, "cmd": cmd, "reason": reason }));
                self.broadcast(ServerEvent::ExecCancelled { exec_id: id, reason: reason.into() });
            }
        }
    }

    /// Frontend/human: run a scheduled execution right now (co-sign during the grace window).
    pub fn execute_now_scheduled(&mut self, exec_id: &str) -> Result<(), SessionError> {
        self.require_shared()?;
        match &mut self.scheduled {
            Some(s) if s.exec_id == exec_id && s.state == ExecState::Scheduled => {
                s.state = ExecState::Executed;
                let (agent, cmd, intent) = (s.agent_id.clone(), s.cmd.clone(), s.intent.clone());
                self.audit.record("human", "exec_cosigned", json!({ "exec": exec_id, "cmd": cmd, "agentId": agent }));
                self.broadcast(ServerEvent::ExecCosigned { exec_id: exec_id.into(), agent_id: agent.clone() });
                self.execute_with_source(&agent, &cmd, intent, Some("human_cosign"));
                Ok(())
            }
            Some(s) if s.exec_id == exec_id => Err(SessionError::InvalidInput(format!("{exec_id} is not scheduled"))),
            _ => Err(SessionError::NotFound(exec_id.to_string())),
        }
    }

    /// Frontend/human: cancel a scheduled execution during its grace window.
    /// The typed line is left for the human to inspect; Ctrl-U it if unwanted.
    pub fn cancel_exec(&mut self, exec_id: &str) -> Result<(), SessionError> {
        match &self.scheduled {
            Some(s) if s.exec_id == exec_id && s.state == ExecState::Scheduled => {
                self.cancel_scheduled_if(|_| true, "cancelled");
                Ok(())
            }
            Some(s) if s.exec_id == exec_id => Err(SessionError::InvalidInput(format!("{exec_id} is not scheduled"))),
            _ => Err(SessionError::NotFound(exec_id.to_string())),
        }
    }

    pub fn exec_state(&self, exec_id: &str) -> Option<(ExecState, Option<String>)> {
        self.scheduled.as_ref().filter(|s| s.exec_id == exec_id).map(|s| (s.state, s.cancel_reason.clone()))
    }

    pub fn scheduled_exec(&self) -> Option<&ScheduledExec> {
        self.scheduled.as_ref().filter(|s| s.state == ExecState::Scheduled)
    }

    /// Ctrl-C. Cancels any pending approval / scheduled execution from this agent.
    pub fn agent_interrupt(&mut self, conn: ConnId) -> Result<(), SessionError> {
        self.require_participant(conn)?;
        if !self.process_alive {
            return Err(SessionError::ProcessExited);
        }
        self.check_mask(Affordance::Interrupt)?;
        self.require_shared()?;
        self.authority.check_and_touch(conn, Instant::now())?;
        self.cancel_scheduled_if(|s| s.conn == conn, "interrupted");
        for id in self.approvals.pending_for_conn(conn) {
            self.finish_approval(&id, ApprovalState::Denied, "interrupted");
        }
        self.reject_proposal_if(|p| p.conn == conn, "interrupted");
        self.write_pty(b"\x03");
        self.input.reset();
        self.human_input_pending = false;
        let name = self.agent_name(conn);
        self.audit.record(&name, "interrupt", json!({}));
        self.note_agent_write(conn, 1);
        Ok(())
    }

    pub fn owns_request(&self, conn: ConnId, method: &str, id: &str) -> bool {
        if !self.participant_allowed(conn) { return false; }
        match method {
            "check_approval" => self.approvals.get(id).is_some_and(|a| a.conn == conn),
            "control_request_state" => self.control_requests.iter().any(|r| r.request_id == id && r.conn == conn),
            "proposal_state" => self.proposal.as_ref().is_some_and(|p| p.proposal_id == id && p.conn == conn),
            "exec_state" => self.scheduled.as_ref().is_some_and(|e| e.exec_id == id && e.conn == conn),
            _ => false,
        }
    }

    pub fn check_approval(&mut self, id: &str) -> Result<ApprovalInfo, SessionError> {
        let a = self.approvals.get(id).ok_or_else(|| SessionError::NotFound(id.to_string()))?;
        if a.state == ApprovalState::Pending {
            // polling for a decision counts as activity
            self.authority.renew(a.conn, Instant::now());
        }
        Ok(ApprovalInfo { approval_id: a.id.clone(), state: a.state, cmd: a.cmd.clone(), label: a.label.clone() })
    }

    // ----- approval -------------------------------------------------------

    /// Human decision on a pending approval. `by` is recorded in the audit log
    /// (e.g. "cli", "frontend").
    pub fn resolve_approval(&mut self, id: &str, decision: ApprovalDecision, by: &str) -> Result<ApprovalInfo, SessionError> {
        if self.approvals.get(id).is_none() {
            return Err(SessionError::NotFound(id.to_string()));
        }
        if decision == ApprovalDecision::AllowSession && self.review_required() {
            return Err(SessionError::InvalidInput("this shell or remote environment requires review for every command; allow_session is unavailable".into()));
        }
        if decision != ApprovalDecision::Deny {
            self.require_shared()?;
        }
        let state = match decision {
            ApprovalDecision::Grant | ApprovalDecision::AllowSession => ApprovalState::Granted,
            ApprovalDecision::Deny => ApprovalState::Denied,
        };
        let req = self
            .finish_approval(id, state, by)
            .ok_or_else(|| SessionError::InvalidInput(format!("{id} is not pending")))?;
        if decision == ApprovalDecision::AllowSession {
            self.policy.allow_for_session(&req.label);
            self.audit.record("human", "session_allow", json!({ "label": req.label }));
        }
        Ok(ApprovalInfo { approval_id: req.id, state: req.state, cmd: req.cmd, label: req.label })
    }

    /// Resolve the oldest pending approval.
    pub fn resolve_first_pending(&mut self, decision: ApprovalDecision, by: &str) -> Result<ApprovalInfo, SessionError> {
        let id = self
            .approvals
            .first_pending()
            .map(|a| a.id.clone())
            .ok_or_else(|| SessionError::NotFound("no pending approval".into()))?;
        self.resolve_approval(&id, decision, by)
    }

    fn finish_approval(&mut self, id: &str, state: ApprovalState, by: &str) -> Option<ApprovalRequest> {
        let req = self.approvals.resolve(id, state)?;
        let outcome = match state {
            ApprovalState::Granted => {
                self.write_pty(b"\r");
                "granted"
            }
            ApprovalState::Denied => {
                self.write_pty(b"\x15");
                "denied"
            }
            ApprovalState::Expired => {
                self.write_pty(b"\x15");
                "expired"
            }
            ApprovalState::Pending => unreachable!(),
        };
        self.input.reset();
        let submission_id = if state == ApprovalState::Granted { self.shell_submission(&req.agent_id, &req.cmd) } else { None };
        self.audit.record(
            &req.agent_id,
            "exec",
            json!({ "cmd": req.cmd, "policy": "confirm", "label": req.label, "approval": outcome, "by": by, "id": req.id, "intent": req.intent, "submissionId": submission_id }),
        );
        let ev = ServerEvent::ApprovalResolved { approval_id: req.id.clone(), state, by: by.into() };
        self.notify(req.conn, ev.clone());
        self.broadcast(ev);
        self.broadcast(ServerEvent::AgentExec { agent_id: req.agent_id.clone(), cmd: req.cmd.clone(), policy: format!("confirm:{outcome}"), intent: req.intent.clone(), submission_id });
        self.notify_tools_changed();
        Some(req)
    }

    // ----- housekeeping ---------------------------------------------------

    /// Periodic maintenance. Call often (the engine uses 50 ms): lease expiry, approval
    /// expiry, scheduled executions, screen-change coalescing, policy reload.
    pub fn tick(&mut self, now: Instant) {
        if self.is_private() {
            if self.screen_dirty {
                self.screen_dirty = false;
                self.broadcast(ServerEvent::ScreenChanged { revision: self.screen.revision() });
            }
            return;
        }
        // Waiting on a human decision is not idleness: keep the lease alive meanwhile.
        if let Some(conn) = self.authority.controller().lease().map(|l| l.conn) {
            if self.approvals.has_pending_for(conn) {
                self.authority.renew(conn, now);
            }
        }
        if let Some(lease) = self.authority.expire_if_due(now) {
            self.cancel_scheduled_if(|_| true, "lease_expired");
            self.audit.record(&lease.agent_id, "lease_expired", json!({ "lease": lease.label() }));
            let ev = ServerEvent::ControlRevoked { lease: lease.label(), agent_id: lease.agent_id, reason: RevokeReason::Expired };
            self.notify(lease.conn, ev.clone());
            self.broadcast(ev);
            self.notify_tools_changed();
        }
        for id in self.approvals.due(now) {
            self.finish_approval(&id, ApprovalState::Expired, "timeout");
        }
        let ttl = Duration::from_secs(self.pacing.approval_ttl_secs.max(1));
        for id in self.control_requests.iter().filter(|r| r.state == ControlRequestState::Pending && now.duration_since(r.created) >= ttl).map(|r| r.request_id.clone()).collect::<Vec<_>>() {
            self.finish_control_request(&id, ControlRequestState::Expired);
        }
        let surface_ready = self.shared;
        if let Some(s) = &mut self.scheduled {
            if surface_ready && s.state == ExecState::Scheduled && s.due <= now {
                s.state = ExecState::Executed;
                let (agent, cmd, intent) = (s.agent_id.clone(), s.cmd.clone(), s.intent.clone());
                self.execute_with_source(&agent, &cmd, intent, None);
            }
        }
        if self.screen_dirty {
            self.screen_dirty = false;
            self.broadcast(ServerEvent::ScreenChanged { revision: self.screen.revision() });
        }
        match self.policy.maybe_reload() {
            Reload::Unchanged => {}
            Reload::Reloaded => self.audit.record("system", "policy_reloaded", json!({})),
            Reload::Failed(e) => {
                self.audit.record("system", "policy_reload_failed", json!({ "error": e.to_string() }));
                let msg = format!("\r\n\x1b[33mconn: policy reload failed ({e}); keeping previous policy\x1b[0m\r\n");
                self.write_output(msg.as_bytes());
            }
        }
    }

    pub fn status(&self) -> Status {
        let mut agent_connections: Vec<_> = self.conns.iter().filter(|(_, c)| c.kind == ConnKind::Agent)
            .map(|(conn, c)| AgentConnection { conn_id: *conn, agent_id: c.name.clone(), idle_secs: c.last_activity.elapsed().as_secs() }).collect();
        agent_connections.sort_by(|a, b| a.agent_id.cmp(&b.agent_id).then(a.conn_id.cmp(&b.conn_id)));
        let mut connected_agents: Vec<_> = agent_connections.iter().map(|c| c.agent_id.clone()).collect();
        connected_agents.dedup();
        Status {
            shell_integration: self.shell_integration.clone(),
            shared: self.shared,
            external_origin: self.external_origin,
            surface_generation: self.surface_generation,
            output_seq: self.output_seq,
            surface_available: self.shared,
            completion_prompt_ready: self.completion_prompt_ready(),
            input_pending: self.human_input_pending || self.input.has_pending(),
            external_input_available: self.external_writer_active(),
            profile_id: self.execution_profile.as_ref().map(|p|p.id.clone()),
            profile_name: self.execution_profile.as_ref().map(|p|p.name.clone()),
            review_required: self.review_required(),
            controller: self.controller_info(),
            process_alive: self.process_alive,
            revision: self.screen.revision(),
            size: self.screen.size(),
            pending: self.approvals.pending().into_iter().cloned().collect(),
            scheduled: self.scheduled_exec().cloned(),
            session_allows: self.policy.session_allows(),
            policy_path: self.policy.path().map(|p| p.display().to_string()),
            connected_agents,
            agent_connections,
            pacing: self.pacing.clone(),
            affordance_mask: self.affordance_mask(),
            mode: self.mode,
            effective_mode: self.effective_mode(),
            attended: self.attended,
            attention_request: self.attention_request.clone(),
            opened_by: self.opened_by.clone(),
            control_gate: self.control_gate,
            control_requests: self.control_requests.iter().filter(|r| r.state == ControlRequestState::Pending).cloned().collect(),
            proposal: self.proposal().cloned(),
            last_agent: self.last_agent.as_ref().map(|(conn, id)| LastAgent {
                agent_id: id.clone(),
                connected: self.conns.contains_key(conn),
                last_cmd: self.last_agent_cmd.clone(),
            }),
        }
    }

    pub fn audit(&self) -> &Audit {
        &self.audit
    }

    pub fn controller(&self) -> &Controller {
        self.authority.controller()
    }

    pub fn current_lease(&self) -> Option<&Lease> {
        self.authority.controller().lease()
    }

    pub fn input_line(&self) -> String {
        self.input.line()
    }

    pub fn screen(&self) -> &ScreenModel {
        &self.screen
    }
}
