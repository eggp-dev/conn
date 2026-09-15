//! Runtime-tunable behaviour. A frontend may change these at any time.

use serde::{Deserialize, Serialize};

/// Timing knobs that govern agent control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Pacing {
    /// Minimum interval between two agent writes (type / send_key). 0 = unlimited.
    /// Writes that arrive early are delayed by the transport, never dropped.
    pub min_write_interval_ms: u64,
    /// After the policy allows an ENTER, wait this long before actually sending it.
    /// Any human input during the window cancels the execution. 0 = immediate.
    pub enter_grace_ms: u64,
    /// Lease TTL. Every agent write renews it.
    pub lease_ttl_secs: u64,
    /// How long an approval request stays pending before it is treated as denied.
    pub approval_ttl_secs: u64,
}

impl Default for Pacing {
    fn default() -> Self {
        Self { min_write_interval_ms: 0, enter_grace_ms: 0, lease_ttl_secs: 60, approval_ttl_secs: 300 }
    }
}
