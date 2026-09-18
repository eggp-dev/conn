//! Agent-side operations and explicit local-file inspection. Owner controls live in the app.
use std::path::Path;
use serde_json::{json, Value};
use conn_core::ipc::{Client, ClientError};
fn connect(socket: &Path) -> Result<Client, ClientError> {
    let c = Client::connect(socket)?;
    c.hello("agent", "cli")?;
    Ok(c)
}

pub fn sessions(socket: &Path) -> Result<(), ClientError> {
    let c = connect(socket)?;
    let r = c.call("list_tabs", json!({}))?;
    for s in r["sessions"].as_array().cloned().unwrap_or_default() {
        let ctl = if s["controller"]["type"] == "agent" { format!("{} has the conn", s["controller"]["agentId"].as_str().unwrap_or("?")) } else { "you have the conn".into() };
        println!("{} {:<10} {}{}{}", if s["attended"].as_bool().unwrap_or(false) { "▶" } else { " " }, s["id"].as_str().unwrap_or("?"), ctl,
            if s["pending"].as_u64().unwrap_or(0) > 0 { format!("  · pending {}", s["pending"]) } else { String::new() },
            s["attentionRequest"]["agentId"].as_str().map(|a| format!("  · {a} asks for attention")).unwrap_or_default());
    }
    Ok(())
}

pub const GUIDE: &str = include_str!("../../../plugin/skills/conn/SKILL.md");

pub fn guide() {
    // strip the skill frontmatter; the body is the guide
    let body = GUIDE.splitn(3, "---").nth(2).unwrap_or(GUIDE).trim_start();
    println!("{body}");
    println!("\n## Without MCP tools (conn agent)\n");
    println!("The same procedure from a shell: `conn agent snapshot | request [--reason ..] | type <text> | enter [--intent ..] | key <KEY> | interrupt | check <approvalId> | release | tabs | open-tab [--reason ..] | switch-tab <n>`");
    println!("Name yourself with `--agent-id`. Every call uses the same agent socket and participation checks.");
}

/// Thin wrapper over the socket for agents that only have a shell tool.
pub fn agent(socket: &Path, agent_id: &str, sub: &str, args: &[String]) -> Result<(), ClientError> {
    let c = Client::connect(socket)?;
    c.hello("agent", agent_id)?;
    let (method, params) = match sub {
        "snapshot" => ("snapshot", json!({})),
        "request" => {
            let reason = args.iter().position(|a| a == "--reason").and_then(|i| args.get(i + 1)).cloned();
            ("request_control", json!({ "reason": reason, "command": args.iter().position(|a| a == "--command").and_then(|i| args.get(i + 1)) }))
        }
        "type" => ("type", json!({ "text": args.join(" ") })),
        "enter" => {
            let intent = args.iter().position(|a| a == "--intent").and_then(|i| args.get(i + 1)).cloned();
            ("send_key", json!({ "key": "ENTER", "intent": intent }))
        }
        "key" => ("send_key", json!({ "key": args.first().cloned().unwrap_or_default() })),
        "interrupt" => ("interrupt", json!({})),
        "check" => ("check_approval", json!({ "approvalId": args.first().cloned().unwrap_or_default() })),
        "release" => ("release_control", json!({})),
        "affordances" => ("affordances", json!({})),
        "tabs" => ("list_tabs", json!({})),
        "open-tab" => {
            let reason = args.iter().position(|a| a == "--reason").and_then(|i| args.get(i + 1)).cloned();
            ("open_tab", json!({ "reason": reason }))
        }
        "switch-tab" => {
            let tab = args.first().cloned().unwrap_or_default();
            ("switch_tab", json!({ "tab": tab.parse::<u64>().map(Value::from).unwrap_or(Value::String(tab)) }))
        }
        other => return Err(ClientError::Rpc { code: "invalid_input".into(), message: format!("unknown agent action '{other}'") }),
    };
    // Note: the lease is bound to this connection. `type`/`enter` therefore need
    // `request` in the same process — for one-shot CLI use, chain them: `agent run`.
    let r = c.call(method, params)?;
    println!("{}", serde_json::to_string_pretty(&r).unwrap());
    Ok(())
}

/// One-shot: request → type → enter → wait, in a single connection so the lease holds.
pub fn agent_run(socket: &Path, agent_id: &str, reason: Option<String>, text: &str, release: bool) -> Result<(), ClientError> {
    let c = Client::connect(socket)?;
    c.hello("agent", agent_id)?;
    let r = c.call("request_control", json!({ "reason": reason, "command": text }))?;
    eprintln!("control: {}", r["status"].as_str().or(r["type"].as_str()).unwrap_or("?"));
    c.call("type", json!({ "text": text }))?;
    let r = c.call("send_key", json!({ "key": "ENTER", "intent": reason }))?;
    let mut status = r["status"].as_str().unwrap_or("?").to_string();
    if status == "pending" {
        let id = r["approvalId"].as_str().unwrap_or_default().to_string();
        eprintln!("pending approval {id}; waiting for the human…");
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let a = c.call("check_approval", json!({ "approvalId": id }))?;
            let s = a["state"].as_str().unwrap_or("?");
            if s != "pending" { status = format!("approval {s}"); break; }
        }
    }
    eprintln!("result: {status}");
    std::thread::sleep(std::time::Duration::from_millis(400));
    let snap = c.call("snapshot", json!({}))?;
    for line in snap["screen"].as_array().into_iter().flatten().filter_map(|v| v.as_str()) {
        println!("{line}");
    }
    if release { let _ = c.call("release_control", json!({})); }
    Ok(())
}

pub fn log(path: &Path, n: usize, actor: Option<String>, raw: bool) -> std::io::Result<()> {
    let events = match conn_core::audit::read_events(path) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!("no audit log at {}", path.display());
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    let filtered: Vec<_> = events
        .into_iter()
        .filter(|e| actor.as_deref().map(|a| e.actor == a).unwrap_or(true))
        .collect();
    let start = filtered.len().saturating_sub(n);
    for e in &filtered[start..] {
        if raw {
            println!("{}", serde_json::to_string(e).unwrap());
            continue;
        }
        let detail = summarize(&e.action, &e.fields);
        println!("{}  {:<8}  {:<18}  {}", e.ts, e.actor, e.action, detail);
    }
    Ok(())
}

fn summarize(action: &str, f: &serde_json::Map<String, Value>) -> String {
    let g = |k: &str| f.get(k).map(|v| match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    });
    match action {
        "exec" => {
            let mut s = g("cmd").unwrap_or_default();
            if let Some(p) = g("policy") {
                s.push_str(&format!("  [{p}"));
                if let Some(a) = g("approval") {
                    s.push_str(&format!(" → {a}"));
                }
                s.push(']');
            }
            if let Some(i) = g("intent") {
                if i != "null" { s.push_str(&format!("  — {i}")); }
            }
            s
        }
        "takeover" => format!("revoked {}", g("revoked").unwrap_or_default()),
        "observe" => format!("rev {}", g("rev").unwrap_or_default()),
        "approval_requested" => format!("{} [{}] {}{}", g("approval").unwrap_or_default(), g("label").unwrap_or_default(), g("cmd").unwrap_or_default(), g("intent").map(|i| format!("  — {i}")).unwrap_or_default()),
        _ => {
            let parts: Vec<String> = f.iter().map(|(k, v)| format!("{k}={v}")).collect();
            parts.join(" ")
        }
    }
}
