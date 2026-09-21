/** App connections and session participation deliberately have different scopes. */
export type ConnectionId = number;
export type AdmissionRequest = { connId: ConnectionId; agentId: string };
export type AdmissionChange = AdmissionRequest & { state: 'pending' | 'granted' | 'denied' | 'closed'; revision: number };
export type AdmissionSnapshot = { revision: number; pending: AdmissionRequest[] };
export type Participant = AdmissionRequest & { selected: boolean };
export type SharingSnapshot = { revision: number; participants: Participant[] };
export type SharingStatus = { shared: boolean; inputPending: boolean; externalInputAvailable: boolean };
export type DecisionTarget = { session: string; requestId: string };
