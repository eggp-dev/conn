//! conn-core — the engine behind conn.
//!
//! Embed it (`engine::Engine`) or run it as a process and talk to it over a Unix
//! socket (`ipc`). Either way the same core (`session`) arbitrates write access
//! between a human and external agents, gated by a user-declared policy.
//!
//! Layers:
//!
//! * `session`   — all state and invariants. No I/O assumptions beyond two byte sinks.
//! * `engine`    — spawns the PTY, runs reader/tick threads, exposes handles.
//! * `ipc`       — JSON-over-UDS server (for agents, human CLI, remote frontends) and client.
//! * everything else — building blocks used by `session`.
//!
//! Frontends (Tauri, a web UI, Terminal.app via the `conn` proxy) own rendering.
//! The core never draws, except for the optional in-terminal approval prompt.

pub mod backend;
pub mod profiles;
pub mod affordance;
pub mod analysis;
pub mod approval;
pub mod audit;
pub mod authority;
pub mod config;
pub mod engine;
pub mod input;
pub mod ipc;
mod transport;
pub mod paths;
pub mod policy;
pub mod screen;
pub mod session;

pub use config::Pacing;
pub use engine::{Engine, EngineConfig, LaunchSpec};
pub use session::{ServerEvent, Session, SessionConfig, SharedSession};
