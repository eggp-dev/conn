//! App-wide admission, live identity and navigation metadata; never grants terminal access.
use crate::{authority::ConnId, session::AgentConnection};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
struct AgentRegistration {
    name: String,
    last_seen: std::time::Instant,
    catalog_changed: tokio::sync::mpsc::UnboundedSender<()>,
    admission: tokio::sync::watch::Sender<Admission>,
    session: Option<String>,
    preparation: Option<PreparationRequest>,
    last_preparation: Option<std::time::Instant>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparationRequest { pub id: u64, pub reason: Option<String> }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionActivity {
    pub conn_id: ConnId,
    pub agent_id: String,
    pub session: Option<String>,
    pub preparation: Option<PreparationRequest>,
}
#[derive(Serialize)]
pub struct ActivitySnapshot { pub revision: u64, pub connections: Vec<ConnectionActivity> }
pub type ActivityListener = Arc<dyn Fn() + Send + Sync>;

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
    activity_listener: parking_lot::Mutex<Option<ActivityListener>>,
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
            activity_listener: Default::default(),
        }
    }
}
impl ConnectionRegistry {
    pub fn set_activity_listener(&self, listener: ActivityListener) { *self.activity_listener.lock() = Some(listener); }
    fn activity_changed(&self) {
        let listener = self.activity_listener.lock().clone();
        if let Some(listener) = listener { listener(); }
    }
    pub fn activity_snapshot(&self) -> ActivitySnapshot {
        let state = self.state.lock();
        let mut connections: Vec<_> = state.agents.iter()
            .filter(|(_, a)| *a.admission.borrow() == Admission::Granted)
            .map(|(id, a)| ConnectionActivity { conn_id: *id, agent_id: a.name.clone(), session: a.session.clone(), preparation: a.preparation.clone() }).collect();
        connections.sort_by_key(|a| a.conn_id);
        ActivitySnapshot { revision: state.revision, connections }
    }
    pub(crate) fn bind(&self, conn: ConnId, session: &str) {
        let changed = {
            let mut state = self.state.lock();
            if let Some(a) = state.agents.get_mut(&conn).filter(|a| *a.admission.borrow() == Admission::Granted) {
                if a.session.as_deref() == Some(session) && a.preparation.is_none() { false }
                else { a.session = Some(session.into()); a.preparation = None; state.revision += 1; true }
            } else { false }
        };
        if changed { self.activity_changed(); }
    }
    /// One live request per admitted connection. Dismissed requests have a short cooldown.
    pub(crate) fn request_preparation(&self, conn: ConnId, reason: Option<String>) -> Option<PreparationRequest> {
        let request = {
            let mut state = self.state.lock();
            let revision = state.revision + 1;
            let a = state.agents.get_mut(&conn).filter(|a| *a.admission.borrow() == Admission::Granted)?;
            if let Some(request) = &a.preparation { return Some(request.clone()); }
            if a.last_preparation.is_some_and(|at| at.elapsed().as_secs() < 10) { return None; }
            let request = PreparationRequest { id: revision, reason };
            a.preparation = Some(request.clone()); a.last_preparation = Some(std::time::Instant::now());
            state.revision = revision;
            request
        };
        self.activity_changed(); Some(request)
    }
    pub fn dismiss_preparation(&self, conn: ConnId, id: u64) -> bool {
        let changed = {
            let mut state = self.state.lock();
            if let Some(a) = state.agents.get_mut(&conn).filter(|a| a.preparation.as_ref().is_some_and(|r| r.id == id)) {
                a.preparation = None; state.revision += 1; true
            } else { false }
        };
        if changed { self.activity_changed(); } changed
    }
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
        self.activity_changed();
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
                    session: None,
                    preparation: None,
                    last_preparation: None,
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
    fn preparation_requires_admission_coalesces_and_does_not_survive_reconnect() {
        let r=Arc::new(ConnectionRegistry::default());r.set_policy(AdmissionPolicy::Ask);
        let weak=Arc::downgrade(&r);
        r.set_activity_listener(Arc::new(move || { let _=weak.upgrade().unwrap().activity_snapshot(); }));
        let (tx,_)=tokio::sync::mpsc::unbounded_channel();r.register(1,"same",tx);
        assert!(r.request_preparation(1,None).is_none());assert!(r.activity_snapshot().connections.is_empty());
        r.decide(1,true);
        let first=r.request_preparation(1,Some("fixture".into())).unwrap();
        assert_eq!(r.request_preparation(1,None).unwrap().id,first.id);
        assert!(!r.dismiss_preparation(1,first.id+1));
        assert!(r.dismiss_preparation(1,first.id));
        assert!(r.request_preparation(1,None).is_none(),"dismissal cooldown");
        r.state.lock().agents.get_mut(&1).unwrap().last_preparation=None;
        let next=r.request_preparation(1,None).unwrap();assert!(next.id>first.id);
        assert!(!r.dismiss_preparation(1,first.id),"old UI cannot dismiss a replacement");
        r.bind(1,"tab");let snapshot=r.activity_snapshot();
        assert_eq!(snapshot.connections[0].session.as_deref(),Some("tab"));assert!(snapshot.connections[0].preparation.is_none());
        r.forget(1);assert!(r.activity_snapshot().connections.is_empty());
        let (tx,_)=tokio::sync::mpsc::unbounded_channel();r.register(2,"same",tx);r.decide(2,true);
        let snapshot=r.activity_snapshot();assert!(snapshot.connections[0].session.is_none());assert!(snapshot.connections[0].preparation.is_none());
    }
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
