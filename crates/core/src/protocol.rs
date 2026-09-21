//! Transport-neutral request/result contracts shared by adapters.
use crate::{authority::AuthorityError, session::SessionError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl From<SessionError> for RpcError {
    fn from(e: SessionError) -> Self {
        let code = match &e {
            SessionError::Authority(AuthorityError::Busy { .. }) => "busy",
            SessionError::Authority(AuthorityError::NotController) => "not_controller",
            SessionError::Authority(AuthorityError::Expired) => "lease_expired",
            SessionError::ProcessExited => "process_exited",
            SessionError::InputPending => "input_pending",
            SessionError::InvalidInput(_) => "invalid_input",
            SessionError::NotFound(_) => "not_found",
            SessionError::ApprovalPending(_) => "approval_pending",
            SessionError::ExecPending(_) => "exec_pending",
            SessionError::RateLimited { .. } => "rate_limited",
            SessionError::Masked(_) => "masked",
            SessionError::WrongMode(_) => "wrong_mode",
            SessionError::ProposalPending(_) => "proposal_pending",
            SessionError::IntentRequired => "intent_required",
            SessionError::NotAvailable(_) => "not_available",
            SessionError::SurfaceUnavailable => "surface_unavailable",
        };
        RpcError {
            code: code.into(),
            message: e.to_string(),
        }
    }
}
