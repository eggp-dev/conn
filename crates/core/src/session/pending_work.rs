//! Common cancellation, retaining the distinct review/proposal/grace lifecycles.
//! Runs under Session's lock; never creates its own consistency boundary.
use super::*;
impl Session {
    pub(super) fn cancel_pending_work(&mut self, conn: Option<ConnId>, reason: &str) {
        self.cancel_scheduled_if(|s| conn.is_none_or(|id| s.conn == id), reason);
        self.reject_proposal_if(|p| conn.is_none_or(|id| p.conn == id), reason);
        let approvals: Vec<_> = match conn {
            Some(id) => self.approvals.pending_for_conn(id),
            None => self
                .approvals
                .pending()
                .into_iter()
                .map(|a| a.id.clone())
                .collect(),
        };
        for id in approvals {
            self.finish_approval(&id, ApprovalState::Denied, reason);
        }
        let controls: Vec<_> = self
            .control_requests
            .iter()
            .filter(|r| {
                r.state == ControlRequestState::Pending && conn.is_none_or(|id| r.conn == id)
            })
            .map(|r| r.request_id.clone())
            .collect();
        for id in controls {
            self.finish_control_request(&id, ControlRequestState::Denied);
        }
    }
}
