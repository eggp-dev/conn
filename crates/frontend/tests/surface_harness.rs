#![cfg(unix)]
use conn_frontend::Harness;
use serde_json::{json,Value};
use std::{sync::{Arc,Mutex},time::{Duration,Instant}};

fn setup() -> (tempfile::TempDir,Harness,Arc<Mutex<Vec<Value>>>,String) {
    let dir=tempfile::tempdir().unwrap();
    let mut profile=conn_core::backend::Profile::local("test".into(),"/bin/bash".into());
    profile.args=vec!["--noprofile".into(),"--norc".into()];
    profile.cwd=Some(dir.path().to_string_lossy().into());
    std::fs::write(dir.path().join("profiles.json"),serde_json::to_vec(&conn_core::profiles::Profiles{version:1,revision:0,default_profile:"test".into(),profiles:vec![profile]}).unwrap()).unwrap();
    let output=Arc::new(Mutex::new(Vec::new()));let capture=output.clone();
    let h=Harness::new(dir.path().into(),dir.path().join("conn.sock"),Arc::new(move |name,v| {if name=="ss:output" {capture.lock().unwrap().push(v);}}));
    let start=h.invoke("start",json!({"rows":24,"cols":80})).unwrap();
    let id=start["session"].as_str().unwrap().to_string();
    h.invoke("attend",json!({"session":id})).unwrap();
    h.invoke("attach_output",json!({"session":id})).unwrap();
    let deadline=Instant::now()+Duration::from_secs(4);
    while output.lock().unwrap().is_empty() {assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(10));}
    (dir,h,output,id)
}
fn frame(h:&Harness,id:&str,revision:u64)->Value {
    let status=h.invoke("status",json!({"session":id})).unwrap();
    json!({"surfaceId":"owner-renderer","generation":status["surfaceGeneration"],"revision":revision,"outputSeq":status["outputSeq"],"rows":24,"cols":80,"cursor":null,"screen":["visible synthetic frame"],"alternateScreen":false,"visible":true})
}
#[test]
fn native_frames_are_sequenced_owner_bound_and_explicitly_invalidated() {
    let (_dir,h,output,id)=setup();
    let entries=output.lock().unwrap();
    assert!(entries.iter().all(|v|v["generation"].is_u64()&&v["outputSeq"].is_u64()&&v["window"]=="main"));
    let seq:Vec<_>=entries.iter().map(|v|v["outputSeq"].as_u64().unwrap()).collect();
    assert!(seq.windows(2).all(|w|w[0]<w[1]));drop(entries);
    let f=frame(&h,&id,1);
    assert!(h.invoke_in_window("wrong-window","publish_surface",json!({"session":id,"frame":f})).is_err());
    h.invoke("publish_surface",json!({"session":id,"frame":f})).unwrap();
    assert_eq!(h.invoke("status",json!({"session":id})).unwrap()["surfaceAvailable"],true);
    h.blur_window("main");
    assert_eq!(h.invoke("status",json!({"session":id})).unwrap()["surfaceAvailable"],false);
    assert!(h.invoke("publish_surface",json!({"session":id,"frame":f})).is_err());
    assert!(h.invoke("completion_request",json!({"session":id,"explicit":true,"context":"hidden override"})).is_err());
}
#[test]
fn sharing_changes_keep_same_process_and_private_input_is_not_retroactive() {
    let (dir,h,_output,id)=setup();
    h.invoke("set_sharing",json!({"session":id,"shared":false,"connectionIds":[]})).unwrap();
    let private=h.invoke("status",json!({"session":id})).unwrap();
    assert_eq!(private["shared"],false);assert_eq!(private["processAlive"],true);
    h.invoke("input",json!({"session":id,"data":"export CONN_SAME_PROCESS_MARKER=survived\n"})).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    h.invoke("set_sharing",json!({"session":id,"shared":true,"connectionIds":[]})).unwrap();
    h.invoke("input",json!({"session":id,"data":"printf '%s' \"$CONN_SAME_PROCESS_MARKER\" > same-process\n"})).unwrap();
    let deadline=Instant::now()+Duration::from_secs(4);
    while !dir.path().join("same-process").exists() {assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(10));}
    assert_eq!(std::fs::read_to_string(dir.path().join("same-process")).unwrap(),"survived");
    let audit=std::fs::read_to_string(dir.path().join("audit.jsonl")).unwrap();
    assert!(!audit.contains("export CONN_SAME_PROCESS_MARKER=survived"));
    assert!(audit.contains("sharing_started") && audit.contains("sharing_stopped"));
}
#[test]
fn private_and_wrong_window_cannot_request_completion_or_add_participants() {
    let (_dir,h,_output,id)=setup();
    assert!(h.invoke("set_sharing",json!({"session":id,"shared":true,"connectionIds":[999999]})).is_err());
    assert!(h.invoke_in_window("wrong-window","set_sharing",json!({"session":id,"shared":true,"connectionIds":[]})).is_err());
    h.invoke("set_sharing",json!({"session":id,"shared":false,"connectionIds":[]})).unwrap();
    assert!(h.invoke("completion_request",json!({"session":id,"explicit":true})).is_err());
    let status=h.invoke("extensions_status",json!({"session":id})).unwrap();
    assert_eq!(status["settings"]["providerEnabled"],false);
    assert_eq!(status["settings"]["completionEnabled"],false);
    assert!(status.get("key").is_none());
}
