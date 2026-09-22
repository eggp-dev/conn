export type AgentConnection = { connId: number; agentId: string; idleSecs: number; sessions: string[] };
export type ConnectionGroup = { agentId: string; connections: AgentConnection[] };

/** Preserve real connections; duplicate names are identities, not disconnect evidence. */
export function groupConnections(connections: AgentConnection[]): ConnectionGroup[] {
  const groups = new Map<string, Map<number, AgentConnection>>();
  for (const connection of connections) {
    let group = groups.get(connection.agentId);
    if (!group) groups.set(connection.agentId, group = new Map());
    group.set(connection.connId, connection);
  }
  return [...groups].sort(([a], [b]) => a.localeCompare(b)).map(([agentId, group]) => ({
    agentId, connections: [...group.values()].sort((a, b) => a.connId - b.connId),
  }));
}
