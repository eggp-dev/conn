#![cfg(unix)]
use base64::Engine as _;
use conn_web::{Config, WebHost};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
};
type Socket = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

struct TestServer(std::process::Child);
impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn sigterm_ends_live_owner_socket_and_shell_without_client_close() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state");
    std::fs::create_dir(&state).unwrap();
    let ui = dir.path().join("ui");
    std::fs::create_dir(&ui).unwrap();
    std::fs::write(ui.join("index.html"), "Conn shutdown test").unwrap();
    let pid_file = dir.path().join("shell.pid");
    std::fs::write(state.join("profiles.json"), json!({"version":1,"revision":0,"defaultProfile":"test","profiles":[{"id":"test","name":"Test","program":"/bin/sh","args":[],"shell":"posix","cwd":dir.path()}]}).to_string()).unwrap();
    let mut server = TestServer(
        std::process::Command::new(env!("CARGO_BIN_EXE_conn-web"))
            .args(["serve", "--port", "0", "--state-dir"])
            .arg(&state)
            .arg("--ui-dir")
            .arg(ui)
            .env("CONN_SHUTDOWN_PID", &pid_file)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let metadata: Value = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(bytes) = std::fs::read(state.join("connection.json")) {
                if let Ok(metadata) = serde_json::from_slice(&bytes) {
                    break metadata;
                }
            }
            assert!(
                server.0.try_wait().unwrap().is_none(),
                "Server exited before startup"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let origin = metadata["url"].as_str().unwrap();
    let cookie = authenticate(origin, metadata["bootstrapToken"].as_str().unwrap()).await;
    let (mut socket, hello) = connect(origin, &cookie, false).await;
    let epoch = hello["epoch"].as_u64().unwrap();
    let (started, _) = call(
        &mut socket,
        epoch,
        1,
        "start",
        json!({"rows":24,"cols":80}),
        None,
    )
    .await;
    let session = started["result"]["session"].clone();
    assert!(session.is_string(), "{started}");
    let (attached, _) = call(
        &mut socket,
        epoch,
        2,
        "attach_output",
        json!({"session":session}),
        None,
    )
    .await;
    assert!(attached.get("error").is_none(), "{attached}");
    let (input, _) = call(
        &mut socket,
        epoch,
        3,
        "input",
        json!({"session":session,"data":"printf '%s\\n' \"$$\" > \"$CONN_SHUTDOWN_PID\"\r"}),
        Some(1),
    )
    .await;
    assert!(input.get("error").is_none(), "{input}");
    let shell_pid: i32 = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(text) = std::fs::read_to_string(&pid_file) {
                if let Ok(pid) = text.trim().parse() {
                    break pid;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        unsafe { libc::kill(shell_pid, 0) },
        0,
        "PTY shell was not alive before SIGTERM"
    );
    assert_eq!(
        unsafe { libc::kill(server.0.id() as i32, libc::SIGTERM) },
        0
    );
    // Keep the owner WebSocket open; the server must initiate termination itself.
    let exit = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(status) = server.0.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Server did not stop while its owner WebSocket stayed open");
    assert!(exit.success(), "{exit}");
    let mut stopping = false;
    tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(Ok(message)) = socket.next().await {
            match message {
                Message::Text(text) => {
                    let value: Value = serde_json::from_str(&text).unwrap();
                    stopping |= value["type"] == "server_stopping";
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    assert!(stopping, "Server closed the owner without a stopping event");
    tokio::time::timeout(Duration::from_secs(2), async {
        while unsafe { libc::kill(shell_pid, 0) } == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("PTY shell survived its server");
}
async fn http_get(origin: &str, path: &str, host: Option<&str>) -> String {
    let addr = origin.trim_start_matches("http://");
    let mut tcp = TcpStream::connect(addr).await.unwrap();
    tcp.write_all(
        format!(
            "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            host.unwrap_or(addr)
        )
        .as_bytes(),
    )
    .await
    .unwrap();
    let mut out = Vec::new();
    tcp.read_to_end(&mut out).await.unwrap();
    String::from_utf8(out).unwrap()
}
async fn authenticate(origin: &str, token: &str) -> String {
    let addr = origin.trim_start_matches("http://");
    let mut tcp = TcpStream::connect(addr).await.unwrap();
    let body = json!({"token":token}).to_string();
    let req=format!("POST /api/bootstrap HTTP/1.1\r\nHost: {addr}\r\nOrigin: {origin}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
    tcp.write_all(req.as_bytes()).await.unwrap();
    let mut out = Vec::new();
    tcp.read_to_end(&mut out).await.unwrap();
    let response = String::from_utf8(out).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    response
        .lines()
        .find_map(|line| line.strip_prefix("set-cookie: "))
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .into()
}
async fn connect(origin: &str, cookie: &str, takeover: bool) -> (Socket, Value) {
    let mut request = format!("{}/api/ws", origin.replace("http:", "ws:"))
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("origin", origin.parse().unwrap());
    request
        .headers_mut()
        .insert("cookie", cookie.parse().unwrap());
    let (mut socket, _) = connect_async(request).await.unwrap();
    socket
        .send(Message::Text(
            json!({"type":"hello","protocol":1,"takeover":takeover})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let hello = receive(&mut socket).await;
    (socket, hello)
}
async fn receive(socket: &mut Socket) -> Value {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        if let Message::Text(text) = message {
            return serde_json::from_str(&text).unwrap();
        }
    }
}
async fn call(
    socket: &mut Socket,
    epoch: u64,
    id: u64,
    name: &str,
    args: Value,
    sequence: Option<u64>,
) -> (Value, Vec<Value>) {
    socket.send(Message::Text(json!({"id":id,"operationId":id,"epoch":epoch,"name":name,"args":args,"sequence":sequence}).to_string().into())).await.unwrap();
    let mut events = Vec::new();
    loop {
        let v = receive(socket).await;
        if v["id"] == id {
            return (v, events);
        }
        events.push(v);
    }
}
#[tokio::test(flavor = "multi_thread")]
async fn reload_and_explicit_handoff_keep_shell_but_fence_old_attachment() {
    let dir = tempfile::tempdir().unwrap();
    let ui = dir.path().join("ui");
    std::fs::create_dir(&ui).unwrap();
    std::fs::write(ui.join("index.html"), "<!doctype html>Conn test").unwrap();
    let state = dir.path().join("state");
    std::fs::create_dir(&state).unwrap();
    std::fs::write(state.join("profiles.json"),json!({"version":1,"revision":0,"defaultProfile":"test","profiles":[{"id":"test","name":"Test","program":"/bin/sh","args":[],"shell":"posix","cwd":dir.path()}]}).to_string()).unwrap();
    let host = WebHost::bind(Config {
        state_dir: state,
        host: "127.0.0.1".into(),
        port: 0,
        ui_dir: Some(ui),
        dev_origin: None,
        setup_home: Some(dir.path().join("clients")),
        connection_file: None,
    })
    .await
    .unwrap();
    let origin = host.origin().to_owned();
    let runtime = host.runtime();
    let metadata: Value =
        serde_json::from_slice(&std::fs::read(host.connection_file()).unwrap()).unwrap();
    let serve = tokio::spawn(host.serve());
    let page = http_get(&origin, "/", None).await;
    assert!(page.starts_with("HTTP/1.1 200") && page.contains("Conn test"));
    assert!(http_get(&origin, "/", Some("attacker.invalid"))
        .await
        .starts_with("HTTP/1.1 403"));
    assert!(http_get(&origin, "/%2e%2e/connection.json", None)
        .await
        .starts_with("HTTP/1.1 404"));
    let mut unauth = format!("{}/api/ws", origin.replace("http:", "ws:"))
        .into_client_request()
        .unwrap();
    unauth
        .headers_mut()
        .insert("origin", origin.parse().unwrap());
    assert!(connect_async(unauth).await.is_err());
    let cookie = authenticate(&origin, metadata["bootstrapToken"].as_str().unwrap()).await;
    let mut bad_origin = format!("{}/api/ws", origin.replace("http:", "ws:"))
        .into_client_request()
        .unwrap();
    bad_origin
        .headers_mut()
        .insert("origin", "http://attacker.invalid".parse().unwrap());
    bad_origin
        .headers_mut()
        .insert("cookie", cookie.parse().unwrap());
    assert!(connect_async(bad_origin).await.is_err());
    let mut old_query = format!(
        "{}/api/ws?token=not-accepted",
        origin.replace("http:", "ws:")
    )
    .into_client_request()
    .unwrap();
    old_query
        .headers_mut()
        .insert("origin", origin.parse().unwrap());
    old_query
        .headers_mut()
        .insert("cookie", cookie.parse().unwrap());
    assert!(connect_async(old_query).await.is_err());

    let (mut first, hello) = connect(&origin, &cookie, false).await;
    assert_eq!(hello["type"], "hello");
    let epoch = hello["epoch"].as_u64().unwrap();
    let (start, _) = call(
        &mut first,
        epoch,
        1,
        "start",
        json!({"rows":24,"cols":80}),
        None,
    )
    .await;
    assert!(start.get("error").is_none(), "{start}");
    let id = start["result"]["session"].clone();
    let (attached, frames) = call(
        &mut first,
        epoch,
        2,
        "attach_output",
        json!({"session":id}),
        None,
    )
    .await;
    assert!(attached.get("error").is_none(), "{attached}");
    assert!(
        frames.iter().any(|v| v["payload"]["reset"] == true),
        "{frames:?}"
    );
    let (input, _) = call(
        &mut first,
        epoch,
        3,
        "input",
        json!({"session":id,"data":"CONN_REATTACH=kept; printf 'BEFORE_RELOAD\\n'\r"}),
        Some(1),
    )
    .await;
    assert!(input.get("error").is_none());
    tokio::time::sleep(Duration::from_millis(100)).await;
    // A real blocking connection check must not stall owner input or PTY events.
    let delayed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let slow_port = delayed.local_addr().unwrap().port();
    let (accepted, ready) = tokio::sync::oneshot::channel();
    let (release, hold) = tokio::sync::oneshot::channel();
    let peer = tokio::spawn(async move {
        let (mut socket, _) = delayed.accept().await.unwrap();
        let mut bytes = [0; 256];
        let _ = socket.read(&mut bytes).await;
        let _ = accepted.send(());
        let _ = hold.await;
    });
    first.send(Message::Text(json!({"id":10,"operationId":10,"epoch":epoch,"name":"profiles_test","args":{"profile":{"id":"slow","name":"Slow loopback greeting","backend":"ssh","target":"127.0.0.1","port":slow_port}}}).to_string().into())).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), ready)
        .await
        .unwrap()
        .unwrap();
    let (input, events) = call(
        &mut first,
        epoch,
        11,
        "input",
        json!({"session":id,"data":"printf 'INPUT_WHILE_PROFILE_PENDING\\n'\r"}),
        Some(2),
    )
    .await;
    assert!(input.get("error").is_none(), "{input}");
    assert!(!events.iter().any(|v| v["id"] == 10));
    let mut live = events
        .iter()
        .filter_map(|v| v["payload"]["data"].as_str())
        .flat_map(|data| {
            base64::engine::general_purpose::STANDARD
                .decode(data)
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    while !String::from_utf8_lossy(&live).contains("INPUT_WHILE_PROFILE_PENDING\r\n") {
        let event = receive(&mut first).await;
        assert_ne!(event["id"], 10);
        if let Some(data) = event["payload"]["data"].as_str() {
            live.extend(
                base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .unwrap(),
            );
        }
    }
    release.send(()).unwrap();
    peer.await.unwrap();
    loop {
        if receive(&mut first).await["id"] == 10 {
            break;
        }
    }

    let (mut duplicate, rejected) = connect(&origin, &cookie, false).await;
    assert_eq!(rejected["error"], "owner_attached");
    let _ = duplicate.close(None).await;
    let (mut second, next) = connect(&origin, &cookie, true).await;
    let epoch2 = next["epoch"].as_u64().unwrap();
    assert!(epoch2 > epoch);
    assert_eq!(hello["runtimeId"], next["runtimeId"]);
    let (start, _) = call(
        &mut second,
        epoch2,
        1,
        "start",
        json!({"rows":24,"cols":80}),
        None,
    )
    .await;
    assert_eq!(start["result"]["session"], id);
    let (_, frames) = call(
        &mut second,
        epoch2,
        2,
        "attach_output",
        json!({"session":id}),
        None,
    )
    .await;
    let checkpoint = frames
        .iter()
        .find(|v| v["payload"]["reset"] == true)
        .unwrap();
    assert_eq!(checkpoint["payload"]["epoch"], epoch2);
    let (old, _) = call(
        &mut second,
        epoch,
        3,
        "input",
        json!({"session":id,"data":"BAD\r"}),
        Some(2),
    )
    .await;
    assert_eq!(old["error"], "attachment_fenced");
    let _ = first.close(None).await;
    let (input, events) = call(
        &mut second,
        epoch2,
        3,
        "input",
        json!({"session":id,"data":"printf 'AFTER_%s\\n' \"$CONN_REATTACH\"\r"}),
        Some(1),
    )
    .await;
    assert!(input.get("error").is_none());
    // The output bytes are real PTY frames. Retained shell state proves no new process was spawned.
    let mut bytes = Vec::new();
    for event in events {
        if event["event"] == "ss:output" {
            bytes.extend(
                base64::engine::general_purpose::STANDARD
                    .decode(event["payload"]["data"].as_str().unwrap())
                    .unwrap(),
            );
        }
    }
    for _ in 0..20 {
        if String::from_utf8_lossy(&bytes).contains("AFTER_kept") {
            break;
        }
        let event = receive(&mut second).await;
        if event["event"] == "ss:output" {
            bytes.extend(
                base64::engine::general_purpose::STANDARD
                    .decode(event["payload"]["data"].as_str().unwrap())
                    .unwrap(),
            );
        }
    }
    assert!(
        String::from_utf8_lossy(&bytes).contains("AFTER_kept"),
        "{bytes:?}"
    );
    let _ = second.close(None).await;
    tokio::time::sleep(Duration::from_millis(80)).await;
    let (mut third, resumed) = connect(&origin, &cookie, false).await;
    assert_eq!(resumed["type"], "hello");
    let epoch3 = resumed["epoch"].as_u64().unwrap();
    let (start, _) = call(
        &mut third,
        epoch3,
        1,
        "start",
        json!({"rows":24,"cols":80}),
        None,
    )
    .await;
    assert_eq!(start["result"]["session"], id);
    let _ = third.close(None).await;
    // Competing explicit takeovers must install delivery for the newest epoch.
    // Neither an older registration nor its late disconnect may detach the winner.
    for _ in 0..8 {
        let ((mut left, a), (mut right, b)) = tokio::join!(
            connect(&origin, &cookie, true),
            connect(&origin, &cookie, true)
        );
        let newest = a["epoch"]
            .as_u64()
            .unwrap()
            .max(b["epoch"].as_u64().unwrap());
        let (winner, loser) = if a["epoch"].as_u64().unwrap() == newest {
            (&mut left, &mut right)
        } else {
            (&mut right, &mut left)
        };
        let _ = loser.close(None).await;
        let (response, frames) = call(
            winner,
            newest,
            1,
            "attach_output",
            json!({"session":id}),
            None,
        )
        .await;
        assert!(response.get("error").is_none(), "{response}");
        assert!(
            frames
                .iter()
                .any(|v| v["payload"]["reset"] == true && v["payload"]["epoch"] == newest),
            "{frames:?}"
        );
        let _ = winner.close(None).await;
    }
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap();
    serve.abort();
}
