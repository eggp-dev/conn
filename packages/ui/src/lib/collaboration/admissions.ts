import type { AdmissionChange, AdmissionSnapshot } from './contracts.ts';
/** Per-connection versions allow out-of-order events; snapshots replace only older state. */
export class Admissions {
  private floor = -1;
  private versions = new Map<number, number>();
  private pending = new Map<number, { connId: number; agentId: string }>();
  get requests() { return [...this.pending.values()].sort((a, b) => a.connId - b.connId); }
  event(event: AdmissionChange) {
    if (event.revision <= Math.max(this.floor, this.versions.get(event.connId) ?? -1)) return;
    this.versions.set(event.connId, event.revision);
    if (event.state === 'pending') this.pending.set(event.connId, { connId: event.connId, agentId: event.agentId });
    else this.pending.delete(event.connId);
  }
  snapshot(snapshot: AdmissionSnapshot) {
    if (snapshot.revision < this.floor) return;
    const incoming = new Map(snapshot.pending.map(request => [request.connId, request]));
    for (const id of new Set([...this.pending.keys(), ...incoming.keys()])) {
      if ((this.versions.get(id) ?? -1) > snapshot.revision) continue;
      if (incoming.has(id)) this.pending.set(id, incoming.get(id)!); else this.pending.delete(id);
    }
    this.floor = snapshot.revision;
    for (const [id, version] of this.versions) if (version <= this.floor) this.versions.delete(id);
  }
}
