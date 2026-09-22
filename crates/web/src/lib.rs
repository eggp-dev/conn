//! Authenticated loopback host for the production AppRuntime. A socket attaches
//! a renderer; neither a socket nor a page reload owns a shell's lifetime.
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{HeaderMap, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use conn_frontend::{AppRuntime, OwnerMeta, OwnerRequest, PROTOCOL_VERSION};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{mpsc, watch, Semaphore};

pub struct Config {
    pub state_dir: PathBuf,
    pub host: String,
    pub port: u16,
    pub ui_dir: Option<PathBuf>,
    pub dev_origin: Option<String>,
    pub setup_home: Option<PathBuf>,
    pub connection_file: Option<PathBuf>,
}
#[derive(Clone)]
struct Client {
    epoch: u64,
    tx: mpsc::Sender<Value>,
    stop: watch::Sender<Option<&'static str>>,
    closed: watch::Receiver<bool>,
}
struct SocketCompletion(watch::Sender<bool>);
impl Drop for SocketCompletion {
    fn drop(&mut self) {
        self.0.send_replace(true);
    }
}
#[derive(Default)]
struct Delivery {
    client: parking_lot::Mutex<Option<Client>>,
}
impl Delivery {
    fn emit(&self, name: &str, payload: Value) {
        if let Some(client) = self.client.lock().as_ref() {
            if payload
                .get("epoch")
                .and_then(Value::as_u64)
                .is_some_and(|epoch| epoch != client.epoch)
            {
                return;
            }
            if client
                .tx
                .try_send(json!({"event":name,"payload":payload,"epoch":client.epoch}))
                .is_err()
            {
                let _ = client.stop.send(Some("resync_required"));
            }
        }
    }
}
struct Shared {
    runtime: Arc<AppRuntime>,
    delivery: Arc<Delivery>,
    origin: String,
    dev_origin: Option<String>,
    token: String,
    cookie: String,
    cookie_value: String,
    ui_dir: Option<PathBuf>,
    _lock: std::fs::File,
    attachment: tokio::sync::Mutex<()>,
    stopping: AtomicBool,
}
pub struct WebHost {
    listener: tokio::net::TcpListener,
    shared: Arc<Shared>,
    connection_file: PathBuf,
}
impl WebHost {
    pub async fn bind(config: Config) -> Result<Self, Box<dyn std::error::Error>> {
        // Remote hosting is intentionally not enabled by a bind-address option.
        if config.host != "127.0.0.1" {
            return Err("Only 127.0.0.1 is supported by the local web host".into());
        }
        if let Some(origin) = &config.dev_origin {
            validate_origin(origin)?;
        }
        let ui_dir = config
            .ui_dir
            .filter(|path| path.join("index.html").is_file())
            .map(|p| p.canonicalize())
            .transpose()?;
        if ui_dir.is_none() && config.dev_origin.is_none() {
            return Err("Built web UI missing; provide --ui-dir with index.html".into());
        }
        std::fs::create_dir_all(&config.state_dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&config.state_dir, std::fs::Permissions::from_mode(0o700))?;
        }
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(config.state_dir.join("web.lock"))?;
        lock.try_lock()
            .map_err(|_| "This web state directory is already in use")?;
        let listener = tokio::net::TcpListener::bind((config.host.as_str(), config.port)).await?;
        let origin = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
        let delivery = Arc::new(Delivery::default());
        let output = delivery.clone();
        let socket = config.state_dir.join("conn.sock");
        let runtime = Arc::new(AppRuntime::with_setup_home(
            config.state_dir.clone(),
            socket.clone(),
            Arc::new(move |name, payload| output.emit(name, payload)),
            config.setup_home,
        ));
        let token = uuid::Uuid::new_v4().to_string();
        let cookie_value = uuid::Uuid::new_v4().to_string();
        let cookie = format!("conn_owner_{}", runtime.runtime_id().replace('-', ""));
        let connection_file = config
            .connection_file
            .unwrap_or_else(|| config.state_dir.join("connection.json"));
        let metadata = json!({"url":origin,"wsUrl":format!("{}/api/ws",origin.replace("http:","ws:")),"bootstrapToken":token,
            "runtimeId":runtime.runtime_id(),"protocol":PROTOCOL_VERSION,"buildVersion":env!("CARGO_PKG_VERSION"),"agentSocket":socket});
        if let Some(parent) = connection_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        use std::io::Write;
        let mut file = options.open(&connection_file)?;
        file.write_all(serde_json::to_string_pretty(&metadata)?.as_bytes())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(Self {
            listener,
            connection_file,
            shared: Arc::new(Shared {
                runtime,
                delivery,
                origin,
                dev_origin: config.dev_origin,
                token,
                cookie,
                cookie_value,
                ui_dir,
                _lock: lock,
                attachment: tokio::sync::Mutex::new(()),
                stopping: AtomicBool::new(false),
            }),
        })
    }
    pub fn origin(&self) -> &str {
        &self.shared.origin
    }
    pub fn connection_file(&self) -> &std::path::Path {
        &self.connection_file
    }
    pub fn runtime(&self) -> Arc<AppRuntime> {
        self.shared.runtime.clone()
    }
    pub async fn serve(self) -> Result<(), std::io::Error> {
        let shared = self.shared.clone();
        let shutdown = shared.clone();
        let result = axum::serve(
            self.listener,
            Router::new()
                .route("/api/bootstrap", post(bootstrap))
                .route("/api/info", get(info))
                .route("/api/ws", get(upgrade))
                .fallback(static_file)
                .with_state(self.shared),
        )
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            // Stop upgraded owner connections before asking HTTP to drain.
            shutdown.begin_shutdown();
        })
        .await;
        shared.begin_shutdown();
        let closed = shared
            .delivery
            .client
            .lock()
            .as_ref()
            .map(|client| client.closed.clone());
        if let Some(mut closed) = closed {
            // Flush the stopping event/close even when the page remains open.
            // An unresponsive browser cannot hold process termination forever.
            let _ = tokio::time::timeout(Duration::from_secs(2), async {
                while !*closed.borrow_and_update() {
                    if closed.changed().await.is_err() {
                        break;
                    }
                }
            })
            .await;
        }
        let _ = tokio::task::spawn_blocking(move || shared.runtime.shutdown()).await;
        result
    }
}
fn validate_origin(origin: &str) -> Result<(), &'static str> {
    let port = origin
        .strip_prefix("http://127.0.0.1:")
        .and_then(|s| s.parse::<u16>().ok())
        .filter(|p| *p != 0)
        .ok_or("Development origin must be http://127.0.0.1:<port>")?;
    if origin != format!("http://127.0.0.1:{port}") {
        return Err("Invalid development origin");
    }
    Ok(())
}
impl Shared {
    fn begin_shutdown(&self) {
        self.stopping.store(true, Ordering::Release);
        if let Some(client) = self.delivery.client.lock().as_ref() {
            let _ = client.stop.send(Some("server_stopping"));
        }
    }
    fn host_valid(&self, headers: &HeaderMap) -> bool {
        headers
            .get("host")
            .and_then(|h| h.to_str().ok())
            .is_some_and(|h| {
                h == self.origin.trim_start_matches("http://")
                    || self
                        .dev_origin
                        .as_ref()
                        .is_some_and(|o| h == o.trim_start_matches("http://"))
            })
    }
    fn origin_valid(&self, headers: &HeaderMap) -> bool {
        headers
            .get("origin")
            .and_then(|h| h.to_str().ok())
            .is_some_and(|o| o == self.origin || self.dev_origin.as_deref() == Some(o))
    }
    fn authorized(&self, headers: &HeaderMap) -> bool {
        headers
            .get("cookie")
            .and_then(|h| h.to_str().ok())
            .is_some_and(|cookies| {
                cookies
                    .split(';')
                    .any(|part| part.trim() == format!("{}={}", self.cookie, self.cookie_value))
            })
    }
}
#[derive(Deserialize)]
struct Bootstrap {
    token: String,
}
async fn bootstrap(
    State(s): State<Arc<Shared>>,
    headers: HeaderMap,
    Json(body): Json<Bootstrap>,
) -> Response {
    if !s.host_valid(&headers) || !s.origin_valid(&headers) || body.token != s.token {
        return (StatusCode::FORBIDDEN, "Owner authentication failed").into_response();
    }
    let mut response = Json(json!({"authenticated":true})).into_response();
    response.headers_mut().insert(
        "set-cookie",
        HeaderValue::from_str(&format!(
            "{}={}; HttpOnly; SameSite=Strict; Path=/api; Max-Age=86400",
            s.cookie, s.cookie_value
        ))
        .unwrap(),
    );
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    response
}
async fn info(State(s): State<Arc<Shared>>, headers: HeaderMap) -> Response {
    if !s.host_valid(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    Json(json!({"protocol":PROTOCOL_VERSION,"runtimeId":s.runtime.runtime_id(),"buildVersion":env!("CARGO_PKG_VERSION"),"authenticated":s.authorized(&headers)})).into_response()
}
async fn upgrade(
    State(s): State<Arc<Shared>>,
    headers: HeaderMap,
    uri: Uri,
    ws: WebSocketUpgrade,
) -> Response {
    if s.stopping.load(Ordering::Acquire) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    if uri.query().is_some()
        || !s.host_valid(&headers)
        || !s.origin_valid(&headers)
        || !s.authorized(&headers)
    {
        return (StatusCode::FORBIDDEN, "Owner authentication required").into_response();
    }
    ws.max_message_size(2 * 1024 * 1024)
        .on_upgrade(move |socket| serve_socket(socket, s))
}
async fn serve_socket(mut socket: WebSocket, s: Arc<Shared>) {
    let Some(Ok(Message::Text(text))) =
        tokio::time::timeout(std::time::Duration::from_secs(10), socket.recv())
            .await
            .ok()
            .flatten()
    else {
        return;
    };
    let Ok(hello) = serde_json::from_str::<Value>(&text) else {
        return;
    };
    if hello["type"] != "hello" {
        return;
    }
    let protocol = hello["protocol"]
        .as_u64()
        .and_then(|p| u32::try_from(p).ok())
        .unwrap_or(0);
    let takeover = hello["takeover"].as_bool().unwrap_or(false);
    let runtime = s.runtime.clone();
    // Epoch replacement and delivery installation are one attachment operation.
    // Otherwise two concurrent takeovers could install the older emitter last.
    let registration = s.attachment.lock().await;
    if s.stopping.load(Ordering::Acquire) {
        return;
    }
    let attached =
        tokio::task::spawn_blocking(move || runtime.owner_attach("main", protocol, takeover)).await;
    let hello = match attached {
        Ok(Ok(v)) => v,
        other => {
            drop(registration);
            let error = match other {
                Ok(Err(e)) => e,
                Err(e) => e.to_string(),
                _ => unreachable!(),
            };
            let _ = socket
                .send(Message::Text(
                    json!({"type":"error","error":error}).to_string().into(),
                ))
                .await;
            return;
        }
    };
    let epoch = hello.epoch;
    if s.stopping.load(Ordering::Acquire) {
        drop(registration);
        let _ = tokio::task::spawn_blocking(move || s.runtime.owner_detach("main", epoch)).await;
        return;
    }
    let (tx, mut rx) = mpsc::channel::<Value>(1024);
    let (stop, mut stopped) = watch::channel(None);
    let (completed, closed) = watch::channel(false);
    let _completion = SocketCompletion(completed);
    {
        let mut current = s.delivery.client.lock();
        if let Some(old) = current.replace(Client {
            epoch,
            tx: tx.clone(),
            stop: stop.clone(),
            closed,
        }) {
            let _ = old.stop.send(Some("attachment_fenced"));
        }
    }
    drop(registration);
    let mut hello = serde_json::to_value(hello).unwrap();
    hello["type"] = json!("hello");
    if socket
        .send(Message::Text(hello.to_string().into()))
        .await
        .is_err()
    {
        s.runtime.owner_detach("main", epoch);
        return;
    }
    let (mut writer, mut reader) = socket.split();
    let writer_stop = stop.clone();
    let writer_task = tokio::spawn(async move {
        loop {
            tokio::select! {biased;
                changed=stopped.changed()=>{if changed.is_err(){break;}let reason=*stopped.borrow_and_update();if let Some(reason)=reason{let _=tokio::time::timeout(Duration::from_millis(500),writer.send(Message::Text(json!({"type":reason,"epoch":epoch}).to_string().into()))).await;break;}},
                value=rx.recv()=>{let Some(value)=value else{break;};if !matches!(tokio::time::timeout(Duration::from_millis(500),writer.send(Message::Text(value.to_string().into()))).await,Ok(Ok(()))){break;}}
            }
        }
        let _ = tokio::time::timeout(Duration::from_millis(500), writer.close()).await;
        writer_stop.send_if_modified(|reason| {
            if reason.is_none() {
                *reason = Some("disconnected");
                true
            } else {
                false
            }
        });
    });
    let limit = Arc::new(Semaphore::new(64));
    let mut reader_stop = stop.subscribe();
    loop {
        if reader_stop.borrow().is_some() {
            break;
        }
        let message =
            tokio::select! { _=reader_stop.changed()=>break, message=reader.next()=>message };
        let Some(Ok(message)) = message else {
            break;
        };
        let Message::Text(text) = message else {
            if matches!(message, Message::Close(_)) {
                break;
            }
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            break;
        };
        let id = v["id"].clone();
        let tx = tx.clone();
        let runtime = s.runtime.clone();
        let host = s.clone();
        let Ok(permit) = limit.clone().try_acquire_owned() else {
            let _ = tx
                .send(json!({"id":id,"error":"owner_busy","code":"owner_busy"}))
                .await;
            continue;
        };
        tokio::spawn(async move {
            let _permit = permit;
            let result=tokio::task::spawn_blocking(move||{
                if host.stopping.load(Ordering::Acquire) {return Err("server_stopping".into());}
                if v["type"]=="outcome" {return runtime.owner_outcome("main",v["epoch"].as_u64().unwrap_or(0),v["operationId"].as_u64().unwrap_or(0));}
                let request=OwnerRequest{name:v["name"].as_str().unwrap_or("").to_owned(),args:v["args"].clone(),meta:serde_json::from_value::<OwnerMeta>(json!({"epoch":v["epoch"],"operationId":v["operationId"],"sequence":v["sequence"]})).map_err(|_|"invalid_owner_request")?};
                if request.meta.epoch!=epoch{return Err("attachment_fenced".into());}
                runtime.invoke_owner("main",request)
            }).await;
            let response = match result {
                Ok(Ok(value)) => json!({"id":id,"result":value}),
                Ok(Err(error)) => json!({"id":id,"error":error,"code":error}),
                Err(_) => json!({"id":id,"error":"outcome_unknown","code":"outcome_unknown"}),
            };
            let _ = tx.send(response).await;
        });
    }
    stop.send_if_modified(|reason| {
        if reason.is_none() {
            *reason = Some("disconnected");
            true
        } else {
            false
        }
    });
    let _ = writer_task.await;
    {
        let mut client = s.delivery.client.lock();
        if client.as_ref().is_some_and(|c| c.epoch == epoch) {
            client.take();
        }
    }
    let _ = tokio::task::spawn_blocking(move || s.runtime.owner_detach("main", epoch)).await;
}
async fn static_file(State(s): State<Arc<Shared>>, headers: HeaderMap, uri: Uri) -> Response {
    if !s.host_valid(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if uri.path().starts_with("/api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(root) = s.ui_dir.as_ref() else {
        return (
            StatusCode::NOT_FOUND,
            "Use the configured development frontend",
        )
            .into_response();
    };
    let Ok(path) = percent_encoding::percent_decode_str(uri.path()).decode_utf8() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let relative = path.trim_start_matches('/');
    if std::path::Path::new(relative)
        .components()
        .any(|p| !matches!(p, std::path::Component::Normal(_)))
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let candidate = root.join(if relative.is_empty() {
        "index.html"
    } else {
        relative
    });
    let file = if candidate.is_file() {
        candidate
    } else if std::path::Path::new(relative).extension().is_none() {
        root.join("index.html")
    } else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(file) = file.canonicalize() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !file.starts_with(root) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Ok(bytes) = tokio::fs::read(&file).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mime = match file.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    };
    (
        [
            ("content-type", mime),
            ("cache-control", "no-cache"),
            ("x-content-type-options", "nosniff"),
        ],
        bytes,
    )
        .into_response()
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! {_=tokio::signal::ctrl_c()=>{},_=terminate.recv()=>{}};
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
