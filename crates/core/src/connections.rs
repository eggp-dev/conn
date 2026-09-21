//! App-wide admission and live connection identity; no sessions or terminal access.
use crate::{authority::ConnId, session::AgentConnection};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
struct AgentRegistration {
    name: String,
    last_seen: std::time::Instant,
    catalog_changed: tokio::sync::mpsc::UnboundedSender<()>,
    admission: tokio::sync::watch::Sender<Admission>,
}

/// Whether a new agent connection participates at once or waits for the owner.
/// Ordinary tabs are open to every admitted connection, so admission is where
/// the owner's explicit consent lives. Embedders default to `Allow`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdmissionPolicy {
    Allow,
    Ask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Admission {
    Pending,
    Granted,
    Denied,
}

/// `state` is `pending`, `granted`, `denied` or `closed` (left while pending).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionChange {
    pub conn_id: ConnId,
    pub agent_id: String,
    pub state: &'static str,
    pub revision: u64,
}

pub type AdmissionListener = Arc<dyn Fn(AdmissionChange) + Send + Sync>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionSnapshot {
    pub revision: u64,
    pub pending: Vec<AgentConnection>,
}
struct State {
    agents: HashMap<ConnId, AgentRegistration>,
    revision: u64,
    policy: AdmissionPolicy,
}
pub struct ConnectionRegistry {
    state: parking_lot::Mutex<State>,
    listener: parking_lot::Mutex<Option<AdmissionListener>>,
}
impl Default for ConnectionRegistry {
    fn default() -> Self {
        Self {
            state: parking_lot::Mutex::new(State {
                agents: HashMap::new(),
                revision: 0,
                policy: AdmissionPolicy::Allow,
            }),
            listener: Default::default(),
        }
    }
}
impl ConnectionRegistry {
    fn connections(state: &State, admission: Admission) -> Vec<AgentConnection> {
        let mut out: Vec<_> = state
            .agents
            .iter()
            .filter(|(_, a)| *a.admission.borrow() == admission)
            .map(|(conn, a)| AgentConnection {
                conn_id: *conn,
                agent_id: a.name.clone(),
                idle_secs: a.last_seen.elapsed().as_secs(),
            })
            .collect();
        out.sort_by_key(|a| a.conn_id);
        out
    }
    pub fn admitted(&self) -> Vec<AgentConnection> {
        Self::connections(&self.state.lock(), Admission::Granted)
    }
    pub fn snapshot(&self) -> AdmissionSnapshot {
        let state = self.state.lock();
        AdmissionSnapshot {
            revision: state.revision,
            pending: Self::connections(&state, Admission::Pending),
        }
    }
    pub fn policy(&self) -> AdmissionPolicy {
        self.state.lock().policy
    }
    pub fn set_listener(&self, listener: AdmissionListener) {
        *self.listener.lock() = Some(listener);
    }
    fn emit(&self, change: AdmissionChange) {
        let listener = self.listener.lock().clone();
        if let Some(listener) = listener {
            listener(change);
        }
    }
    pub fn set_policy(&self, policy: AdmissionPolicy) {
        let events = {
            let mut state = self.state.lock();
            state.policy = policy;
            let pending = if policy == AdmissionPolicy::Allow {
                Self::connections(&state, Admission::Pending)
            } else {
                Vec::new()
            };
            pending
                .into_iter()
                .filter_map(|a| Self::decide_locked(&mut state, a.conn_id, true))
                .collect::<Vec<_>>()
        };
        for event in events {
            self.emit(event);
        }
    }
    fn decide_locked(state: &mut State, conn: ConnId, allow: bool) -> Option<AdmissionChange> {
        let agent = state
            .agents
            .get(&conn)
            .filter(|a| *a.admission.borrow() == Admission::Pending)?;
        agent.admission.send_replace(if allow {
            Admission::Granted
        } else {
            Admission::Denied
        });
        let _ = agent.catalog_changed.send(());
        state.revision += 1;
        Some(AdmissionChange {
            conn_id: conn,
            agent_id: agent.name.clone(),
            state: if allow { "granted" } else { "denied" },
            revision: state.revision,
        })
    }
    pub fn decide(&self, conn: ConnId, allow: bool) -> bool {
        let event = Self::decide_locked(&mut self.state.lock(), conn, allow);
        if let Some(event) = event {
            self.emit(event);
            true
        } else {
            false
        }
    }
    pub(crate) fn register(
        &self,
        conn: ConnId,
        name: &str,
        catalog_changed: tokio::sync::mpsc::UnboundedSender<()>,
    ) -> tokio::sync::watch::Receiver<Admission> {
        let (rx, event) = {
            let mut state = self.state.lock();
            let admission_state = if state.policy == AdmissionPolicy::Ask {
                Admission::Pending
            } else {
                Admission::Granted
            };
            let (admission, rx) = tokio::sync::watch::channel(admission_state);
            state.agents.insert(
                conn,
                AgentRegistration {
                    name: name.into(),
                    last_seen: std::time::Instant::now(),
                    catalog_changed,
                    admission,
                },
            );
            state.revision += 1;
            (
                rx,
                AdmissionChange {
                    conn_id: conn,
                    agent_id: name.into(),
                    state: if admission_state == Admission::Pending {
                        "pending"
                    } else {
                        "granted"
                    },
                    revision: state.revision,
                },
            )
        };
        self.emit(event);
        rx
    }
    pub(crate) fn forget(&self, conn: ConnId) {
        let event = {
            let mut state = self.state.lock();
            state.agents.remove(&conn).map(|agent| {
                state.revision += 1;
                AdmissionChange {
                    conn_id: conn,
                    agent_id: agent.name,
                    state: "closed",
                    revision: state.revision,
                }
            })
        };
        if let Some(event) = event {
            self.emit(event);
        }
    }
    pub(crate) fn touch(&self, conn: ConnId) {
        if let Some(agent) = self.state.lock().agents.get_mut(&conn) {
            agent.last_seen = std::time::Instant::now();
        }
    }
    pub(crate) fn invalidate(&self, affected: Option<&std::collections::HashSet<ConnId>>) {
        for (conn, agent) in self.state.lock().agents.iter() {
            if affected.is_none_or(|ids| ids.contains(conn)) {
                let _ = agent.catalog_changed.send(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_and_events_share_a_revision_and_callbacks_can_read_state() {
        let registry = Arc::new(ConnectionRegistry::default());
        registry.set_policy(AdmissionPolicy::Ask);
        let weak = Arc::downgrade(&registry);
        registry.set_listener(Arc::new(move |event| {
            let registry = weak.upgrade().unwrap();
            assert!(registry.snapshot().revision >= event.revision);
        }));
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let admission = registry.register(7, "same-name", tx);
        assert_eq!(*admission.borrow(), Admission::Pending);
        assert_eq!(registry.snapshot().revision, 1);
        assert!(registry.decide(7, true));
        assert!(!registry.decide(7, false));
        assert_eq!(registry.snapshot().revision, 2);
        assert!(registry.snapshot().pending.is_empty());
        registry.forget(7);
        assert_eq!(registry.snapshot().revision, 3);
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let next = registry.register(8, "same-name", tx);
        assert_eq!(*next.borrow(), Admission::Pending);
        assert!(registry.admitted().is_empty());
    }
    #[test]
    fn simultaneous_windows_can_resolve_only_once() {
        let registry = Arc::new(ConnectionRegistry::default());
        registry.set_policy(AdmissionPolicy::Ask);
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let _rx = registry.register(1, "test", tx);
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let threads: Vec<_> = [true, false]
            .into_iter()
            .map(|allow| {
                let (r, b) = (registry.clone(), barrier.clone());
                std::thread::spawn(move || {
                    b.wait();
                    r.decide(1, allow)
                })
            })
            .collect();
        barrier.wait();
        assert_eq!(
            threads
                .into_iter()
                .filter_map(|t| t.join().ok())
                .filter(|won| *won)
                .count(),
            1
        );
        assert_eq!(registry.snapshot().revision, 2);
    }
}
