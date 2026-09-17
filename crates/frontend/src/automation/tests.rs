use super::*;
use crate::Harness;
use conn_core::backend::Profile;
use base64::Engine as _;
fn caller() -> Caller { Caller { identity:"test-sender:1".into(), name:"Synthetic launcher".into(), still_alive:Arc::new(|| true) } }
fn harness() -> (tempfile::TempDir, Arc<Harness>) { harness_with_ack(true) }
fn harness_with_ack(ack: bool) -> (tempfile::TempDir, Arc<Harness>) {
    let (tmp,h,_) = harness_with_events(ack); (tmp,h)
}
fn harness_with_events(ack: bool) -> (tempfile::TempDir, Arc<Harness>, Arc<Mutex<Vec<(String,Value)>>>) {
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("config"); std::fs::create_dir_all(&config).unwrap();
    let mut p = Profile::local("test-shell".into(), "/bin/sh".into());
    p.args = vec!["-i".into()]; p.cwd = Some(tmp.path().display().to_string());
    p.env.insert("ENV".into(), "/dev/null".into()); p.env.insert("PS1".into(), "external-test$ ".into());
    let profiles = conn_core::profiles::Profiles { version:1, revision:0, default_profile:p.id.clone(), profiles:vec![p] };
    std::fs::write(config.join("profiles.json"), serde_json::to_vec(&profiles).unwrap()).unwrap();
    let target: Arc<Mutex<Weak<Harness>>> = Default::default();
    let receiver = target.clone();
    let events = Arc::new(Mutex::new(Vec::new())); let recorded = events.clone();
    let h = Arc::new(Harness::new(config, tmp.path().join("conn.sock"), Arc::new(move |name,value| {
        recorded.lock().push((name.to_owned(),value.clone()));
        if ack && name == "ss:tab_opened" && value["externalStarting"] == true {
            if let Some(h) = receiver.lock().upgrade() {
                let window = value["window"].as_str().unwrap();
                let session = value["session"].as_str().unwrap();
                let status = h.invoke_in_window(window,"status",json!({"session":session})).unwrap();
                assert_eq!(status["shared"],false); assert_eq!(status["externalOrigin"],true); assert_eq!(status["externalStarting"],true);
                h.invoke_in_window(window,"attach_output",json!({"session":session})).unwrap();
            }
        }
    })));
    *target.lock() = Arc::downgrade(&h);
    (tmp,h,events)
}
fn ready(h: &Harness) { h.invoke_in_window("main", "ui_ready", json!({})).unwrap(); }
fn enable(h: &Harness) { h.invoke_in_window("main", "automation_save", json!({"config":{"enabled":true,"profiles":["test-shell"]}})).unwrap(); }
fn open(h: &Harness) -> String { h.automate(caller(),"session.create",json!({})).unwrap().as_str().unwrap().into() }
fn physical(h: &Harness, handle: &str) -> String { h.state.automation.bindings.lock()[handle].session_id.clone() }
fn wait_for(mut condition: impl FnMut()->bool) {
    let until = Instant::now()+Duration::from_secs(5);
    while !condition() { assert!(Instant::now()<until,"timed out"); std::thread::sleep(Duration::from_millis(15)); }
}
fn done(h:&Harness,id:&str)->Value {
    let mut value=Value::Null;
    wait_for(|| { value=h.automate(caller(),"request.status",json!({"requestId":id})).unwrap(); matches!(value["state"].as_str(),Some("delivered"|"cancelled"|"failed")) }); value
}
// Capture the output delivered to the owning renderer. This is deliberately not
// an agent snapshot: no renderer has published a SurfaceFrame in these tests.
fn owner_output(events:&Arc<Mutex<Vec<(String,Value)>>>,id:&str,window:&str)->String {
    let bytes: Vec<u8> = events.lock().iter()
        .filter(|(name,value)| name == "ss:output" && value["session"] == id && value["window"] == window)
        .flat_map(|(_,value)| base64::engine::general_purpose::STANDARD.decode(value["data"].as_str().unwrap()).unwrap())
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}
fn assert_unrecorded(h:&Harness,secret:&str) {
    let audit=h.state.config_dir.join("audit.jsonl");
    let text=std::fs::read_to_string(audit).unwrap_or_default(); assert!(!text.contains(secret));
    assert!(!text.contains("automation_opened") && !text.contains("control_requested") && !text.contains("lease_granted"));
    let settings=h.state.automation.settings().to_string(); assert!(!settings.contains(secret)); assert!(!settings.contains("recent"));
    for job in h.state.automation.requests.lock().values() { if job.done() { assert!(job.payload.lock().is_none()); } assert!(!job.result.lock().to_string().contains(secret)); }
}
#[test]
fn parser_preserves_quoted_script_without_expansion_or_implicit_shell() {
    match launch_spec("/bin/sh -c 'printf \"%s\" \"$TOKEN\"; read ANSWER'").unwrap() {
        LaunchSpec::Program { executable,argv } => {assert_eq!(executable,"/bin/sh");assert_eq!(argv,vec!["-c","printf \"%s\" \"$TOKEN\"; read ANSWER"]);},_=>panic!()
    }
    for text in ["", "echo x; echo y", "echo $(id)", "echo `id`", "echo 'broken", "echo x\necho y"] { assert!(launch_spec(text).is_err()); }
    match launch_spec("/bin/echo 한글 'two words' \"\" 'a'\\''b' \"x\\qy\"").unwrap() {
        LaunchSpec::Program { argv,.. }=>assert_eq!(argv,vec!["한글","two words","","a'b","x\\qy"]),_=>panic!()
    }
}
#[test]
fn old_enabled_settings_require_reenable_without_losing_profiles() {
    let dir=tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("automation.json"),r#"{"enabled":true,"profiles":["test-shell"]}"#).unwrap();
    let a=Automation::load(dir.path()); assert_eq!(a.settings()["requiresReenable"],true);assert_eq!(a.settings()["config"]["enabled"],false);assert_eq!(a.settings()["config"]["profiles"],json!(["test-shell"]));
    let (_tmp,h)=harness();enable(&h);let reloaded=Automation::load(&h.state.config_dir);assert_eq!(reloaded.settings()["config"]["enabled"],true);assert_eq!(reloaded.settings()["requiresReenable"],false);
}
#[test]
fn launch_waits_for_window_and_never_creates_fallback_shell() {
    let (tmp,h)=harness();enable(&h);h.prepare_automation_window("external-1").unwrap();
    let proof=tmp.path().join("started");let end=tmp.path().join("finished");
    let command=format!("/bin/sh -c 'printf SYNTHETIC_STARTUP_비밀 > \"{}\"; read answer; printf done > \"{}\"'",proof.display(),end.display());
    let worker=h.clone();let task=std::thread::spawn(move||worker.automate_in_window("external-1",caller(),"window.create",json!({"command":command})));
    std::thread::sleep(Duration::from_millis(80));assert!(!proof.exists());assert!(h.state.hub.ids().is_empty());
    let start=h.invoke_in_window("external-1","start",json!({"rows":24,"cols":80})).unwrap();assert_eq!(start["externalPending"],true);assert_eq!(start["sessions"],json!([]));
    h.invoke_in_window("external-1","ui_ready",json!({})).unwrap();
    let handle=task.join().unwrap().unwrap()["session"].as_str().unwrap().to_owned();let id=physical(&h,&handle);
    wait_for(||proof.exists());assert_eq!(std::fs::read_to_string(proof).unwrap(),"SYNTHETIC_STARTUP_비밀");assert!(!end.exists());
    assert_eq!(h.invoke_in_window("external-1","status",json!({"session":id})).unwrap()["shared"],false);
    h.invoke_in_window("external-1","input",json!({"session":id,"data":"\r"})).unwrap();
    wait_for(||end.exists());wait_for(||!h.state.hub.get(&id).unwrap().lock().process_alive());
    assert_unrecorded(&h,"SYNTHETIC_STARTUP_비밀");assert_eq!(h.state.hub.ids().len(),1);
}
#[test]
fn private_writes_deliver_without_approval_and_discard_completed_payloads() {
    let (_tmp,h,events)=harness_with_events(true);ready(&h);enable(&h);let handle=open(&h);let id=physical(&h,&handle);
    let secret="SYNTHETIC_DIRECT_비밀";
    let args=json!({"session":handle,"text":format!("printf '%s\\n' '{secret}'"),"intent":secret,"requestId":"once"});
    let rid=h.automate(caller(),"session.write",args.clone()).unwrap();assert_eq!(done(&h,rid.as_str().unwrap())["state"],"delivered");
    wait_for(||owner_output(&events,&id,"main").contains(secret));
    assert_eq!(h.automate(caller(),"session.write",args).unwrap(),rid);
    assert!(h.automate(caller(),"session.write",json!({"session":handle,"text":"echo changed","requestId":"once"})).is_err());
    assert_unrecorded(&h,secret);
    for method in ["snapshot","status","input","affordances"] { assert!(conn_core::ipc::dispatch(&h.state.hub.get(&id).unwrap(),1,method,&json!({"data":"x"})).is_err()); }
    assert!(h.invoke_in_window("wrong-window","status",json!({"session":id})).is_err());
    assert_eq!(h.invoke("status",json!({"session":id})).unwrap()["shared"],false);
    let status=h.invoke_in_window("main","status",json!({"session":id})).unwrap();assert!(status["controlRequests"].as_array().unwrap().is_empty());
}
#[test]
fn human_input_cannot_be_recorded_and_permanently_revokes_injection() {
    let (_tmp,h)=harness();ready(&h);enable(&h);let handle=open(&h);let id=physical(&h,&handle);
    h.invoke_in_window("main","input",json!({"session":id,"data":"echo SYNTHETIC_HUMAN_INPUT\r"})).unwrap();
    assert!(h.automate(caller(),"session.write",json!({"session":handle,"text":"echo forbidden"})).is_err());
    assert_unrecorded(&h,"SYNTHETIC_HUMAN_INPUT");
    assert_eq!(h.invoke_in_window("main","status",json!({"session":id})).unwrap()["externalInputAvailable"],false);
}
#[test]
fn sender_ownership_exit_and_revocation_are_checked_at_delivery() {
    let (_tmp,h)=harness();ready(&h);enable(&h);let handle=open(&h);
    let mut foreign=caller();foreign.identity="different-sender".into();
    assert!(h.automate(foreign,"session.write",json!({"session":handle,"text":"secret"})).is_err());
    let active=Arc::new(AtomicBool::new(true));let alive=active.clone();let mut owner=caller();owner.still_alive=Arc::new(move||alive.load(Ordering::Acquire));
    let owned=h.automate(owner.clone(),"session.create",json!({})).unwrap();
    active.store(false,Ordering::Release);
    assert!(h.automate(owner,"session.write",json!({"session":owned,"text":"secret"})).is_err());
    wait_for(||!h.state.automation.bindings.lock().contains_key(owned.as_str().unwrap()));
    h.invoke_in_window("main","automation_save",json!({"config":{"enabled":false,"profiles":["test-shell"]}})).unwrap();
    assert!(h.automate(caller(),"session.write",json!({"session":handle,"text":"secret"})).is_err());
}
#[test]
fn cancelled_pending_jobs_release_payload_and_do_not_write_late() {
    let (_tmp,h,events)=harness_with_events(true);ready(&h);enable(&h);let handle=open(&h);
    let b=h.state.automation.bindings.lock()[&handle].clone();
    let id=b.session_id.clone();
    // Cancellation is set before publication; even an immediate worker cannot write.
    let payload=Payload(b"SYNTHETIC_CANCELLED\r".to_vec());
    let job=Arc::new(Job { id:"cancelled".into(),owner:caller().identity,handle:b.handle.clone(),binding:Arc::downgrade(&b),fingerprint:[0;32],payload:Mutex::new(Some(payload)),created:Instant::now(),cancelled:AtomicBool::new(true),result:Mutex::new(Value::Null) });
    job.update("queued",None);
    h.state.automation.requests.lock().insert(job.id.clone(),job.clone());
    b.queue.lock().push_back(job.clone());
    h.state.automation.stop_binding(&b);
    wait_for(||job.done());assert!(job.payload.lock().is_none());
    assert!(!owner_output(&events,&id,"main").contains("SYNTHETIC_CANCELLED"));
    assert_unrecorded(&h,"SYNTHETIC_CANCELLED");
}
#[test]
fn closing_a_private_session_releases_its_screen_but_keeps_minimal_polling_status() {
    let (_tmp,h,events)=harness_with_events(true);ready(&h);enable(&h);let handle=open(&h);let id=physical(&h,&handle);
    let private=Arc::downgrade(&h.state.hub.get(&id).unwrap());
    let binding=Arc::downgrade(&h.state.automation.bindings.lock()[&handle]);
    let request=h.automate(caller(),"session.write",json!({"session":handle,"text":"printf CLOSED_SCREEN_SENTINEL"})).unwrap();
    assert_eq!(done(&h,request.as_str().unwrap())["state"],"delivered");
    wait_for(||owner_output(&events,&id,"main").contains("CLOSED_SCREEN_SENTINEL"));
    h.invoke_in_window("main","close_tab",json!({"session":id})).unwrap();
    wait_for(||private.upgrade().is_none() && binding.upgrade().is_none());
    assert!(h.state.automation.bindings.lock().is_empty());
    let status=h.automate(caller(),"request.status",json!({"requestId":request})).unwrap();
    assert_eq!(status["state"],"delivered");
    assert!(!status.to_string().contains("CLOSED_SCREEN_SENTINEL"));
    assert_unrecorded(&h,"CLOSED_SCREEN_SENTINEL");
}
#[test]
fn released_handles_do_not_accumulate_across_sessions() {
    let (_tmp,h)=harness();ready(&h);enable(&h);
    for _ in 0..36 {
        let handle=open(&h);let id=physical(&h,&handle);
        h.automate(caller(),"session.release",json!({"session":handle})).unwrap();
        assert!(h.state.automation.bindings.lock().is_empty());
        assert!(h.automate(caller(),"session.write",json!({"session":handle,"text":"late"})).is_err());
        h.invoke_in_window("main","close_tab",json!({"session":id})).unwrap();
    }
    assert!(h.state.automation.bindings.lock().is_empty());
}
#[test]
fn close_during_readiness_and_malformed_command_create_no_orphan() {
    let (_tmp,h)=harness();enable(&h);h.prepare_automation_window("external-closed").unwrap();
    let worker=h.clone();let task=std::thread::spawn(move||worker.automate_in_window("external-closed",caller(),"window.create",json!({"command":"/bin/echo no"})));
    h.close_window("external-closed");assert!(task.join().unwrap().is_err());assert!(h.state.hub.ids().is_empty());
    ready(&h);assert!(h.automate(caller(),"window.create",json!({"command":"/bin/sh 'SYNTHETIC_UNCLOSED"})).unwrap_err().find("SYNTHETIC").is_none());
    assert!(h.automate(caller(),"window.create",json!({"command":"/no/SYNTHETIC_EXECUTABLE"})).unwrap_err().find("SYNTHETIC").is_none());
    assert!(h.state.hub.ids().is_empty());assert_unrecorded(&h,"SYNTHETIC");
}

#[test]
fn sender_exiting_while_startup_is_busy_cannot_launch_later() {
    use std::sync::atomic::AtomicUsize;
    let (_tmp,h)=harness();ready(&h);enable(&h);
    let held=h.state.startup.lock();
    let active=Arc::new(AtomicBool::new(true));let checks=Arc::new(AtomicUsize::new(0));
    let alive=active.clone();let seen=checks.clone();let mut owner=caller();
    owner.still_alive=Arc::new(move|| { seen.fetch_add(1,Ordering::AcqRel); alive.load(Ordering::Acquire) });
    let worker=h.clone();let create=std::thread::spawn(move||worker.automate(owner,"session.create",json!({})));
    wait_for(||checks.load(Ordering::Acquire)>=2);
    active.store(false,Ordering::Release);
    drop(held);
    assert!(create.join().unwrap().is_err());
    assert!(h.state.hub.ids().is_empty());
    assert!(h.state.automation.bindings.lock().is_empty());
}

#[test]
fn private_child_waits_for_its_owner_renderer_and_uses_prelaunch_dimensions() {
    let (tmp,h)=harness_with_ack(false);ready(&h);enable(&h);
    let proof=tmp.path().join("renderer-proof");
    let command=format!("/bin/sh -c 'stty size > \"{}\"'",proof.display());
    let worker=h.clone();let create=std::thread::spawn(move||worker.automate(caller(),"session.create",json!({"command":command})));
    wait_for(||!h.state.pending_sessions.lock().is_empty());
    let id=h.state.pending_sessions.lock().keys().next().unwrap().clone();
    assert!(!proof.exists()); assert!(h.state.hub.ids().is_empty());
    assert!(h.invoke_in_window("wrong-window","status",json!({"session":id})).is_err());
    assert!(h.invoke_in_window("wrong-window","attach_output",json!({"session":id})).is_err());
    assert!(!proof.exists());
    h.invoke_in_window("main","resize",json!({"session":id,"rows":33,"cols":111})).unwrap();
    h.invoke_in_window("main","attach_output",json!({"session":id})).unwrap();
    create.join().unwrap().unwrap();
    wait_for(||std::fs::read_to_string(&proof).is_ok_and(|s| s.trim() == "33 111"));
    assert_eq!(std::fs::read_to_string(proof).unwrap().trim(),"33 111");
    assert!(h.state.pending_sessions.lock().is_empty());
}
#[test]
fn closing_or_typing_before_renderer_attachment_cancels_without_a_child() {
    for action in ["input","take","close_tab","close_window","disable","profiles","revoke"] {
        let (tmp,h)=harness_with_ack(false);enable(&h);h.prepare_automation_window("reserved").unwrap();
        h.invoke_in_window("reserved","ui_ready",json!({})).unwrap();
        let proof=tmp.path().join("must-not-start");
        let command=format!("/bin/sh -c 'printf no > \"{}\"'",proof.display());
        let worker=h.clone();let create=std::thread::spawn(move||worker.automate_in_window("reserved",caller(),"window.create",json!({"command":command})));
        wait_for(||!h.state.pending_sessions.lock().is_empty());
        let id=h.state.pending_sessions.lock().keys().next().unwrap().clone();
        match action {
            "close_window" => h.close_window("reserved"),
            "disable" => { h.invoke_in_window("reserved","automation_save",json!({"config":{"enabled":false,"profiles":["test-shell"]}})).unwrap(); },
            "profiles" => {
                let config: Value = serde_json::from_slice(&std::fs::read(h.state.config_dir.join("profiles.json")).unwrap()).unwrap();
                h.invoke_in_window("reserved","profiles_save",json!({"config":config})).unwrap();
            },
            "revoke" => { h.invoke_in_window("reserved","automation_revoke",json!({})).unwrap(); },
            _ => { h.invoke_in_window("reserved",action,json!({"session":id,"data":"x"})).unwrap(); },
        }
        assert!(create.join().unwrap().is_err());
        assert!(!proof.exists());assert!(h.state.hub.ids().is_empty());
        assert!(h.state.pending_sessions.lock().is_empty());assert!(h.state.output.lock().is_empty());
        assert!(h.state.windows.lock().sessions("reserved").is_empty());
        assert!(h.invoke_in_window("reserved","attach_output",json!({"session":id})).is_err());
    }
}

#[test]
fn renderer_timeout_aborts_the_provisional_tab_without_running_the_command() {
    let (tmp,h,events)=harness_with_events(false);ready(&h);enable(&h);
    let proof=tmp.path().join("timeout-must-not-start");
    let command=format!("/bin/sh -c 'printf no > \"{}\"'",proof.display());
    let result=h.automate(caller(),"session.create",json!({"command":command}));
    assert!(result.is_err());assert!(!proof.exists());
    assert!(h.state.hub.ids().is_empty());assert!(h.state.pending_sessions.lock().is_empty());
    assert!(h.state.output.lock().is_empty());assert!(h.state.windows.lock().sessions("main").is_empty());
    let events=events.lock();
    let opened=events.iter().find(|(name,_)|name=="ss:tab_opened").unwrap();
    let aborted=events.iter().find(|(name,_)|name=="ss:tab_aborted").unwrap();
    assert_eq!(opened.1["session"],aborted.1["session"]);assert_eq!(aborted.1["window"],"main");
    assert!(events.iter().all(|(_,value)|!value.to_string().contains("timeout-must-not-start")));
}

#[test]
fn legacy_automation_config_does_not_authorize_linux_callers() {
    let (_tmp, h) = harness();
    enable(&h);
    let settings = h.invoke("automation_settings", json!({})).unwrap();
    assert_eq!(settings["config"]["linuxExecutables"], json!([]));
    let before = std::fs::read(h.state.config_dir.join("automation.json")).unwrap();
    assert!(h.invoke("automation_save", json!({"config":{"enabled":true,"profiles":["test-shell"],"linuxExecutables":["relative-launcher"]}})).is_err());
    assert_eq!(before, std::fs::read(h.state.config_dir.join("automation.json")).unwrap());
}

#[test]
fn linux_executable_permission_is_explicit_and_revocable() {
    let (_tmp, h) = harness();
    enable(&h);
    let exe = std::fs::canonicalize("/bin/sh").unwrap();
    assert!(!h.linux_automation_allowed(&exe));
    h.invoke("automation_save", json!({"config":{"enabled":true,"profiles":["test-shell"],"linuxExecutables":["/bin/sh"]}})).unwrap();
    assert!(h.linux_automation_allowed(&exe));
    h.invoke("automation_save", json!({"config":{"enabled":false,"profiles":["test-shell"],"linuxExecutables":["/bin/sh"]}})).unwrap();
    assert!(!h.linux_automation_allowed(&exe));
}

#[test]
fn hidden_and_masked_authentication_can_share_the_same_process_without_backfilling_secrets() {
    use conn_core::ipc::Client;
    let (tmp,h,events)=harness_with_events(true);ready(&h);enable(&h);
    // The child controls echo, just as an interactive authentication program does.
    // The masked field emits one star per received character and keeps echo off.
    let script=r#"printf 'ID: '; IFS= read -r user
stty -echo; printf 'Password (hidden): '; IFS= read -r secret; unset secret
stty -icanon min 1 time 0; printf '\nPassword (masked): '
while :; do character=$(dd bs=1 count=1 2>/dev/null); [ -z "$character" ] && break; printf '*'; done
stty echo icanon; unset character
printf '\nID: %s\nAUTH TEST PASSED\n' "$user"
export CONN_SHARED_MARKER=AUTHENTICATED_PROCESS_RETAINED
exec /bin/sh -i"#;
    let command=format!("/bin/sh -c '{}'",script.replace('\n',"; ").replace('\'',"'\\''"));
    let handle=h.automate(caller(),"session.create",json!({"command":command})).unwrap().as_str().unwrap().to_owned();
    let id=physical(&h,&handle);let original_session=h.state.hub.get(&id).unwrap();
    wait_for(||owner_output(&events,&id,"main").contains("ID: "));
    let selected=Client::connect(&tmp.path().join("conn.sock")).unwrap();
    let selected_id=selected.hello("agent","selected-auth-collaborator").unwrap()["conn"].as_u64().unwrap();
    let other=Client::connect(&tmp.path().join("conn.sock")).unwrap();
    other.hello("agent","unselected-collaborator").unwrap();
    assert!(selected.call("snapshot",json!({"session":id})).is_err());
    assert!(selected.call("publish_surface",json!({"session":id})).is_err());
    assert!(selected.call("set_sharing",json!({"session":id,"shared":true})).is_err());
    let hidden="SYNTHETIC_HIDDEN_PASSWORD";let masked="SYNTHETIC_MASKED_PASSWORD";
    for (text,prompt) in [("demo-user","Password (hidden): "),(hidden,"Password (masked): "),(masked,"external-test$ ")] {
        let request=h.automate(caller(),"session.write",json!({"session":handle,"text":text})).unwrap();
        assert_eq!(done(&h,request.as_str().unwrap())["state"],"delivered");
        wait_for(||owner_output(&events,&id,"main").contains(prompt));
    }
    let output=owner_output(&events,&id,"main");
    assert!(output.contains("ID: demo-user") && output.contains("AUTH TEST PASSED"));
    assert!(output.contains(&"*".repeat(masked.len())));
    assert!(!output.contains(hidden) && !output.contains(masked));
    assert_unrecorded(&h,hidden);assert_unrecorded(&h,masked);
    let shared=h.invoke_in_window("main","set_sharing",json!({"session":id,"shared":true,"connectionIds":[selected_id]})).unwrap();
    assert_eq!(shared["shared"],true);assert_eq!(shared["externalOrigin"],true);
    assert_eq!(shared["externalInputAvailable"],false);
    assert!(Arc::ptr_eq(&original_session,&h.state.hub.get(&id).unwrap()));
    assert!(h.automate(caller(),"session.write",json!({"session":handle,"text":"late external write"})).is_err());
    assert!(selected.call("snapshot",json!({"session":id})).is_err(),"sharing does not manufacture a screen");
    // This fixture acts as the owner renderer: only these presented rows are visible
    // to the agent, even though the native output stream includes the earlier ID prompt.
    let visible=json!(["ID: demo-user",format!("Password (masked): {}","*".repeat(masked.len())),"AUTH TEST PASSED","external-test$ "]);
    let status=h.invoke("status",json!({"session":id})).unwrap();
    let frame=json!({"surfaceId":"authentication-owner","generation":status["surfaceGeneration"],"revision":1,"outputSeq":status["outputSeq"],"rows":24,"cols":80,"cursor":null,"screen":visible,"alternateScreen":false,"visible":true});
    h.invoke_in_window("main","publish_surface",json!({"session":id,"frame":frame})).unwrap();
    let snapshot=selected.call("snapshot",json!({"session":id})).unwrap();
    assert_eq!(snapshot["screen"],visible);
    assert!(!snapshot.to_string().contains(hidden) && !snapshot.to_string().contains(masked));
    assert!(!snapshot.to_string().contains("Password (hidden):"));
    assert!(other.call("snapshot",json!({"session":id})).is_err());
    // Shell-local state set before sharing proves that no replacement PTY was spawned.
    h.invoke_in_window("main","input",json!({"session":id,"data":"printf '%s\\n' \"$CONN_SHARED_MARKER\"\r"})).unwrap();
    wait_for(||owner_output(&events,&id,"main").contains("AUTHENTICATED_PROCESS_RETAINED"));
    let audit=std::fs::read_to_string(h.state.config_dir.join("audit.jsonl")).unwrap();
    assert!(audit.contains("sharing_started"));
    assert!(!audit.contains(hidden) && !audit.contains(masked) && !audit.contains(script));
    assert!(!audit.contains("dd bs=1") && !audit.contains("read -r secret"));
    h.invoke_in_window("main","set_sharing",json!({"session":id,"shared":false,"connectionIds":[]})).unwrap();
    assert!(selected.call("snapshot",json!({"session":id})).is_err());
    assert!(h.automate(caller(),"session.write",json!({"session":handle,"text":"revoked stays revoked"})).is_err());
}
