//! Approval queue and the in-terminal approval prompt.

use std::time::{Duration, Instant};

use serde::Serialize;
use unicode_width::UnicodeWidthStr;

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

/// Render the approval prompt as raw terminal bytes (alternate screen, raw mode).
/// The label comes first: the human should be able to decide in three seconds.
pub fn render_prompt(req: &ApprovalRequest, cols: u16) -> Vec<u8> {
    let inner_max = (cols as usize).saturating_sub(4).clamp(30, 100);
    let mut lines: Vec<String> = Vec::new();
    lines.push(String::new());
    lines.push(format!("  ⚠  {}", req.label));
    if let Some(i) = &req.intent {
        lines.push(format!("     intent   {i}"));
    }
    if let Some(a) = &req.analysis {
        for s in &a.segments {
            for t in &s.targets {
                let mut d = format!("     target   {}", t.path);
                if t.git_repo { d.push_str(" · git"); }
                if let Some(n) = t.entries { d.push_str(&format!(" · {n} entries")); }
                if !t.exists { d.push_str(" · missing"); }
                if t.protected { d.push_str(" · protected path"); }
                lines.push(d);
            }
        }
    }
    lines.push(String::new());
    lines.push(format!("     from     {} · {}", req.agent_id, req.requested_at));
    lines.push(format!("     id       {}", req.id));
    lines.push(String::new());
    for chunk in wrap(&req.cmd, inner_max - 4) {
        lines.push(format!("  {chunk}"));
    }
    lines.push(String::new());
    lines.push("  [a] approve   [d] deny   [A] allow for this session".to_string());
    lines.push(String::new());

    let width = lines.iter().map(|l| l.width()).max().unwrap_or(0).max(inner_max.min(44));
    let title = "─ approval needed ";
    let mut out = String::new();
    out.push_str("\x1b[?1049h\x1b[H\x1b[2J\x1b[?25l");
    out.push_str("\r\n");
    out.push_str(&format!("┌{}{}┐\r\n", title, "─".repeat(width.saturating_sub(title.width()))));
    for l in &lines {
        out.push_str(&format!("│{}{}│\r\n", l, " ".repeat(width.saturating_sub(l.width()))));
    }
    out.push_str(&format!("└{}┘\r\n", "─".repeat(width)));
    out.push_str("\r\n  You can also decide with `conn approve <id>`.\r\n");
    out.into_bytes()
}

pub fn leave_prompt() -> Vec<u8> {
    b"\x1b[?25h\x1b[?1049l".to_vec()
}

fn wrap(s: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
        if cur.width() + w > max && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        cur.push(ch);
    }
    if !cur.is_empty() || out.is_empty() {
        out.push(cur);
    }
    out
}
