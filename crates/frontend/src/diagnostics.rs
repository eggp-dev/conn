//! Live socket facts are independent from local integration configuration.
use std::collections::BTreeMap;

use conn_core::{ipc::SharedHub, session::AgentConnection};
use serde_json::{json, Value};

pub(crate) fn connections(hub: &SharedHub) -> Vec<Value> {
    merge(hub.ids().into_iter().filter_map(|id| {
        hub.get(&id).map(|s| (id, s.lock().status().agent_connections))
    }))
}

fn merge(sessions: impl IntoIterator<Item = (String, Vec<AgentConnection>)>) -> Vec<Value> {
    // One socket can be registered with multiple tabs. Count it once and retain
    // its tab registrations; never infer disconnection from an idle duration.
    let mut connections = BTreeMap::new();
    for (session, agents) in sessions {
        for agent in agents {
            let row = connections.entry(agent.conn_id).or_insert_with(|| {
                (agent.agent_id, agent.idle_secs, Vec::<String>::new())
            });
            row.1 = row.1.min(agent.idle_secs);
            row.2.push(session.clone());
        }
    }
    connections.into_iter().map(|(id, (agent, idle, mut sessions))| {
        sessions.sort(); sessions.dedup();
        json!({ "connId": id, "agentId": agent, "idleSecs": idle, "sessions": sessions })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_socket_on_two_tabs_is_counted_once_but_idle_connections_remain_visible() {
        let row = |conn_id, idle_secs| AgentConnection { conn_id, agent_id: "copilot".into(), idle_secs };
        let rows = merge([
            ("t1".into(), vec![row(1, 720), row(2, 1)]),
            ("t2".into(), vec![row(2, 0)]),
        ]);
        assert_eq!(rows, vec![
            json!({"connId":1,"agentId":"copilot","idleSecs":720,"sessions":["t1"]}),
            json!({"connId":2,"agentId":"copilot","idleSecs":0,"sessions":["t1","t2"]}),
        ]);
    }
}
