//! Owner renderer identity, ordered terminal operations and bounded outcomes.
//! Adapters authenticate the view; no web payload chooses a native window.
use parking_lot::{Condvar, Mutex, RwLock};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub const PROTOCOL_VERSION: u32 = 1;
const OUTCOMES: u64 = 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
fn read_only(name: &str) -> bool {
    matches!(
        name,
        "profiles_test"
            | "profiles_discover"
            | "profiles_catalog"
            | "diagnostics"
            | "agent_integrations"
            | "cli_status"
            | "extensions_status"
            | "automation_settings"
            | "status"
            | "sharing_participants"
            | "sharing_state"
            | "agents"
            | "policy_rules"
            | "policy_read"
            | "policy_test"
            | "audit_tail"
            | "pending_admissions"
            | "admission_snapshot"
            | "admission_policy"
            | "activity_snapshot"
    )
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnerMeta {
    pub epoch: u64,
    pub operation_id: u64,
    #[serde(default)]
    pub sequence: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnerRequest {
    pub name: String,
    #[serde(default)]
    pub args: Value,
    #[serde(flatten)]
    pub meta: OwnerMeta,
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OwnerHello {
    pub protocol: u32,
    pub runtime_id: String,
    pub build_version: String,
    pub epoch: u64,
    pub view_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OwnerSize {
    pub rows: u16,
    pub cols: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OwnerOutput {
    pub session: String,
    pub data: String,
    pub output_seq: u64,
    pub generation: u64,
    pub size: OwnerSize,
    pub epoch: u64,
    pub stream_seq: u64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reset: bool,
}
impl OwnerOutput {
    pub(crate) fn frame(
        session: &str,
        frame: &conn_core::session::OutputFrame,
        epoch: u64,
        stream_seq: u64,
        reset: bool,
    ) -> Value {
        use base64::Engine as _;
        serde_json::to_value(Self {
            session: session.into(),
            data: base64::engine::general_purpose::STANDARD.encode(&frame.data),
            output_seq: frame.output_seq,
            generation: frame.generation,
            size: OwnerSize {
                rows: frame.size.rows,
                cols: frame.size.cols,
            },
            epoch,
            stream_seq,
            reset,
        })
        .unwrap()
    }
}
#[derive(Default)]
struct Lane {
    next: u64,
    running: bool,
}
struct Operation {
    fingerprint: Vec<u8>,
    result: Option<Result<Value, String>>,
}
#[derive(Default)]
struct ViewState {
    epoch: u64,
    attached: bool,
    high: u64,
    operations: BTreeMap<u64, Operation>,
    archived: BTreeMap<(u64, u64), Option<Result<Value, String>>>,
    lanes: HashMap<String, Lane>,
}
#[derive(Default)]
struct View {
    current_epoch: AtomicU64,
    fence: RwLock<()>,
    state: Mutex<ViewState>,
    changed: Condvar,
}
pub(crate) struct Owners {
    runtime: String,
    views: Mutex<HashMap<String, Arc<View>>>,
}
impl Owners {
    pub fn new() -> Self {
        Self {
            runtime: uuid::Uuid::new_v4().to_string(),
            views: Default::default(),
        }
    }
    pub fn runtime_id(&self) -> &str {
        &self.runtime
    }
    fn view(&self, window: &str) -> Arc<View> {
        self.views.lock().entry(window.into()).or_default().clone()
    }
    pub fn epoch(&self, window: &str) -> u64 {
        self.view(window).current_epoch.load(Ordering::Acquire)
    }
    pub fn attach(
        &self,
        window: &str,
        takeover: bool,
        reset: impl FnOnce(u64),
    ) -> Result<OwnerHello, String> {
        let view = self.view(window);
        let _fence = view.fence.write();
        let mut state = view.state.lock();
        if state.attached && !takeover {
            return Err("owner_attached".into());
        }
        let previous = state.epoch;
        for (id, op) in std::mem::take(&mut state.operations) {
            state.archived.insert((previous, id), op.result);
        }
        while state.archived.len() > OUTCOMES as usize {
            state.archived.pop_first();
        }
        state.epoch += 1;
        state.attached = true;
        state.high = 0;
        state.lanes.clear();
        let epoch = state.epoch;
        view.current_epoch.store(epoch, Ordering::Release);
        reset(epoch);
        view.changed.notify_all();
        Ok(OwnerHello {
            protocol: PROTOCOL_VERSION,
            runtime_id: self.runtime.clone(),
            build_version: env!("CARGO_PKG_VERSION").into(),
            epoch,
            view_id: window.into(),
        })
    }
    pub fn detach(&self, window: &str, epoch: u64, reset: impl FnOnce()) {
        let view = self.view(window);
        let _fence = view.fence.write();
        let mut state = view.state.lock();
        if state.epoch == epoch {
            state.attached = false;
            reset();
            view.changed.notify_all();
        }
    }
    pub fn outcome(&self, window: &str, epoch: u64, id: u64) -> Result<Value, String> {
        let view = self.view(window);
        let state = view.state.lock();
        let record = if epoch == state.epoch {
            state.operations.get(&id).map(|op| &op.result)
        } else {
            state.archived.get(&(epoch, id))
        };
        Ok(match record {
            Some(Some(Ok(v))) => json!({"status":"completed","result":v}),
            Some(Some(Err(e))) => json!({"status":"completed","error":e}),
            Some(None) => json!({"status":"pending"}),
            None => json!({"status":"unknown"}),
        })
    }

    pub fn execute(
        &self,
        window: &str,
        request: OwnerRequest,
        run: impl FnOnce(&str, Value) -> Result<Value, String>,
    ) -> Result<Value, String> {
        let view = self.view(window);
        let id = request.meta.operation_id;
        let epoch = request.meta.epoch;
        if id == 0 || id > MAX_SAFE_INTEGER {
            return Err("invalid_operation_id".into());
        }
        let lane_key = if matches!(
            request.name.as_str(),
            "input" | "terminal_response" | "resize"
        ) {
            if request
                .meta
                .sequence
                .is_none_or(|n| n == 0 || n > MAX_SAFE_INTEGER)
            {
                return Err("terminal_sequence_required".into());
            }
            Some(
                request.args["session"]
                    .as_str()
                    .ok_or("session_required")?
                    .to_owned(),
            )
        } else {
            None
        };
        // Dedup retains a digest, never raw password/editor input.
        let fingerprint = Sha256::digest(
            serde_json::to_vec(&(request.name.as_str(), &request.args, request.meta.sequence))
                .unwrap(),
        )
        .to_vec();
        let deadline = Instant::now() + Duration::from_secs(10);
        // Wait without holding the epoch fence, so a takeover can retire a missing sequence.
        {
            let mut state = view.state.lock();
            loop {
                if state.epoch != epoch || !state.attached {
                    return Err("attachment_fenced".into());
                }
                if let Some(op) = state.operations.get(&id) {
                    if op.fingerprint != fingerprint {
                        return Err("operation_id_conflict".into());
                    }
                    if let Some(result) = &op.result {
                        return result.clone();
                    }
                    if Instant::now() >= deadline {
                        return Err("outcome_pending".into());
                    }
                    view.changed.wait_for(&mut state, Duration::from_millis(50));
                    continue;
                }
                if id <= state.high.saturating_sub(OUTCOMES) {
                    return Err("outcome_unknown".into());
                }
                if let Some(key) = &lane_key {
                    let lane = state.lanes.entry(key.clone()).or_insert(Lane {
                        next: 1,
                        running: false,
                    });
                    let sequence = request.meta.sequence.unwrap();
                    if sequence < lane.next {
                        return Err("terminal_sequence_stale".into());
                    }
                    if sequence != lane.next || lane.running {
                        if Instant::now() >= deadline {
                            return Err("terminal_sequence_gap".into());
                        }
                        view.changed.wait_for(&mut state, Duration::from_millis(50));
                        continue;
                    }
                    lane.running = true;
                }
                state.high = state.high.max(id);
                let floor = state.high.saturating_sub(OUTCOMES);
                state
                    .operations
                    .retain(|n, op| *n > floor || op.result.is_none());
                if state.operations.len() >= OUTCOMES as usize {
                    if let Some(key) = &lane_key {
                        state.lanes.get_mut(key).unwrap().running = false;
                    }
                    return Err("owner_busy".into());
                }
                state.operations.insert(
                    id,
                    Operation {
                        fingerprint,
                        result: None,
                    },
                );
                break;
            }
        }
        // Slow observations do not postpone handoff. Mutations finish atomically
        // before epoch replacement; stale queued mutations never start.
        let _fence = (!read_only(&request.name)).then(|| view.fence.read());
        let mut state = view.state.lock();
        if state.epoch != epoch || !state.attached {
            let rejected = Some(Err("attachment_fenced".to_owned()));
            if state.epoch == epoch {
                if let Some(operation) = state.operations.get_mut(&id) {
                    operation.result = rejected;
                }
            } else if let Some(outcome) = state.archived.get_mut(&(epoch, id)) {
                *outcome = rejected;
            }
            view.changed.notify_all();
            return Err("attachment_fenced".into());
        }
        drop(state);
        let result = run(&request.name, request.args);
        state = view.state.lock();
        if state.epoch != epoch {
            if let Some(outcome) = state.archived.get_mut(&(epoch, id)) {
                *outcome = Some(result);
            }
            view.changed.notify_all();
            return Err("attachment_fenced".into());
        }
        if let Some(op) = state.operations.get_mut(&id) {
            op.result = Some(result.clone());
        }
        if let Some(key) = &lane_key {
            let lane = state.lanes.get_mut(key).unwrap();
            lane.running = false;
            lane.next += 1;
        }
        view.changed.notify_all();
        result
    }
}

/// Machine-readable source for the generated TypeScript owner transport contract.
pub fn contract() -> Value {
    json!({"protocol":PROTOCOL_VERSION,"request":schemars::schema_for!(OwnerRequest),
        "meta":schemars::schema_for!(OwnerMeta),"hello":schemars::schema_for!(OwnerHello),"output":schemars::schema_for!(OwnerOutput)})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(epoch: u64, id: u64, seq: Option<u64>, name: &str) -> OwnerRequest {
        OwnerRequest {
            name: name.into(),
            args: json!({"session":"a"}),
            meta: OwnerMeta {
                epoch,
                operation_id: id,
                sequence: seq,
            },
        }
    }
    #[test]
    fn session_operations_wait_for_their_sequence_not_worker_scheduling() {
        let owner = Arc::new(Owners::new());
        let epoch = owner.attach("main", false, |_| {}).unwrap().epoch;
        let out = Arc::new(Mutex::new(Vec::new()));
        let o = owner.clone();
        let recorded = out.clone();
        let later = std::thread::spawn(move || {
            o.execute("main", request(epoch, 2, Some(2), "resize"), |_, _| {
                recorded.lock().push(2);
                Ok(Value::Null)
            })
        });
        std::thread::sleep(Duration::from_millis(25));
        owner
            .execute("main", request(epoch, 1, Some(1), "input"), |_, _| {
                out.lock().push(1);
                Ok(Value::Null)
            })
            .unwrap();
        later.join().unwrap().unwrap();
        assert_eq!(*out.lock(), vec![1, 2]);
    }
    #[test]
    fn slow_independent_work_does_not_block_terminal_input_or_event_callback() {
        let owner = Arc::new(Owners::new());
        let epoch = owner.attach("main", false, |_| {}).unwrap().epoch;
        let (started, ready) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let o = owner.clone();
        let slow = std::thread::spawn(move || {
            o.execute("main", request(epoch, 1, None, "profiles_test"), |_, _| {
                started.send(()).unwrap();
                wait.recv().unwrap();
                Ok(Value::Null)
            })
        });
        ready.recv().unwrap();
        let start = Instant::now();
        owner
            .execute("main", request(epoch, 2, Some(1), "input"), |_, _| {
                Ok(json!("input"))
            })
            .unwrap();
        assert!(start.elapsed() < Duration::from_millis(100));
        release.send(()).unwrap();
        slow.join().unwrap().unwrap();
    }
    #[test]
    fn slow_read_does_not_delay_takeover_or_write_into_the_new_epoch_outcome() {
        let owner = Arc::new(Owners::new());
        let epoch = owner.attach("main", false, |_| {}).unwrap().epoch;
        let (started, ready) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let o = owner.clone();
        let slow = std::thread::spawn(move || {
            o.execute("main", request(epoch, 1, None, "profiles_test"), |_, _| {
                started.send(()).unwrap();
                wait.recv().unwrap();
                Ok(json!("old"))
            })
        });
        ready.recv().unwrap();
        let next = owner.attach("main", true, |_| {}).unwrap();
        owner
            .execute("main", request(next.epoch, 1, None, "status"), |_, _| {
                Ok(json!("new"))
            })
            .unwrap();
        release.send(()).unwrap();
        assert_eq!(slow.join().unwrap().unwrap_err(), "attachment_fenced");
        assert_eq!(
            owner.outcome("main", next.epoch, 1).unwrap()["result"],
            "new"
        );
    }
    #[test]
    fn retry_returns_the_same_outcome_and_never_reexecutes_evicted_operation() {
        let owner = Owners::new();
        let epoch = owner.attach("main", false, |_| {}).unwrap().epoch;
        let req = request(epoch, 1, None, "take");
        assert_eq!(
            owner
                .execute("main", req.clone(), |_, _| Ok(json!("first")))
                .unwrap(),
            json!("first")
        );
        assert_eq!(
            owner
                .execute("main", req.clone(), |_, _| panic!("duplicate mutation"))
                .unwrap(),
            json!("first")
        );
        for id in 2..=1030 {
            owner
                .execute("main", request(epoch, id, None, "status"), |_, _| {
                    Ok(Value::Null)
                })
                .unwrap();
        }
        assert_eq!(
            owner
                .execute("main", req, |_, _| panic!("expired mutation rerun"))
                .unwrap_err(),
            "outcome_unknown"
        );
    }
    #[test]
    fn explicit_handoff_fences_old_queued_input_and_duplicate_views() {
        let owner = Arc::new(Owners::new());
        let first = owner.attach("main", false, |_| {}).unwrap();
        assert_eq!(
            owner.attach("main", false, |_| {}).unwrap_err(),
            "owner_attached"
        );
        let o = owner.clone();
        let late = std::thread::spawn(move || {
            o.execute("main", request(first.epoch, 2, Some(2), "input"), |_, _| {
                panic!("old input executed")
            })
        });
        std::thread::sleep(Duration::from_millis(20));
        let next = owner.attach("main", true, |_| {}).unwrap();
        assert_eq!(late.join().unwrap().unwrap_err(), "attachment_fenced");
        owner.detach("main", first.epoch, || {
            panic!("old disconnect detached current owner")
        });
        owner
            .execute("main", request(next.epoch, 1, Some(1), "input"), |_, _| {
                Ok(Value::Null)
            })
            .unwrap();
    }
    #[test]
    fn lost_mutation_outcome_can_be_queried_after_reconnect_without_reexecution() {
        let owner = Owners::new();
        let first = owner.attach("main", false, |_| {}).unwrap();
        owner
            .execute("main", request(first.epoch, 1, None, "take"), |_, _| {
                Ok(json!({"retained":true}))
            })
            .unwrap();
        owner.detach("main", first.epoch, || {});
        let second = owner.attach("main", false, |_| {}).unwrap();
        assert!(second.epoch > first.epoch);
        assert_eq!(
            owner.outcome("main", first.epoch, 1).unwrap()["result"]["retained"],
            true
        );
        assert_eq!(
            owner
                .execute(
                    "main",
                    request(first.epoch, 1, None, "take"),
                    |_, _| panic!("old mutation reexecuted")
                )
                .unwrap_err(),
            "attachment_fenced"
        );
    }
    #[test]
    fn owner_wire_contract_deserializes_flat_metadata() {
        let r: OwnerRequest = serde_json::from_value(
            json!({"name":"input","args":{"session":"a"},"epoch":1,"operationId":1,"sequence":1}),
        )
        .unwrap();
        assert_eq!(r.meta.sequence, Some(1));
    }
}
