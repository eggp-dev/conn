//! Loopback test transport for the same native backend and frontend commands used by Tauri.
use std::{collections::HashMap, path::PathBuf, sync::{Arc, atomic::{AtomicBool, Ordering}}};
use axum::{Router, routing::get, extract::{ws::{WebSocket, WebSocketUpgrade, Message}, State, Query}, http::{HeaderMap, StatusCode}, response::IntoResponse};
use conn_core::{backend::Profile, policy::PolicyStore};
use conn_frontend::Harness;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};

#[derive(Clone)]
struct Server { token: String, origin: String, dir: PathBuf, connected: Arc<AtomicBool> }
async fn upgrade(State(s): State<Server>, Query(q): Query<HashMap<String,String>>, headers: HeaderMap, ws: WebSocketUpgrade) -> axum::response::Response {
    if headers.get("origin").and_then(|v|v.to_str().ok()) != Some(s.origin.as_str()) || q.get("token") != Some(&s.token) {
        return (StatusCode::FORBIDDEN,"invalid test origin or token").into_response();
    }
    if s.connected.swap(true,Ordering::SeqCst) { return (StatusCode::CONFLICT,"one browser test client at a time").into_response(); }
    ws.max_message_size(128*1024).on_upgrade(move |socket| serve(socket,s))
}
async fn serve(socket: WebSocket, server: Server) {
    let (tx,mut rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
    let emit_tx = tx.clone();
    let harness = Arc::new(Harness::new(server.dir.clone(),server.dir.join("conn.sock"),Arc::new(move |name,payload| { let _ = emit_tx.send(json!({"event":name,"payload":payload})); })));
    let (mut writer,mut reader) = socket.split();
    loop {
        if rx.len() > 2048 { break; } // Disconnect on overload rather than silently dropping state transitions.
        tokio::select! {
            Some(value) = rx.recv() => { if writer.send(Message::Text(value.to_string().into())).await.is_err() { break; } },
            message = reader.next() => {
                let Some(Ok(Message::Text(text))) = message else { break; };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { break; };
                let id = v["id"].clone(); let name = v["name"].as_str().unwrap_or("").to_owned(); let args = v["args"].clone();
                let h = harness.clone();
                // Serialized commands preserve typing order and prevent racing start/open/close.
                let result = tokio::task::spawn_blocking(move || h.invoke(&name,args)).await;
                let response = match result { Ok(Ok(result))=>json!({"id":id,"result":result}), Ok(Err(e))=>json!({"id":id,"error":e}), Err(e)=>json!({"id":id,"error":e.to_string()}) };
                if writer.send(Message::Text(response.to_string().into())).await.is_err() { break; }
            }
        }
    }
    let _ = tokio::task::spawn_blocking(move || { harness.shutdown(); drop(harness); }).await;
    server.connected.store(false,Ordering::SeqCst);
}
#[tokio::main]
async fn main() -> Result<(),Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).map(PathBuf::from).ok_or("usage: conn-browser-harness TEST_DIRECTORY")?;
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&dir,std::fs::Permissions::from_mode(0o700))?; }
    if !dir.join("profiles.json").exists() {
        let workspace = dir.join("workspace"); std::fs::create_dir_all(&workspace)?;
        let mut p=Profile::local("test-native-shell".into(), conn_core::engine::default_shell());
        p.name="Native shell · browser test".into();p.cwd=Some(workspace.to_string_lossy().into());
        std::fs::write(dir.join("profiles.json"),serde_json::to_vec_pretty(&conn_core::profiles::Profiles {version:1,revision:0,default_profile:p.id.clone(),profiles:vec![p]})?)?;
    }
    PolicyStore::open(&dir.join("policy.yaml"))?;
    // A missing policy must never silently fall back to a simulated or permissive web policy.
    conn_core::policy::Policy::load(&dir.join("policy.yaml"))?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:1423").await?;
    let token = uuid::Uuid::new_v4().to_string();
    std::fs::write(dir.join("connection.json"),serde_json::to_vec(&json!({"url":"ws://127.0.0.1:1423/ws","token":token}))?)?;
    let server=Server{token,origin:"http://127.0.0.1:1421".into(),dir:dir.clone(),connected:Arc::new(AtomicBool::new(false))};
    println!("Conn shared harness: 127.0.0.1:1423; isolated state: {}",dir.display());
    axum::serve(listener,Router::new().route("/ws",get(upgrade)).with_state(server)).await?;
    Ok(())
}
