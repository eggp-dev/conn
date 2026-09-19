//! What an actor may do right now. The single source of truth for both the MCP
//! adapter (`tools/list`) and the human CLI.

use serde::{Deserialize, Serialize};

use crate::authority::{ConnId, Controller};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor {
    Human,
    Agent { conn: ConnId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Affordance {
    Snapshot,
    RequestControl,
    Type,
    SendKey,
    Interrupt,
    ReleaseControl,
    CheckApproval,
    /// Human only: revoke the agent's lease.
    Take,
    /// Human only: decide a pending approval.
    Approve,
    /// Agent, when the human is not looking at this session: ask them to come back.
    RequestAttention,
    /// Agent: open a new tab (session). It starts unattended — the human decides
    /// whether to look at it.
    OpenTab,
    /// Agent: move this connection to another tab. Never moves the human's view.
    SwitchTab,
}

impl Affordance {
    pub fn name(&self) -> &'static str {
        match self {
            Affordance::Snapshot => "snapshot",
            Affordance::RequestControl => "request_control",
            Affordance::Type => "type",
            Affordance::SendKey => "send_key",
            Affordance::Interrupt => "interrupt",
            Affordance::ReleaseControl => "release_control",
            Affordance::CheckApproval => "check_approval",
            Affordance::Take => "take",
            Affordance::Approve => "approve",
            Affordance::RequestAttention => "request_attention",
            Affordance::OpenTab => "open_tab",
            Affordance::SwitchTab => "switch_tab",
        }
    }
}

pub struct AffordanceState<'a> {
    pub controller: &'a Controller,
    pub process_alive: bool,
    /// The actor (agent) has at least one pending approval.
    pub has_pending_approval: bool,
    /// Any approval is pending (relevant to the human).
    pub any_pending_approval: bool,
    /// The human is looking at this session.
    pub attended: bool,
    /// The host can open and list tabs (a hub with an opener). Headless single
    /// sessions cannot.
    pub tabs: bool,
}

pub fn affordances_for(actor: Actor, state: &AffordanceState<'_>) -> Vec<Affordance> {
    use Affordance::*;
    let mut out = vec![Snapshot];
    match actor {
        Actor::Human => {
            if !state.controller.is_human() {
                out.push(Take);
            }
            if state.any_pending_approval {
                out.push(Approve);
            }
        }
        Actor::Agent { conn } => {
            if !state.process_alive {
                return out;
            }
            if !state.attended { out.push(RequestAttention); }
            match state.controller {
                Controller::Human => out.push(RequestControl),
                Controller::Agent(l) if l.conn == conn => {
                    out.extend([Type, SendKey, Interrupt, ReleaseControl]);
                }
                Controller::Agent(_) => {}
            }
            if state.has_pending_approval && !out.contains(&CheckApproval) {
                out.push(CheckApproval);
            }
            if state.tabs {
                out.extend([OpenTab, SwitchTab]);
            }
        }
    }
    out
}
