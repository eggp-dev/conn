#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Before GTK/WebKit or any application threads start. DMA-BUF negotiation
    // can abort WebKit on Linux GPU/driver combinations before a window exists.
    // Honor explicit user overrides; this does not disable the WebKit sandbox.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    conn_desktop_lib::run()
}
