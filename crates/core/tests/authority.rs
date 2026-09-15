use std::time::{Duration, Instant};

use conn_core::authority::{Authority, AuthorityError, Controller};

#[test]
fn human_to_agent_and_back() {
    let mut a = Authority::new(Duration::from_secs(60));
    assert!(a.controller().is_human());
    let lease = a.request("copilot", 1, Instant::now()).unwrap();
    assert!(matches!(a.controller(), Controller::Agent(l) if l.conn == 1));
    assert_eq!(lease.label(), "lease#1");
    let revoked = a.revoke().unwrap();
    assert_eq!(revoked.id, lease.id);
    assert!(a.controller().is_human());
}

#[test]
fn busy_when_another_agent_holds_valid_lease() {
    let mut a = Authority::new(Duration::from_secs(60));
    a.request("a", 1, Instant::now()).unwrap();
    assert!(matches!(a.request("b", 2, Instant::now()), Err(AuthorityError::Busy { agent_id }) if agent_id == "a"));
}

#[test]
fn expired_lease_is_reclaimable_and_rejects_writes() {
    let mut a = Authority::new(Duration::from_millis(10));
    let t0 = Instant::now();
    a.request("a", 1, t0).unwrap();
    let later = t0 + Duration::from_millis(50);
    assert_eq!(a.check_and_touch(1, later), Err(AuthorityError::Expired));
    assert!(a.controller().is_human());
    a.request("b", 2, later).unwrap();
    assert!(a.holds(2));
}

#[test]
fn touch_extends_lease() {
    let mut a = Authority::new(Duration::from_millis(100));
    let t0 = Instant::now();
    a.request("a", 1, t0).unwrap();
    a.check_and_touch(1, t0 + Duration::from_millis(80)).unwrap();
    assert!(a.expire_if_due(t0 + Duration::from_millis(150)).is_none());
    assert!(a.expire_if_due(t0 + Duration::from_millis(200)).is_some());
}

#[test]
fn foreign_conn_is_not_controller() {
    let mut a = Authority::new(Duration::from_secs(60));
    a.request("a", 1, Instant::now()).unwrap();
    assert_eq!(a.check_and_touch(2, Instant::now()), Err(AuthorityError::NotController));
    assert!(a.revoke_if_held_by(2).is_none());
    assert!(a.revoke_if_held_by(1).is_some());
}
