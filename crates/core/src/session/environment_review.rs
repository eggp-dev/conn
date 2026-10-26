//! Ephemeral owner decisions. Never persisted or writable through agent dispatch.
use super::*;

pub(super) struct EnvironmentReviewGrant {
    pub id: String,
    conn: ConnId,
    expires: Instant,
    revision: u64,
    profile_revision: u64,
    foreground: Option<i32>,
    transport_sequence: Option<u64>,
    remote: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentReviewInfo {
    pub grant_id: String,
    pub conn_id: ConnId,
    pub agent_id: String,
    pub remaining_ms: u64,
    pub context_revision: u64,
}

impl Session {
    /// Local owner only. A repeated click cannot extend an existing decision.
    pub fn start_environment_review(&mut self, conn: ConnId) -> Result<(), SessionError> {
        self.start_environment_review_at(conn, Instant::now())
    }

    fn start_environment_review_at(
        &mut self,
        conn: ConnId,
        now: Instant,
    ) -> Result<(), SessionError> {
        self.refresh_environment_review(now);
        self.require_shared()?;
        self.require_participant(conn)?;
        if !self.process_alive {
            return Err(SessionError::ProcessExited);
        }
        if self.conn_kind(conn) != Some(ConnKind::Agent) || !self.review_required() {
            return Err(SessionError::InvalidInput(
                "environment_review_unavailable".into(),
            ));
        }
        if self.environment_grants.contains_key(&conn) {
            return Err(SessionError::InvalidInput(
                "environment_review_already_active".into(),
            ));
        }
        let grant = EnvironmentReviewGrant {
            id: uuid::Uuid::new_v4().to_string(),
            conn,
            expires: now + Duration::from_secs(15 * 60),
            revision: self.environment_revision,
            profile_revision: self.profile_revision,
            foreground: self.foreground_id(),
            transport_sequence: self.ssh_foreground_sequence,
            remote: self.remote_policy(),
        };
        self.audit.record(
            "human",
            "environment_review_started",
            json!({
                "grantID": grant.id, "sessionID": self.surface_id, "connID": conn,
                "contextRevision": grant.revision, "durationSecs": 900,
            }),
        );
        self.environment_grants.insert(conn, grant);
        self.broadcast(ServerEvent::EnvironmentReviewChanged {
            grants: self.environment_review_status(None, now),
        });
        Ok(())
    }

    pub fn stop_environment_review(&mut self, conn: ConnId, reason: &str) -> bool {
        let Some(grant) = self.environment_grants.remove(&conn) else {
            return false;
        };
        self.audit.record(
            "system",
            "environment_review_ended",
            json!({
                "grantID": grant.id, "sessionID": self.surface_id, "connID": conn,
                "contextRevision": grant.revision, "reason": reason,
            }),
        );
        self.broadcast(ServerEvent::EnvironmentReviewChanged {
            grants: self.environment_review_status(None, Instant::now()),
        });
        self.cancel_scheduled_if(
            |s| s.environment_grant.as_deref() == Some(&grant.id),
            "environment_review_changed",
        );
        true
    }

    pub(super) fn invalidate_environment_review(&mut self, reason: &str) {
        self.environment_revision = self.environment_revision.wrapping_add(1);
        for conn in self.environment_grants.keys().copied().collect::<Vec<_>>() {
            self.stop_environment_review(conn, reason);
        }
    }

    pub(super) fn valid_environment_grant(
        &self,
        conn: ConnId,
        now: Instant,
    ) -> Option<&EnvironmentReviewGrant> {
        self.environment_grants.get(&conn).filter(|g| {
            g.conn == conn
                && now < g.expires
                && g.revision == self.environment_revision
                && g.profile_revision == self.profile_revision
                && g.foreground == self.foreground_id()
                && g.transport_sequence == self.ssh_foreground_sequence
                && g.remote == self.remote_policy()
                && self.process_alive
                && self.participant_allowed(conn)
                && self.conn_kind(conn) == Some(ConnKind::Agent)
        })
    }

    /// Read-only, scoped to the caller on public IPC. Reading never renews it.
    pub fn environment_review_status(
        &self,
        conn: Option<ConnId>,
        now: Instant,
    ) -> Vec<EnvironmentReviewInfo> {
        self.environment_grants
            .keys()
            .filter(|id| conn.is_none_or(|c| c == **id))
            .filter_map(|id| self.valid_environment_grant(*id, now))
            .map(|g| EnvironmentReviewInfo {
                grant_id: g.id.clone(),
                conn_id: g.conn,
                agent_id: self.agent_name(g.conn),
                remaining_ms: g.expires.saturating_duration_since(now).as_millis() as u64,
                context_revision: g.revision,
            })
            .collect()
    }

    pub(super) fn refresh_environment_review(&mut self, now: Instant) {
        match self.policy.maybe_reload() {
            Reload::Unchanged => {}
            Reload::Reloaded => {
                self.invalidate_environment_review("policy_changed");
                self.audit.record("system", "policy_reloaded", json!({}));
            }
            Reload::Failed(e) => {
                self.invalidate_environment_review("policy_changed");
                self.audit.record(
                    "system",
                    "policy_reload_failed",
                    json!({"error":e.to_string()}),
                );
                self.write_output(format!("\r\n\x1b[33mconn: policy reload failed ({e}); keeping previous policy\x1b[0m\r\n").as_bytes());
            }
        }
        let invalid: Vec<_> = self
            .environment_grants
            .iter()
            .filter_map(|(conn, g)| {
                if now >= g.expires {
                    Some((*conn, "expired"))
                } else if self.valid_environment_grant(*conn, now).is_none() {
                    Some((*conn, "context_changed"))
                } else {
                    None
                }
            })
            .collect();
        for (conn, reason) in invalid {
            self.stop_environment_review(conn, reason);
        }
    }

    fn environment_shell(&self) -> crate::backend::ShellKind {
        // Only an explicit target dialect can certify grammar. A locally selected
        // POSIX shell does not establish the dialect of an unknown foreground app.
        self.execution_profile
            .as_ref()
            .filter(|p| p.needs_review())
            .map_or(crate::backend::ShellKind::Custom, |p| p.shell)
    }

    pub(super) fn analyse_for_connection(
        &self,
        cmd: &str,
        conn: ConnId,
        now: Instant,
    ) -> crate::policy::LineAnalysis {
        if self.review_required() {
            return self.policy.analyse_external_with_environment(
                cmd,
                self.environment_shell(),
                self.valid_environment_grant(conn, now).is_some(),
            );
        }
        self.analyse(cmd)
    }

    pub(super) fn validate_scheduled_environment(&mut self, now: Instant) {
        let invalid = self
            .scheduled
            .as_ref()
            .filter(|s| s.state == ExecState::Scheduled)
            .is_some_and(|s| {
                let lease = self.authority.controller().lease();
                !self.participant_allowed(s.conn)
                    || !self.process_alive
                    || self.mode != AgentMode::Autopilot
                    || !lease.is_some_and(|l| l.conn == s.conn && now < l.expires_at)
                    || s.environment_grant.as_ref().is_some_and(|id| {
                        self.valid_environment_grant(s.conn, now)
                            .is_none_or(|g| &g.id != id)
                    })
                    || !matches!(
                        self.analyse_for_connection(&s.cmd, s.conn, now).decision,
                        PolicyDecision::Allow
                    )
            });
        if invalid {
            self.cancel_scheduled_if(|_| true, "execution_context_changed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session() -> Session {
        let (audit, _) = Audit::memory();
        let mut s = Session::new(SessionConfig {
            rows: 24,
            cols: 80,
            audit,
            policy: PolicyStore::from_policy(
                crate::policy::Policy::parse(crate::policy::EXAMPLE_POLICY).unwrap(),
            ),
            pty_writer: Box::new(Vec::<u8>::new()),
            master: None,
            pacing: Pacing::default(),
            shell_pid: None,
        });
        let mut p = crate::backend::Profile::local("test".into(), "bash".into());
        p.backend = crate::backend::BackendKind::Docker;
        s.set_execution_profile(p);
        s.register_conn(1, ConnKind::Agent, "agent", Box::new(|_| {}));
        s
    }
    #[test]
    fn monotonic_boundary_and_polling_do_not_extend_grant() {
        let mut s = session();
        let now = Instant::now();
        s.start_environment_review_at(1, now).unwrap();
        assert_eq!(
            s.environment_review_status(Some(1), now)[0].remaining_ms,
            900_000
        );
        assert_eq!(
            s.environment_review_status(Some(1), now + Duration::from_secs(899))[0].remaining_ms,
            1000
        );
        assert!(s
            .environment_review_status(Some(1), now + Duration::from_secs(900))
            .is_empty());
        s.refresh_environment_review(now + Duration::from_secs(900));
        assert!(s.environment_grants.is_empty());
    }
    #[test]
    fn observed_transport_change_revokes_without_waiting_for_a_command() {
        let mut s = session();
        s.start_environment_review(1).unwrap();
        s.set_ssh_transport(true);
        assert!(s.environment_grants.is_empty());
    }
}
