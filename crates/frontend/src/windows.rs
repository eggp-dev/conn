//! Frontend ownership only; every window still shares one engine hub and policy.
use std::collections::HashMap;

#[derive(Default)]
struct Window {
    sessions: Vec<String>,
    active: Option<String>,
    ready: bool,
    closed: bool,
}

#[derive(Default)]
pub(crate) struct Windows(HashMap<String, Window>);
impl Windows {
    pub fn available(&self, window: &str) -> bool {
        !window.is_empty() && !self.0.get(window).is_some_and(|w| w.closed)
    }
    pub fn add(&mut self, window: &str, session: &str) {
        let w = self.0.entry(window.into()).or_default();
        w.sessions.push(session.into());
        if w.active.is_none() {
            w.active = Some(session.into());
        }
    }
    pub fn owner(&self, session: &str) -> Option<String> {
        self.0
            .iter()
            .find(|(_, w)| w.sessions.iter().any(|id| id == session))
            .map(|(id, _)| id.clone())
    }
    pub fn sessions(&self, window: &str) -> Vec<String> {
        self.0
            .get(window)
            .map(|w| w.sessions.clone())
            .unwrap_or_default()
    }
    pub fn active(&self, window: &str) -> Option<String> {
        self.0.get(window).and_then(|w| w.active.clone())
    }
    pub fn select(&mut self, window: &str, session: &str) {
        if let Some(w) = self.0.get_mut(window) {
            if w.sessions.iter().any(|id| id == session) {
                w.active = Some(session.into());
            }
        }
    }
    pub fn remove(&mut self, session: &str) {
        for w in self.0.values_mut() {
            w.sessions.retain(|id| id != session);
            if w.active.as_deref() == Some(session) {
                w.active = w.sessions.first().cloned();
            }
        }
    }
    pub fn ready(&self, window: &str) -> bool {
        self.0.get(window).is_some_and(|w| w.ready && !w.closed)
    }
    pub fn mark_ready(&mut self, window: &str) {
        self.0.entry(window.into()).or_default().ready = true;
    }
    pub fn close(&mut self, window: &str) {
        let w = self.0.entry(window.into()).or_default();
        w.closed = true;
        w.ready = false;
    }
}
