//! Human-facing subcommands: status / take / approve / log.

use std::path::Path;

use serde_json::{json, Value};

use conn_core::ipc::{Client, ClientError};

fn connect(socket: &Path) -> Result<Client, ClientError> {
    let c = Client::connect(socket)?;
    c.hello("human", "cli")?;
    Ok(c)
}

pub fn status(socket: &Path) -> Result<(), ClientError> {
    let c = connect(socket)?;
    let s = c.call("status", json!({}))?;
    let controller = &s["controller"];
    match controller["type"].as_str() {
        Some("agent") => println!(
            "conn         {} has the conn ({}, expires in {}s)",
            controller["agentId"].as_str().unwrap_or("?"),
            controller["leaseId"].as_str().unwrap_or("?"),
            controller["expiresInSecs"].as_u64().unwrap_or(0)
        ),
        _ => println!("conn         you have the conn"),
    }
    println!("process      {}", if s["processAlive"].as_bool().unwrap_or(false) { "alive" } else { "exited" });
    println!("screen       {}x{} rev {}", s["size"]["cols"], s["size"]["rows"], s["revision"]);
    let agents: Vec<&str> = s["connectedAgents"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
    println!("agents       {}", if agents.is_empty() { "-".to_string() } else { agents.join(", ") });
    if let Some(p) = s["policyPath"].as_str() {
        println!("policy       {p}");
    }
    let allows: Vec<&str> = s["sessionAllows"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
    if !allows.is_empty() {
        println!("session allow {}", allows.join(", "));
    }
    let fronts: Vec<&str> = s["connectedFrontends"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
    if !fronts.is_empty() {
        println!("frontends    {}", fronts.join(", "));
    }
    let p = &s["pacing"];
    println!(
        "pacing       write≥{}ms  grace {}ms  lease {}s  approval {}s",
        p["minWriteIntervalMs"], p["enterGraceMs"], p["leaseTtlSecs"], p["approvalTtlSecs"]
    );
    if let Some(m) = s["affordanceMask"].as_array() {
        println!("agent mask   {}", m.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(" "));
    }
    if let Some(sc) = s["scheduled"].as_object() {
        println!("scheduled    {}  {}  ({})", sc["execId"].as_str().unwrap_or("?"), sc["cmd"].as_str().unwrap_or("?"), sc["agentId"].as_str().unwrap_or("?"));
    }
    println!("mode         {}{}{}", s["mode"].as_str().unwrap_or("?"), if s["controlGate"].as_bool().unwrap_or(false) { "  (control gate: ask)" } else { "" }, if s["attended"].as_bool().unwrap_or(true) { "" } else { "  (unattended)" });
    if let Some(a) = s["entrustedTo"].as_str() { println!("entrusted    {a} (cap: {})", s["effectiveMode"].as_str().unwrap_or("?")); }
    if let Some(a) = s["attentionRequest"]["agentId"].as_str() { println!("attention    {a} asks: {}", s["attentionRequest"]["reason"].as_str().unwrap_or("-")); }
    if let Some(p) = s["proposal"].as_object() {
        println!("proposal     {}  {}  [{}]", p["proposalId"].as_str().unwrap_or("?"), p["text"].as_str().unwrap_or(""), p["state"].as_str().unwrap_or("?"));
    }
    for r in s["controlRequests"].as_array().cloned().unwrap_or_default() {
        println!("control req  {}  {}  {}", r["requestId"].as_str().unwrap_or("?"), r["agentId"].as_str().unwrap_or("?"), r["reason"].as_str().unwrap_or("-"));
    }
    let pending = s["pending"].as_array().cloned().unwrap_or_default();
    if pending.is_empty() {
        println!("pending      none");
    } else {
        println!("pending");
        for p in pending {
            println!(
                "  {}  [{}]  {}  ({} · {})",
                p["id"].as_str().unwrap_or("?"),
                p["label"].as_str().unwrap_or("?"),
                p["cmd"].as_str().unwrap_or("?"),
                p["agentId"].as_str().unwrap_or("?"),
                p["requestedAt"].as_str().unwrap_or("?")
            );
            if let Some(i) = p["intent"].as_str() { println!("      intent  {i}"); }
            for s in p["analysis"]["segments"].as_array().into_iter().flatten() {
                for t in s["targets"].as_array().into_iter().flatten() {
                    println!("      target  {}{}{}{}", t["path"].as_str().unwrap_or("?"), if t["gitRepo"].as_bool().unwrap_or(false) { " · git" } else { "" }, t["entries"].as_u64().map(|n| format!(" · {n} entries")).unwrap_or_default(), if t["protected"].as_bool().unwrap_or(false) { " · protected path" } else { "" });
                }
            }
        }
    }
    Ok(())
}

pub fn take(socket: &Path) -> Result<(), ClientError> {
    let c = connect(socket)?;
    let r = c.call("take", json!({}))?;
    match r["revoked"].as_str() {
        Some(l) => println!("you have the conn (revoked {l})"),
        None => println!("you already have the conn"),
    }
    Ok(())
}

pub fn approve(socket: &Path, id: Option<String>, deny: bool, allow_session: bool) -> Result<(), ClientError> {
    let c = connect(socket)?;
    let Some(id) = id else {
        // Never approve "whatever is pending": show what is waiting and require an id.
        let st = c.call("status", json!({}))?;
        let pending = st["pending"].as_array().cloned().unwrap_or_default();
        if pending.is_empty() {
            println!("no pending approval");
        } else {
            println!("pending approvals — pass an id: conn approve <id> [-d|-A]");
            for p in pending {
                println!("  {}  [{}]  {}  ({} · {})", p["id"].as_str().unwrap_or("?"), p["label"].as_str().unwrap_or("?"), p["cmd"].as_str().unwrap_or("?"), p["agentId"].as_str().unwrap_or("?"), p["requestedAt"].as_str().unwrap_or("?"));
                if let Some(i) = p["intent"].as_str() { println!("      intent  {i}"); }
                for s in p["analysis"]["segments"].as_array().into_iter().flatten() {
                    for t in s["targets"].as_array().into_iter().flatten() {
                        println!("      target  {}{}{}", t["path"].as_str().unwrap_or("?"), if t["gitRepo"].as_bool().unwrap_or(false) { " · git" } else { "" }, t["entries"].as_u64().map(|n| format!(" · {n} entries")).unwrap_or_default());
                    }
                }
            }
        }
        return Err(ClientError::Rpc { code: "invalid_input".into(), message: "approval id required".into() });
    };
    let decision = if deny { "deny" } else if allow_session { "allow_session" } else { "grant" };
    let r = c.call("approve", json!({ "approvalId": id, "decision": decision }))?;
    println!(
        "{}  {}  [{}]  {}",
        r["approvalId"].as_str().unwrap_or("?"),
        r["state"].as_str().unwrap_or("?"),
        r["label"].as_str().unwrap_or("?"),
        r["cmd"].as_str().unwrap_or("?")
    );
    Ok(())
}

pub fn pacing(socket: &Path, min: Option<u64>, grace: Option<u64>, lease: Option<u64>, approval: Option<u64>) -> Result<(), ClientError> {
    let c = connect(socket)?;
    let mut params = serde_json::Map::new();
    if let Some(v) = min { params.insert("minWriteIntervalMs".into(), json!(v)); }
    if let Some(v) = grace { params.insert("enterGraceMs".into(), json!(v)); }
    if let Some(v) = lease { params.insert("leaseTtlSecs".into(), json!(v)); }
    if let Some(v) = approval { params.insert("approvalTtlSecs".into(), json!(v)); }
    let r = if params.is_empty() { c.call("get_pacing", json!({}))? } else { c.call("set_pacing", Value::Object(params))? };
    println!("{}", serde_json::to_string_pretty(&r).unwrap());
    Ok(())
}

pub fn affordances(socket: &Path, allow: Vec<String>, clear: bool) -> Result<(), ClientError> {
    let c = connect(socket)?;
    let r = if clear {
        c.call("set_affordances", json!({ "allow": null }))?
    } else if allow.is_empty() {
        let s = c.call("status", json!({}))?;
        json!({ "allow": s["affordanceMask"] })
    } else {
        c.call("set_affordances", json!({ "allow": allow }))?
    };
    match r["allow"].as_array() {
        Some(a) => println!("agent affordances restricted to: {}", a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(" ")),
        None => println!("no restriction (state-derived affordances only)"),
    }
    Ok(())
}

pub fn mode(socket: &Path, mode: Option<String>) -> Result<(), ClientError> {
    let c = connect(socket)?;
    let r = match mode {
        Some(m) => c.call("set_mode", json!({ "mode": m }))?,
        None => json!({ "mode": c.call("status", json!({}))?["mode"] }),
    };
    println!("mode {}", r["mode"].as_str().unwrap_or("?"));
    Ok(())
}

pub fn gate(socket: &Path, ask: Option<bool>) -> Result<(), ClientError> {
    let c = connect(socket)?;
    let r = match ask {
        Some(a) => c.call("set_control_gate", json!({ "ask": a }))?,
        None => json!({ "ask": c.call("status", json!({}))?["controlGate"] }),
    };
    println!("control gate: {}", if r["ask"].as_bool().unwrap_or(false) { "ask (human decides each request_control)" } else { "auto" });
    Ok(())
}

pub fn sessions(socket: &Path) -> Result<(), ClientError> {
    let c = connect(socket)?;
    let r = c.call("sessions", json!({}))?;
    for s in r["sessions"].as_array().cloned().unwrap_or_default() {
        let ctl = if s["controller"]["type"] == "agent" { format!("{} has the conn", s["controller"]["agentId"].as_str().unwrap_or("?")) } else { "you have the conn".into() };
        println!("{} {:<10} {}{}{}{}", if s["attended"].as_bool().unwrap_or(false) { "▶" } else { " " }, s["id"].as_str().unwrap_or("?"), ctl,
            if s["pending"].as_u64().unwrap_or(0) > 0 { format!("  · pending {}", s["pending"]) } else { String::new() },
            s["entrustedTo"].as_str().map(|a| format!("  · entrusted to {a}")).unwrap_or_default(),
            s["attentionRequest"]["agentId"].as_str().map(|a| format!("  · {a} asks for attention")).unwrap_or_default());
    }
    Ok(())
}

pub fn attend(socket: &Path, id: &str) -> Result<(), ClientError> {
    let c = connect(socket)?;
    let r = c.call("set_attended", json!({ "session": id }))?;
    println!("attending {}", r["attended"].as_str().unwrap_or("?"));
    Ok(())
}

pub fn entrust(socket: &Path, session: Option<String>) -> Result<(), ClientError> {
    let c = connect(socket)?;
    let r = c.call("entrust", json!({ "session": session }))?;
    println!("entrusted to {} while you are away", r["entrustedTo"].as_str().unwrap_or("?"));
    Ok(())
}

pub fn handback(socket: &Path) -> Result<(), ClientError> {
    let c = connect(socket)?;
    let r = c.call("hand_back", json!({}))?;
    println!("{} has the conn ({})", r["agentId"].as_str().unwrap_or("?"), r["leaseId"].as_str().unwrap_or("?"));
    Ok(())
}

pub fn decide(socket: &Path, id: Option<String>, deny: bool) -> Result<(), ClientError> {
    let c = connect(socket)?;
    let id = match id {
        Some(id) => id,
        None => {
            let st = c.call("status", json!({}))?;
            st["controlRequests"][0]["requestId"].as_str().map(|s| s.to_string()).ok_or_else(|| ClientError::Rpc { code: "not_found".into(), message: "no pending control request".into() })?
        }
    };
    let r = c.call("decide_control", json!({ "requestId": id, "grant": !deny }))?;
    println!("{}  {}  ({})", r["requestId"].as_str().unwrap_or("?"), r["state"].as_str().unwrap_or("?"), r["agentId"].as_str().unwrap_or("?"));
    Ok(())
}

pub const GUIDE: &str = include_str!("../../../plugin/skills/conn/SKILL.md");

pub fn guide() {
    // strip the skill frontmatter; the body is the guide
    let body = GUIDE.splitn(3, "---").nth(2).unwrap_or(GUIDE).trim_start();
    println!("{body}");
    println!("\n## Without MCP tools (conn agent)\n");
    println!("The same procedure from a shell: `conn agent snapshot | request [--reason ..] | type <text> | enter [--intent ..] | key <KEY> | interrupt | check <approvalId> | release | tabs | open-tab [--reason ..] | switch-tab <n>`");
    println!("Name yourself with `--agent-id`. Every call goes through the same socket and lands in the audit log.");
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
