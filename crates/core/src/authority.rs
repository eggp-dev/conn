//! Lease-based write authority. Exactly one writer at a time; the human by default.

use std::time::{Duration, Instant};

use serde::Serialize;
use uuid::Uuid;

pub type ConnId = u64;

#[derive(Debug, Clone)]
pub struct Lease {
    pub id: Uuid,
    pub seq: u64,
    pub agent_id: String,
    pub conn: ConnId,
    pub expires_at: Instant,
}

impl Lease {
    pub fn label(&self) -> String {
        format!("lease#{}", self.seq)
    }
}

#[derive(Debug, Clone)]
pub enum Controller {
    Human,
    Agent(Lease),
}

impl Controller {
    pub fn is_human(&self) -> bool {
        matches!(self, Controller::Human)
    }
    pub fn lease(&self) -> Option<&Lease> {
        match self {
            Controller::Agent(l) => Some(l),
            Controller::Human => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RevokeReason {
    /// Human typed on stdin (preemptive takeover).
    HumanInput,
    /// Human ran `conn take`.
    Taken,
    /// Agent called `release_control`.
    Released,
    /// TTL elapsed without renewal.
    Expired,
    /// The agent's IPC connection closed.
    Disconnected,
    /// The child process exited.
    ProcessExited,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AuthorityError {
    #[error("another agent ({agent_id}) currently holds control")]
    Busy { agent_id: String },
    #[error("you do not hold control of this session")]
    NotController,
    #[error("your lease has expired")]
    Expired,
}

pub struct Authority {
    controller: Controller,
    ttl: Duration,
    seq: u64,
}

impl Authority {
    pub const DEFAULT_TTL: Duration = Duration::from_secs(60);

    pub fn new(ttl: Duration) -> Self {
        Self { controller: Controller::Human, ttl, seq: 0 }
    }

    pub fn controller(&self) -> &Controller {
        &self.controller
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    pub fn set_ttl(&mut self, ttl: Duration) {
        self.ttl = ttl;
    }

    /// Grant a lease to `conn`. Re-requesting while holding a valid lease renews it.
    pub fn request(&mut self, agent_id: &str, conn: ConnId, now: Instant) -> Result<Lease, AuthorityError> {
        if let Controller::Agent(l) = &self.controller {
            if l.conn == conn {
                let mut l = l.clone();
                l.expires_at = now + self.ttl;
                self.controller = Controller::Agent(l.clone());
                return Ok(l);
            }
            if l.expires_at > now {
                return Err(AuthorityError::Busy { agent_id: l.agent_id.clone() });
            }
            // Expired lease held by someone else: reclaim silently.
        }
        self.seq += 1;
        let lease = Lease {
            id: Uuid::new_v4(),
            seq: self.seq,
            agent_id: agent_id.to_string(),
            conn,
            expires_at: now + self.ttl,
        };
        self.controller = Controller::Agent(lease.clone());
        Ok(lease)
    }

    /// Revoke whatever lease exists. Returns the revoked lease, if any.
    /// After this call the controller is always `Human`.
    pub fn revoke(&mut self) -> Option<Lease> {
        match std::mem::replace(&mut self.controller, Controller::Human) {
            Controller::Agent(l) => Some(l),
            Controller::Human => None,
        }
    }

    /// Revoke only if `conn` holds the lease.
    pub fn revoke_if_held_by(&mut self, conn: ConnId) -> Option<Lease> {
        match &self.controller {
            Controller::Agent(l) if l.conn == conn => self.revoke(),
            _ => None,
        }
    }

    /// Verify `conn` may write now. Renews the lease on success.
    pub fn check_and_touch(&mut self, conn: ConnId, now: Instant) -> Result<(), AuthorityError> {
        match &mut self.controller {
            Controller::Agent(l) if l.conn == conn => {
                if l.expires_at <= now {
                    self.controller = Controller::Human;
                    return Err(AuthorityError::Expired);
                }
                l.expires_at = now + self.ttl;
                Ok(())
            }
            _ => Err(AuthorityError::NotController),
        }
    }

    /// Extend the lease held by `conn` without an expiry check (used while the
    /// holder is blocked on a human decision).
    pub fn renew(&mut self, conn: ConnId, now: Instant) {
        if let Controller::Agent(l) = &mut self.controller {
            if l.conn == conn {
                l.expires_at = now + self.ttl;
            }
        }
    }

    pub fn holds(&self, conn: ConnId) -> bool {
        matches!(&self.controller, Controller::Agent(l) if l.conn == conn)
    }

    /// Drop the lease if its TTL elapsed. Returns the expired lease.
    pub fn expire_if_due(&mut self, now: Instant) -> Option<Lease> {
        match &self.controller {
            Controller::Agent(l) if l.expires_at <= now => self.revoke(),
            _ => None,
        }
    }
}
