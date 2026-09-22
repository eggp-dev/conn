//! Audience state lives inside the Session lock. It never grants a write lease.
use crate::authority::ConnId;
use std::collections::HashSet;
pub(super) struct Participation {
    pub shared: bool,
    pub selected: Option<HashSet<ConnId>>,
    pub generation: u64,
}
impl Participation {
    pub fn new(private: bool) -> Self {
        Self {
            shared: !private,
            selected: if private { Some(HashSet::new()) } else { None },
            generation: 1,
        }
    }
    pub fn allows(&self, conn: ConnId) -> bool {
        self.shared && self.selected.as_ref().is_none_or(|ids| ids.contains(&conn))
    }
    pub fn replace(&mut self, shared: bool, selected: HashSet<ConnId>) {
        self.shared = shared;
        self.selected = Some(selected);
        self.generation = self.generation.saturating_add(1);
    }
}

use super::{ParticipationListener, RevokeReason, ServerEvent, Session, SessionError};
use serde_json::json;
impl Session {
    /// Only the local owner may add/remove agent participation. The PTY is untouched.
    pub fn set_shared(&mut self, shared: bool) -> Result<(), SessionError> {
        self.set_shared_with_agents(shared, Vec::new())
    }

    /// Host-only catalog invalidation. The listener must not lock any session.
    pub(crate) fn set_participation_listener(&mut self, listener: ParticipationListener) {
        self.participation_listener = Some(listener);
    }

    /// Side-effect-free preflight, used while the owner holds the writer fence.
    pub fn validate_sharing(&self, shared: bool, agents: &[ConnId]) -> Result<(), SessionError> {
        if agents.len() > 64 {
            return Err(SessionError::InvalidInput("at most 64 participants".into()));
        }
        if !self.process_alive {
            return Err(SessionError::ProcessExited);
        }
        let selected: HashSet<_> = agents.iter().copied().collect();
        if self.participation.shared == shared
            && self.participation.selected.as_ref() == Some(&selected)
        {
            return Ok(());
        }
        if shared && (self.input.has_pending() || self.human_input.pending) {
            return Err(SessionError::InputPending);
        }
        Ok(())
    }
    pub fn set_shared_with_agents(
        &mut self,
        shared: bool,
        agents: Vec<ConnId>,
    ) -> Result<(), SessionError> {
        self.validate_sharing(shared, &agents)?;
        let selected: HashSet<_> = agents.into_iter().collect();
        if self.participation.shared == shared
            && self.participation.selected.as_ref() == Some(&selected)
        {
            return Ok(());
        }
        let affected = if self.participation.shared && self.participation.selected.is_none() {
            None
        } else {
            let mut ids = if self.participation.shared {
                self.participation.selected.clone().unwrap_or_default()
            } else {
                HashSet::new()
            };
            if shared {
                ids.extend(selected.iter().copied());
            }
            Some(ids)
        };
        // Revocation cannot be held hostage by a partial agent write. Preserve the
        // physical input for the owner instead of guessing a clear key in a TUI.
        let unfinished_agent_input = self.input.has_pending();
        self.revoke_external();
        self.cancel_agent_work("sharing_changed");
        self.human_input.pending |= unfinished_agent_input && self.input.has_pending();
        if !shared {
            self.audit.record(
                "human",
                "sharing_stopped",
                json!({ "generation": self.surface_generation }),
            );
        }
        self.audit.set_enabled(shared);
        self.participation.replace(shared, selected);
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
        if shared {
            self.audit.record(
                "human",
                "sharing_started",
                json!({ "generation": self.surface_generation }),
            );
        }
        self.broadcast(ServerEvent::SharingChanged {
            shared,
            generation: self.surface_generation,
        });
        self.notify_tools_changed();
        // New participants have no session subscription yet; removed participants'
        // session events are deliberately discarded by the transport's ACL guard.
        if let Some(listener) = &self.participation_listener {
            listener(affected.as_ref());
        }
        Ok(())
    }

    fn cancel_agent_work(&mut self, reason: &str) {
        self.cancel_pending_work(None, reason);
        if let Some(lease) = self.authority.revoke() {
            let ev = ServerEvent::ControlRevoked {
                lease: lease.label(),
                agent_id: lease.agent_id,
                reason: RevokeReason::Taken,
            };
            self.notify(lease.conn, ev.clone());
            self.broadcast(ev);
        }
    }
}
