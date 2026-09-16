//! MCP stdio adapter. A thin bridge from JSON-RPC 2.0 on stdin/stdout to the
//! proxy's Unix socket. Implements no permission logic: `tools/list` is whatever
//! the core says this agent may do right now.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use conn_core::ipc::{Client, ClientError};

pub const INSTRUCTIONS: &str = "Use the conn tools for all shell work. Do not use built-in shell tools.

A session (tab) is already open; work in it. Open a new tab with terminal_open_tab only when you truly need a separate shell.

Call terminal_snapshot before and after running a command, check the screen, and report what you saw.

Before writing, obtain control with terminal_request_control. Put what you intend to do in `reason`, in one line. Include the exact planned shell command in `command` when known, so it can be reviewed and retained even if control is denied. This metadata does not execute or authorize the command.

In copilot mode the line you type is shown as a proposal and the human runs it with ENTER. Wait until send_key(ENTER) returns `executed` or `rejected`. When the human touches the keyboard, control is revoked immediately and the write tools disappear; request control again.

terminal_type only types; it does not run. Running is terminal_send_key(ENTER, intent). `intent` is required.

One dangerous command per line. Chaining it with && ; | to other commands is denied. If you need a cd, run the cd separately first.

If status is `pending`, wait by polling terminal_check_approval. Do not work around it.

A human shares this session. Commands you did not run may appear on the screen.

You only see what the human is looking at. When the human moves to another tab, snapshot and writes are refused with `unattended` / `suspended`. Ask them to come back with terminal_request_attention and wait. If the human entrusts the session to you, you may continue within the policy's `unattended` cap.

Modes: snapshot and terminal_list_tabs report mode and effectiveMode. In copilot mode typing creates a proposal; tell the human to press Enter to accept it. Mode can change between calls.
Tabs: terminal_list_tabs shows the tabs and which one the human is looking at. A tab you open with terminal_open_tab is not being watched yet: you can neither see nor write there until the human comes to it or entrusts it — open it, then wait. terminal_switch_tab moves only your connection; it never moves the human's view.";

struct ToolDef {
    name: &'static str,
    affordance: &'static str,
    method: &'static str,
    description: &'static str,
    schema: fn() -> Value,
}

const TOOLS: &[ToolDef] = &[
    ToolDef {
        name: "terminal_snapshot",
        affordance: "snapshot",
        method: "snapshot",
        description: "Returns the current terminal screen — exactly what the human sees. No scrollback.",
        schema: || json!({ "type": "object", "properties": {}, "additionalProperties": false }),
    },
    ToolDef {
        name: "terminal_request_control",
        affordance: "request_control",
        method: "request_control",
        description: "Requests write control (a lease). Put what you intend to do in `reason`, one line — it is shown to the human, and depending on settings the call returns only after the human allows it. Any human keystroke revokes control immediately.",
        schema: || json!({ "type": "object", "properties": { "reason": { "type": "string", "description": "What you are about to do, one line (shown to the human)" }, "command": { "type": "string", "description": "Exact planned shell command, including arguments, for review and audit; does not execute it" }, "agentId": { "type": "string", "description": "optional, informational" } }, "additionalProperties": false }),
    },
    ToolDef {
        name: "terminal_type",
        affordance: "type",
        method: "type",
        description: "Appends text to the input line. Does not run it (no ENTER). Newlines are not allowed. In copilot mode the text does not reach the shell; it is shown to the human as a proposal (ghost text).",
        schema: || json!({ "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"], "additionalProperties": false }),
    },
    ToolDef {
        name: "terminal_send_key",
        affordance: "send_key",
        method: "send_key",
        description: "Sends a key. ENTER goes through policy: status comes back as executed / cancelled / rejected / denied / pending. ENTER requires `intent` — one line saying what the command does and changes — which is shown to the human and written to the audit log. Run dangerous commands (delete, privilege escalation, force push …) alone on a line; chaining them with && or ; returns denied. If pending, wait with terminal_check_approval.",
        schema: || json!({ "type": "object", "properties": { "key": { "type": "string", "enum": conn_core::session::SUPPORTED_KEYS }, "intent": { "type": "string", "description": "Required with ENTER. One line: what this command does and what it changes" } }, "required": ["key"], "additionalProperties": false }),
    },
    ToolDef {
        name: "terminal_interrupt",
        affordance: "interrupt",
        method: "interrupt",
        description: "Sends Ctrl-C and clears the input line. A pending approval is cancelled.",
        schema: || json!({ "type": "object", "properties": {}, "additionalProperties": false }),
    },
    ToolDef {
        name: "terminal_check_approval",
        affordance: "check_approval",
        method: "check_approval",
        description: "Polls an approval: pending / granted / denied / expired.",
        schema: || json!({ "type": "object", "properties": { "approvalId": { "type": "string" } }, "required": ["approvalId"], "additionalProperties": false }),
    },
    ToolDef {
        name: "terminal_request_attention",
        affordance: "request_attention",
        method: "request_attention",
        description: "Asks the human to come back to this session (tab) when they are not looking at it. While they are away you can neither see nor write (unattended / suspended). You can continue once they return or entrust the session to you.",
        schema: || json!({ "type": "object", "properties": { "reason": { "type": "string", "description": "Why they should look, one line" } }, "additionalProperties": false }),
    },
    ToolDef {
        name: "terminal_list_tabs",
        affordance: "*",
        method: "list_tabs",
        description: "Lists the tabs (sessions): number, mode, effectiveMode (including unattended cap), whether the human is looking at it (attended), who has the conn, who opened it, and which one your connection is bound to (current). No screen contents.",
        schema: || json!({ "type": "object", "properties": {}, "additionalProperties": false }),
    },
    ToolDef {
        name: "terminal_open_tab",
        affordance: "open_tab",
        method: "open_tab",
        description: "Opens a new tab (shell) and moves your connection to it. The human is not looking at the new tab yet: snapshot and writes are refused (unattended) until they come to it or entrust it to you. `reason` is shown on the tab. Open one only when you really need it.",
        schema: || json!({ "type": "object", "properties": { "reason": { "type": "string", "description": "Why you need a new tab, one line (shown to the human)" } }, "additionalProperties": false }),
    },
    ToolDef {
        name: "terminal_switch_tab",
        affordance: "switch_tab",
        method: "switch_tab",
        description: "Moves your connection to another tab. It never moves the human's view. In a tab the human is not looking at you still cannot see or write. `tab` is a number or id from terminal_list_tabs.",
        schema: || json!({ "type": "object", "properties": { "tab": { "type": ["integer", "string"], "description": "Tab number (from 1) or session id" } }, "required": ["tab"], "additionalProperties": false }),
    },
    ToolDef {
        name: "terminal_release_control",
        affordance: "release_control",
        method: "release_control",
        description: "Gives write control back.",
        schema: || json!({ "type": "object", "properties": {}, "additionalProperties": false }),
    },
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolMode {
    /// tools/list follows the affordance table; changes are announced with
    /// notifications/tools/list_changed. Needs a harness that re-lists on that notification.
    Dynamic,
    /// Always advertise every tool; a call the agent may not make right now returns a
    /// clear error. For harnesses that ignore list_changed (Codex).
    Static,
    /// Decide from the MCP client's name at initialize.
    Auto,
}

struct Bridge {
    socket: PathBuf,
    agent_id: String,
    agent_id_fixed: bool,
    tool_mode: ToolMode,
    client: Option<Arc<Client>>,
    out: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl Bridge {
    fn ensure(&mut self) -> Result<Arc<Client>, ClientError> {
        if let Some(c) = &self.client {
            return Ok(c.clone());
        }
        let client = Arc::new(Client::connect(&self.socket)?);
        client.hello("agent", &self.agent_id)?;
        if let Some(events) = client.take_events() {
            let out = self.out.clone();
            std::thread::spawn(move || {
                while let Ok(ev) = events.recv() {
                    let kind = ev.get("event").and_then(|v| v.as_str()).unwrap_or("");
                    match kind {
                        "control_handed_back" => {
                            write_msg(&out, &json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" }));
                            write_msg(&out, &json!({ "jsonrpc": "2.0", "method": "notifications/message",
                                "params": { "level": "info", "logger": "conn",
                                            "data": format!("the human handed control back to you{}; continue",
                                                            ev.get("lastCmd").and_then(|v| v.as_str()).map(|c| format!(" (last: {c})")).unwrap_or_default()) } }));
                        }
                        "mode_changed" => {
                            write_msg(&out, &json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" }));
                            write_msg(&out, &json!({ "jsonrpc": "2.0", "method": "notifications/message",
                                "params": { "level": "info", "logger": "conn", "data": {
                                    "event": "mode_changed", "session": ev["session"], "mode": ev["mode"], "effectiveMode": ev["effectiveMode"]
                                } } }));
                        }
                        "tools_changed" | "control_revoked" => {
                            write_msg(&out, &json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" }));
                        }
                        _ => {}
                    }
                    if kind == "control_revoked" {
                        write_msg(
                            &out,
                            &json!({
                                "jsonrpc": "2.0",
                                "method": "notifications/message",
                                "params": { "level": "warning", "logger": "conn",
                                            "data": format!("control revoked ({}); write tools are no longer available",
                                                            ev.get("reason").and_then(|v| v.as_str()).unwrap_or("?")) }
                            }),
                        );
                    }
                }
                // proxy went away: tools changed too
                write_msg(&out, &json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" }));
            });
        }
        self.client = Some(client.clone());
        Ok(client)
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, ClientError> {
        let client = self.ensure()?;
        match client.call(method, params) {
            Err(ClientError::Disconnected) => {
                self.client = None;
                Err(ClientError::Disconnected)
            }
            other => other,
        }
    }

    fn tools_list(&mut self) -> Value {
        let allowed: Vec<String> = match self.call("affordances", json!({})) {
            Ok(v) => serde_json::from_value(v).unwrap_or_default(),
            Err(_) => vec![],
        };
        let static_mode = self.tool_mode == ToolMode::Static;
        let tools: Vec<Value> = TOOLS
            .iter()
            .filter(|t| static_mode || t.affordance == "*" || allowed.iter().any(|a| a == t.affordance))
            .map(|t| {
                let avail = t.affordance == "*" || allowed.iter().any(|a| a == t.affordance);
                let desc = if static_mode && !avail { format!("{} (not available right now — becomes available when the state changes)", t.description) } else { t.description.to_string() };
                json!({ "name": t.name, "description": desc, "inputSchema": (t.schema)() })
            })
            .collect();
        json!({ "tools": tools })
    }

    fn tools_call(&mut self, name: &str, args: Value) -> Value {
        let Some(tool) = TOOLS.iter().find(|t| t.name == name) else {
            return tool_error(format!("unknown tool '{name}'"));
        };
        match self.call(tool.method, args) {
            Ok(v) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap() }] }),
            Err(ClientError::Rpc { code, message }) => {
                let hint = match code.as_str() {
                    "not_controller" | "lease_expired" => " — call terminal_request_control first (tools/list has changed)",
                    "masked" => " — the frontend disabled this affordance for agents",
                    "exec_pending" => " — a previous ENTER is still in its grace window",
                    "busy" => " — another agent holds control; use terminal_snapshot only",
                    "approval_pending" => " — wait with terminal_check_approval or call terminal_interrupt",
                    "control_denied" => " — the human declined to hand over control; observe with terminal_snapshot and ask again later with a clearer reason",
                    "wrong_mode" => " — the frontend put agents in observe mode",
                    "proposal_pending" => " — your proposal is waiting for the human to commit or reject",
                    "intent_required" => " — resend ENTER with an `intent` (one line: what this command does and changes)",
                    "unattended" => " — the human is not looking at this session; call terminal_request_attention and wait",
                    "suspended" => " — the human left this session; wait for control_resumed (they return or entrust it to you)",
                    _ => "",
                };
                tool_error(format!("{code}: {message}{hint}"))
            }
            Err(e) => tool_error(e.to_string()),
        }
    }
}

fn tool_error(msg: String) -> Value {
    json!({ "content": [{ "type": "text", "text": msg }], "isError": true })
}

fn write_msg(out: &Arc<Mutex<Box<dyn Write + Send>>>, v: &Value) {
    if let Ok(mut o) = out.lock() {
        let _ = o.write_all(v.to_string().as_bytes());
        let _ = o.write_all(b"\n");
        let _ = o.flush();
    }
}

/// Run the MCP server on stdin/stdout until EOF.
/// Derive a short agent id from the MCP client's name ("Claude Code" → "claude",
/// "GitHub Copilot CLI" → "copilot").
fn agent_id_from_client(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    for known in ["claude", "copilot", "codex", "gemini", "cursor", "cline", "windsurf"] {
        if n.contains(known) {
            return known.into();
        }
    }
    let id: String = n.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    id.trim_matches('-').split('-').find(|s| !s.is_empty()).unwrap_or("agent").to_string()
}

pub fn run(socket: PathBuf, agent_id: Option<String>, tool_mode: ToolMode) -> std::io::Result<()> {
    let out: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(Box::new(std::io::stdout())));
    let agent_id_fixed = agent_id.is_some();
    let bridge = Arc::new(Mutex::new(Bridge { socket, agent_id: agent_id.unwrap_or_else(|| "agent".into()), agent_id_fixed, tool_mode, client: None, out: out.clone() }));

    // The proxy may start after us (or restart). Keep trying to attach in the
    // background and tell the harness to re-list tools the moment we connect.
    {
        let bridge = bridge.clone();
        let out = out.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let mut b = bridge.lock().unwrap();
            if b.client.is_none() && b.ensure().is_ok() {
                write_msg(&out, &json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" }));
            }
        });
    }

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                write_msg(&out, &json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } }));
                continue;
            }
        };
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let mut bridge = bridge.lock().unwrap();

        let result: Result<Value, (i64, String)> = match method {
            "initialize" => {
                let client_name = params.get("clientInfo").and_then(|c| c.get("name")).and_then(|n| n.as_str()).unwrap_or("").to_string();
                if !bridge.agent_id_fixed && !client_name.is_empty() {
                    bridge.agent_id = agent_id_from_client(&client_name);
                }
                if bridge.tool_mode == ToolMode::Auto {
                    // Codex does not re-list tools on notifications/tools/list_changed.
                    let n = client_name.to_ascii_lowercase();
                    bridge.tool_mode = if n.contains("codex") || n.contains("openai") { ToolMode::Static } else { ToolMode::Dynamic };
                    eprintln!("[conn mcp] client={client_name:?} agent={} tools={:?}", bridge.agent_id, bridge.tool_mode);
                }
                // Connect now that we know who we are; tolerate the proxy not being up yet.
                let _ = bridge.ensure();
                let requested = params.get("protocolVersion").and_then(|v| v.as_str()).unwrap_or("2025-06-18");
                let version = if ["2024-11-05", "2025-03-26", "2025-06-18"].contains(&requested) { requested } else { "2025-06-18" };
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": true }, "logging": {} },
                    "serverInfo": { "name": "conn", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": INSTRUCTIONS
                }))
            }
            "notifications/initialized" | "notifications/cancelled" | "notifications/roots/list_changed" => continue,
            "ping" => Ok(json!({})),
            "tools/list" => Ok(bridge.tools_list()),
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                Ok(bridge.tools_call(name, args))
            }
            "resources/list" => Ok(json!({ "resources": [] })),
            "prompts/list" => Ok(json!({ "prompts": [] })),
            "logging/setLevel" => Ok(json!({})),
            _ if id.is_none() => continue,
            other => Err((-32601, format!("method not found: {other}"))),
        };
        let Some(id) = id else { continue };
        let resp = match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
        };
        write_msg(&out, &resp);
    }
    Ok(())
}
