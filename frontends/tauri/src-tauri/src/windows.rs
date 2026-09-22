//! Native presentation; sessions and ownership stay in the shared AppRuntime.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub fn show(app: &AppHandle, label: &str) -> Result<(), String> {
    let (send, receive) = mpsc::sync_channel(1);
    let handle = app.clone();
    let label = label.to_owned();
    let cancelled = Arc::new(AtomicBool::new(false));
    let task_cancelled = cancelled.clone();
    app.run_on_main_thread(move || {
        if task_cancelled.load(Ordering::Acquire) {
            return;
        }
        let result = (|| {
            let window = match handle.get_webview_window(&label) {
                Some(window) => window,
                None => {
                    WebviewWindowBuilder::new(&handle, &label, WebviewUrl::App("index.html".into()))
                        .title("Conn")
                        .inner_size(1120.0, 720.0)
                        .min_inner_size(720.0, 420.0)
                        .focused(true)
                        .build()
                        .map_err(|e| e.to_string())?
                }
            };
            if task_cancelled.load(Ordering::Acquire) {
                let _ = window.destroy();
                return Err("Window creation cancelled".into());
            }
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())
        })();
        let _ = send.send(result);
    })
    .map_err(|e| e.to_string())?;
    receive
        .recv_timeout(Duration::from_secs(15))
        .unwrap_or_else(|_| {
            cancelled.store(true, Ordering::Release);
            Err("Timed out creating the native window".into())
        })
}

pub fn active_label(app: &AppHandle) -> String {
    let windows = app.webview_windows();
    windows
        .values()
        .find(|w| w.is_focused().unwrap_or(false))
        .or_else(|| windows.get("main"))
        .or_else(|| windows.values().next())
        .map(|w| w.label().to_owned())
        .unwrap_or_else(|| "main".into())
}

pub fn discard(app: &AppHandle, label: &str) {
    let handle = app.clone();
    let label = label.to_owned();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = handle.get_webview_window(&label) {
            let _ = window.destroy();
        }
    });
}
