//! Approval queue.

use std::time::{Duration, Instant};

use serde::Serialize;

use crate::authority::ConnId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalState {
    Pending,
    Granted,
    Denied,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Grant,
    Deny,
    /// Grant, and allow this label for the rest of the session.
    AllowSession,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApprovalRequest {
    pub id: String,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    #[serde(skip)]
    pub conn: ConnId,
    pub cmd: String,
    pub label: String,
    #[serde(rename = "requestedAt")]
    pub requested_at: String,
    #[serde(skip)]
    pub created: Instant,
    pub state: ApprovalState,
    /// What the agent said this command is for.
    pub intent: Option<String>,
    /// Structural analysis: segments, verdicts, resolved targets.
    pub analysis: Option<crate::policy::LineAnalysis>,
}

pub struct ApprovalQueue {
    seq: u64,
    items: Vec<ApprovalRequest>,
    ttl: Duration,
}

impl ApprovalQueue {
    pub const DEFAULT_TTL: Duration = Duration::from_secs(300);

    pub fn new(ttl: Duration) -> Self {
        Self { seq: 0, items: Vec::new(), ttl }
    }

    pub fn set_ttl(&mut self, ttl: Duration) {
        self.ttl = ttl;
    }

    pub fn create(&mut self, agent_id: &str, conn: ConnId, cmd: &str, label: &str, now: Instant) -> &ApprovalRequest {
        self.create_with(agent_id, conn, cmd, label, now, None, None)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_with(&mut self, agent_id: &str, conn: ConnId, cmd: &str, label: &str, now: Instant, intent: Option<String>, analysis: Option<crate::policy::LineAnalysis>) -> &ApprovalRequest {
        self.seq += 1;
        self.items.push(ApprovalRequest {
            id: format!("apr-{}", self.seq),
            agent_id: agent_id.to_string(),
            conn,
            cmd: cmd.to_string(),
            label: label.to_string(),
            requested_at: chrono::Local::now().format("%H:%M:%S").to_string(),
            created: now,
            state: ApprovalState::Pending,
            intent,
            analysis,
        });
        self.items.last().unwrap()
    }

    pub fn get(&self, id: &str) -> Option<&ApprovalRequest> {
        self.items.iter().find(|a| a.id == id)
    }

    pub fn pending(&self) -> Vec<&ApprovalRequest> {
        self.items.iter().filter(|a| a.state == ApprovalState::Pending).collect()
    }

    pub fn first_pending(&self) -> Option<&ApprovalRequest> {
        self.items.iter().find(|a| a.state == ApprovalState::Pending)
    }

    pub fn has_pending_for(&self, conn: ConnId) -> bool {
        self.items.iter().any(|a| a.conn == conn && a.state == ApprovalState::Pending)
    }

    /// Transition a pending request. Returns the request if it was pending.
    pub fn resolve(&mut self, id: &str, state: ApprovalState) -> Option<ApprovalRequest> {
        let a = self.items.iter_mut().find(|a| a.id == id && a.state == ApprovalState::Pending)?;
        a.state = state;
        Some(a.clone())
    }

    /// Ids of pending requests older than the TTL.
    pub fn due(&self, now: Instant) -> Vec<String> {
        self.items
            .iter()
            .filter(|a| a.state == ApprovalState::Pending && now.duration_since(a.created) >= self.ttl)
            .map(|a| a.id.clone())
            .collect()
    }

    pub fn pending_for_conn(&self, conn: ConnId) -> Vec<String> {
        self.items
            .iter()
            .filter(|a| a.conn == conn && a.state == ApprovalState::Pending)
            .map(|a| a.id.clone())
            .collect()
    }
}
