//! Session catalog and cross-session authority. Transport adapters share this coordinator.
use crate::connections::{Admission, AdmissionListener, AdmissionPolicy, ConnectionRegistry};
use crate::{authority::ConnId, protocol::RpcError, session::SharedSession};
use serde_json::Value;
use std::sync::Arc;
pub type SessionId = String;

/// Sessions behind one socket. Exactly one is *attended* — the one the human is
/// looking at — and that is where agents go unless they name a session.
/// Opens a new session (tab) on behalf of an agent: `(agent_id, reason) -> session id`.
/// The host spawns the shell, registers it with the hub and tells its UI.
pub type TabOpener = Arc<dyn Fn(&str, Option<&str>) -> Result<SessionId, String> + Send + Sync>;

pub struct Hub {
    sessions: parking_lot::Mutex<Vec<(SessionId, SharedSession)>>,
    attended: parking_lot::Mutex<Option<SessionId>>,
    opener: parking_lot::Mutex<Option<TabOpener>>,
    pub(crate) connections: Arc<ConnectionRegistry>,
    control: parking_lot::Mutex<()>,
}

pub type SharedHub = Arc<Hub>;

impl Hub {
    pub fn new() -> SharedHub {
        Arc::new(Self {
            sessions: Default::default(),
            attended: Default::default(),
            opener: Default::default(),
            connections: Arc::new(ConnectionRegistry::default()),
            control: Default::default(),
        })
    }
    pub fn agent_connections(&self) -> Vec<crate::session::AgentConnection> {
        self.connections.admitted()
    }
    pub fn pending_admissions(&self) -> Vec<crate::session::AgentConnection> {
        self.connections.snapshot().pending
    }
    pub fn admission_snapshot(&self) -> crate::connections::AdmissionSnapshot {
        self.connections.snapshot()
    }
    pub fn admission_policy(&self) -> AdmissionPolicy {
        self.connections.policy()
    }
    pub fn set_admission_policy(&self, policy: AdmissionPolicy) {
        self.connections.set_policy(policy);
    }
    pub fn set_admission_listener(&self, listener: AdmissionListener) {
        self.connections.set_listener(listener);
    }
    pub fn decide_admission(&self, conn: ConnId, allow: bool) -> bool {
        self.connections.decide(conn, allow)
    }
    pub(crate) fn admit(
        &self,
        conn: ConnId,
        name: &str,
        changed: tokio::sync::mpsc::UnboundedSender<()>,
    ) -> tokio::sync::watch::Receiver<Admission> {
        self.connections.register(conn, name, changed)
    }
    pub(crate) fn forget_agent(&self, conn: ConnId) {
        self.connections.forget(conn);
    }

    /// Locks a stable set of sessions for a short authority transition. No wait,
    /// PTY I/O loop or UI acknowledgement runs here. Observers cannot see two
    /// leases while the new target is committed and the previous work is revoked.
    fn control_transition<T, E: From<crate::session::SessionError>>(
        &self,
        id: &str,
        action: impl FnOnce(&mut crate::Session) -> Result<(ConnId, T), E>,
    ) -> Result<T, E> {
        let _control = self.control.lock();
        let sessions = self.sessions.lock().clone();
        let target = sessions
            .iter()
            .position(|(candidate, _)| candidate == id)
            .ok_or_else(|| crate::session::SessionError::NotFound("session unavailable".into()))?;
        let mut locked: Vec<_> = sessions.iter().map(|(_, session)| session.lock()).collect();
        let (conn, result) = action(&mut locked[target])?;
        for (index, session) in locked.iter_mut().enumerate() {
            if index != target {
                session.agent_left_tab(conn);
            }
        }
        Ok(result)
    }
    pub fn request_control(
        &self,
        id: &str,
        conn: ConnId,
        params: &Value,
    ) -> Result<Value, RpcError> {
        self.control_transition(id, |session| {
            crate::agent_commands::dispatch_locked(session, conn, "request_control", params)
                .map(|result| (conn, result))
        })
    }
    pub(crate) fn leave_other_tabs(&self, conn: ConnId, keep: &str) {
        let _: Result<(), crate::session::SessionError> =
            self.control_transition(keep, |_| Ok((conn, ())));
    }
    pub fn decide_control(
        &self,
        id: &str,
        request: &str,
        grant: bool,
    ) -> Result<crate::session::ControlRequest, crate::session::SessionError> {
        if !grant {
            let session = self.get(id).ok_or_else(|| {
                crate::session::SessionError::NotFound("session unavailable".into())
            })?;
            return session.lock().decide_control(request, false);
        }
        self.control_transition(id, |session| {
            session
                .decide_control(request, true)
                .map(|result| (result.conn, result))
        })
    }
    pub fn hand_back(
        &self,
        id: &str,
    ) -> Result<crate::session::LeaseInfo, crate::session::SessionError> {
        self.control_transition(id, |session| {
            let info = session.hand_back()?;
            Ok((session.controller_conn().expect("hand_back granted"), info))
        })
    }

    pub fn single(id: &str, session: SharedSession) -> SharedHub {
        let hub = Self::new();
        hub.add(id, session);
        hub
    }

    /// Let agents open tabs. Without an opener `open_tab` is unsupported and the
    /// tab affordances are not offered.
    pub fn set_opener(&self, opener: TabOpener) {
        *self.opener.lock() = Some(opener);
        for (_, s) in self.sessions.lock().iter() {
            s.lock().set_tabs_supported(true);
        }
    }

    pub fn tabs_supported(&self) -> bool {
        self.opener.lock().is_some()
    }

    /// Open a tab for `agent_id`. Returns the new session's id.
    pub fn open_tab(&self, agent_id: &str, reason: Option<&str>) -> Result<SessionId, RpcError> {
        let opener = self.opener.lock().clone().ok_or_else(|| RpcError {
            code: "unsupported".into(),
            message: "this host cannot open tabs".into(),
        })?;
        let id = opener(agent_id, reason).map_err(|m| RpcError {
            code: "open_failed".into(),
            message: m,
        })?;
        if self.get_public(&id).is_none() {
            return Err(RpcError {
                code: "open_failed".into(),
                message: "opener did not register an available session".into(),
            });
        }
        Ok(id)
    }

    pub fn add(&self, id: &str, session: SharedSession) {
        let first = self.sessions.lock().is_empty();
        session.lock().set_tabs_supported(self.tabs_supported());
        let agents = Arc::downgrade(&self.connections);
        session
            .lock()
            .set_participation_listener(Box::new(move |affected| {
                if let Some(agents) = agents.upgrade() {
                    agents.invalidate(affected);
                }
            }));
        self.sessions.lock().push((id.to_string(), session.clone()));
        if first {
            *self.attended.lock() = Some(id.to_string());
            session.lock().set_attended(true);
        } else {
            session.lock().set_attended(false);
        }
    }

    pub fn remove(&self, id: &str) -> Option<SharedSession> {
        let mut list = self.sessions.lock();
        let pos = list.iter().position(|(i, _)| i == id)?;
        let (_, s) = list.remove(pos);
        drop(list);
        if self.attended.lock().as_deref() == Some(id) {
            let next = self.sessions.lock().first().map(|(i, _)| i.clone());
            drop(self.attended.lock());
            if let Some(n) = next {
                self.set_attended(&n);
            } else {
                *self.attended.lock() = None;
            }
        }
        Some(s)
    }

    pub fn ids(&self) -> Vec<SessionId> {
        self.sessions
            .lock()
            .iter()
            .map(|(i, _)| i.clone())
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<SharedSession> {
        self.sessions
            .lock()
            .iter()
            .find(|(i, _)| i == id)
            .map(|(_, s)| s.clone())
    }

    pub fn attended_id(&self) -> Option<SessionId> {
        self.attended.lock().clone()
    }

    /// The human now looks at `id`. Every other session becomes unattended.
    pub fn set_attended(&self, id: &str) -> bool {
        let list: Vec<(SessionId, SharedSession)> = self.sessions.lock().clone();
        if !list.iter().any(|(i, _)| i == id) {
            return false;
        }
        *self.attended.lock() = Some(id.to_string());
        for (i, s) in list {
            s.lock().set_attended(i == id);
        }
        true
    }

    /// Public clients cannot enumerate or attach to private native sessions.
    pub fn public_ids(&self) -> Vec<SessionId> {
        self.sessions
            .lock()
            .iter()
            .filter(|(_, s)| !s.lock().is_private())
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn get_public(&self, id: &str) -> Option<SharedSession> {
        self.get(id).filter(|s| !s.lock().is_private())
    }

    pub fn public_attended_id(&self) -> Option<SessionId> {
        self.attended_id()
            .filter(|id| self.get_public(id).is_some())
    }

    pub(crate) fn public_attended_for(&self, conn: ConnId) -> Option<SessionId> {
        self.public_attended_id().filter(|id| {
            self.get_public(id)
                .is_some_and(|s| s.lock().participant_allowed(conn))
        })
    }

    pub(crate) fn public_ids_for(&self, conn: ConnId) -> Vec<SessionId> {
        self.public_ids()
            .into_iter()
            .filter(|id| {
                self.get_public(id)
                    .is_some_and(|s| s.lock().participant_allowed(conn))
            })
            .collect()
    }

    pub(crate) fn public_index_of(&self, id: &str, conn: ConnId) -> Option<usize> {
        self.public_ids_for(conn)
            .iter()
            .position(|candidate| candidate == id)
            .map(|i| i + 1)
    }

    pub(crate) fn find_public_tab(
        &self,
        tab: &Value,
        conn: ConnId,
    ) -> Option<(SessionId, SharedSession)> {
        let ids = self.public_ids_for(conn);
        let id = match tab {
            Value::Number(n) => ids.get(n.as_u64()?.checked_sub(1)? as usize),
            Value::String(id) => ids.iter().find(|i| *i == id).or_else(|| {
                id.parse::<usize>()
                    .ok()
                    .and_then(|i| ids.get(i.checked_sub(1)?))
            }),
            _ => None,
        }?;
        self.get_public(id).map(|session| (id.clone(), session))
    }

    pub(crate) fn resolve_public(
        &self,
        session: Option<&str>,
    ) -> Option<(SessionId, SharedSession)> {
        let id = session
            .map(str::to_string)
            .or_else(|| self.public_attended_id())?;
        self.get_public(&id).map(|s| (id, s))
    }
}
