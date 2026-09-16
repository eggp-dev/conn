//! Native window adapter; command behavior lives in conn-frontend.
use conn_frontend::Harness;
use serde_json::Value;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod windows;

#[tauri::command]
async fn dispatch(
    window: tauri::WebviewWindow,
    state: State<'_, Arc<Harness>>,
    name: String,
    args: Value,
) -> Result<Value, String> {
    let harness = state.inner().clone();
    let label = window.label().to_owned();
    tauri::async_runtime::spawn_blocking(move || harness.invoke_in_window(&label, &name, args))
        .await
        .map_err(|e| e.to_string())?
}
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![dispatch])
        .setup(|app| {
            let handle = app.handle().clone();
            let harness = Arc::new(Harness::new(
                conn_core::paths::config_dir(),
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
            app.manage(harness);
            Ok(())
        })
        .on_window_event(|window, event| {
            let Some(harness) = window.try_state::<Arc<Harness>>() else {
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
                app.state::<Arc<Harness>>().shutdown();
            }
        });
}
