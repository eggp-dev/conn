//! Native window adapter; command behavior lives in conn-frontend.
use conn_frontend::{AppRuntime, OwnerHello, OwnerMeta, OwnerRequest};
use serde_json::Value;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};
mod updates;
#[cfg(test)]
mod update_tests;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod windows;
#[cfg(target_os = "linux")]
mod linux;

#[tauri::command]
async fn dispatch(
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppRuntime>>,
    name: String,
    args: Value,
    meta: OwnerMeta,
) -> Result<Value, String> {
    let harness = state.inner().clone();
    let label = window.label().to_owned();
    tauri::async_runtime::spawn_blocking(move || harness.invoke_owner(&label, OwnerRequest { name, args, meta }))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn owner_attach(window: tauri::WebviewWindow, state: State<'_, Arc<AppRuntime>>, protocol: u32, takeover: bool) -> Result<OwnerHello, String> {
    let runtime = state.inner().clone(); let label = window.label().to_owned();
    tauri::async_runtime::spawn_blocking(move || runtime.owner_attach(&label, protocol, takeover)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
async fn owner_outcome(window: tauri::WebviewWindow, state: State<'_, Arc<AppRuntime>>, epoch: u64, operation_id: u64) -> Result<Value, String> {
    state.owner_outcome(window.label(), epoch, operation_id)
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::Updates::new())
        .invoke_handler(tauri::generate_handler![dispatch, owner_attach, owner_outcome, updates::app_update])
        .setup(|app| {
            let handle = app.handle().clone();
            let harness = Arc::new(AppRuntime::new(
                std::env::var_os("CONN_CONFIG_DIR").map(std::path::PathBuf::from).unwrap_or_else(conn_core::paths::config_dir),
                conn_core::paths::socket_path(),
                Arc::new(move |name, value| {
                    if let Some(window) = value["window"].as_str().map(str::to_owned) {
                        let _ = handle.emit_to(window, name, value);
                    } else {
                        let _ = handle.emit(name, value);
                    }
                }),
            ));
            #[cfg(target_os = "macos")]
            macos::install(app.handle(), &harness);
            #[cfg(target_os = "linux")]
            linux::install(app.handle(), &harness);
            app.manage(harness);
            Ok(())
        })
        .on_window_event(|window, event| {
            let Some(harness) = window.try_state::<Arc<AppRuntime>>() else {
                return;
            };
            match event {
                tauri::WindowEvent::Focused(true) => harness.focus_window(window.label()),
                tauri::WindowEvent::Destroyed => harness.close_window(window.label()),
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building Conn")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<Arc<AppRuntime>>().shutdown();
            }
        });
}
