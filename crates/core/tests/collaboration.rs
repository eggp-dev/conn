mod common;
use common::{Harness, SessionExt};
use conn_core::{
    collaboration::Hub,
    session::{AgentMode, ControlOutcome, ControllerInfo},
};
use serde_json::json;
use std::sync::Arc;
fn pair() -> (Arc<Hub>, conn_core::SharedSession, conn_core::SharedSession) {
    let mut a = Harness::new();
    let mut b = Harness::new();
    a.agent(7, "agent");
    b.agent(7, "agent");
    let a = Arc::new(parking_lot::Mutex::new(a.session));
    let b = Arc::new(parking_lot::Mutex::new(b.session));
    let hub = Hub::new();
    hub.add("a", a.clone());
    hub.add("b", b.clone());
    (hub, a, b)
}
#[test]
fn owner_grant_and_handback_share_cross_session_exclusivity() {
    let (hub, a, b) = pair();
    a.lock().agent_request_control(7).unwrap();
    b.lock().set_control_gate(true);
    let ControlOutcome::Pending { request_id } = b
        .lock()
        .agent_request_control_original(7, json!({}))
        .unwrap()
    else {
        panic!()
    };
    hub.decide_control("b", &request_id, true).unwrap();
    assert!(matches!(
        a.lock().status().controller,
        ControllerInfo::Human
    ));
    assert!(matches!(
        b.lock().status().controller,
        ControllerInfo::Agent { .. }
    ));
    hub.hand_back("a").unwrap();
    assert!(matches!(
        a.lock().status().controller,
        ControllerInfo::Agent { .. }
    ));
    assert!(matches!(
        b.lock().status().controller,
        ControllerInfo::Human
    ));
}
#[test]
fn rejected_target_preserves_the_previous_lease() {
    let (hub, a, b) = pair();
    a.lock().agent_request_control(7).unwrap();
    b.lock().set_mode(AgentMode::Observe).unwrap();
    assert!(hub.request_control("b", 7, &json!({})).is_err());
    assert!(matches!(
        a.lock().status().controller,
        ControllerInfo::Agent { .. }
    ));
}
#[test]
fn simultaneous_owner_handbacks_leave_exactly_one_controller() {
    let (hub, a, b) = pair();
    a.lock().agent_request_control(7).unwrap();
    a.lock().human_take();
    b.lock().agent_request_control(7).unwrap();
    b.lock().human_take();
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let threads: Vec<_> = ["a", "b"]
        .into_iter()
        .map(|id| {
            let (h, b) = (hub.clone(), barrier.clone());
            std::thread::spawn(move || {
                b.wait();
                h.hand_back(id).unwrap();
            })
        })
        .collect();
    barrier.wait();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(
        [a, b]
            .iter()
            .filter(|s| matches!(s.lock().status().controller, ControllerInfo::Agent { .. }))
            .count(),
        1
    );
}
#[test]
fn takeover_and_exit_end_pending_control_requests() {
    for exit in [false, true] {
        let mut h = Harness::new();
        h.agent(7, "agent");
        h.session.set_control_gate(true);
        let ControlOutcome::Pending { request_id } = h
            .session
            .agent_request_control_original(7, json!({}))
            .unwrap()
        else {
            panic!()
        };
        if exit {
            h.session.process_exited(Some(0));
        } else {
            h.session.human_take();
        }
        assert_eq!(
            h.session.control_request_state(&request_id),
            Some(conn_core::session::ControlRequestState::Denied)
        );
        assert!(h.session.decide_control(&request_id, true).is_err());
    }
}
