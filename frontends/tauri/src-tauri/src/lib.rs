//! Native window adapter; command behavior lives in conn-frontend.
use std::sync::Arc;
use serde_json::Value;
use tauri::{Emitter, Manager, State};
use conn_frontend::Harness;

#[tauri::command]
async fn dispatch(state: State<'_, Arc<Harness>>, name: String, args: Value) -> Result<Value,String> {
    let harness = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || harness.invoke(&name,args)).await.map_err(|e|e.to_string())?
}
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![dispatch])
        .setup(|app| {
            let handle = app.handle().clone();
            app.manage(Arc::new(Harness::new(conn_core::paths::config_dir(), conn_core::paths::socket_path(), Arc::new(move |name,value| { let _ = handle.emit(name,value); }))));
            Ok(())
        })
        .on_window_event(|window,event| { if let tauri::WindowEvent::Destroyed = event { window.state::<Arc<Harness>>().shutdown(); } })
        .run(tauri::generate_context!())
        .expect("error while running Conn");
}
