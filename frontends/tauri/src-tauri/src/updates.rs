//! Native, signed updates. No arbitrary endpoint or installer is accepted from IPC.
use parking_lot::Mutex;
use semver::Version;
use serde::Serialize;
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tauri::{Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

const RELEASES: &str = "https://github.com/eggp-dev/conn/releases";
/// Where the repository lived until it moved to the organization. Builds up to 0.8.1 trust only
/// this address, so update manifests keep using it and this build has to accept it too.
const LEGACY_RELEASES: &str = "https://github.com/eggplantiny/conn/releases";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    phase: &'static str,
    current: String,
    version: Option<String>,
    notes: String,
    preview: bool,
    supported: bool,
    downloaded: usize,
    total: Option<u64>,
    error: Option<String>,
}

#[derive(Default)]
struct Payload {
    update: Option<Update>,
    bytes: Option<Vec<u8>>,
}

pub struct Updates {
    status: Mutex<Status>,
    // A single operation across every window; never hold the status lock over IO.
    operation: tokio::sync::Mutex<Payload>,
}

impl Updates {
    pub fn new() -> Self {
        Self {
            status: Mutex::new(Status {
                phase: "idle",
                current: env!("CARGO_PKG_VERSION").into(),
                version: None,
                notes: String::new(),
                preview: option_env!("CONN_RELEASE_CHANNEL") == Some("preview"),
                supported: option_env!("CONN_UPDATER_ENABLED") == Some("1")
                    && !cfg!(debug_assertions)
                    && (cfg!(target_os = "macos")
                        || cfg!(target_os = "windows")
                        || std::env::var_os("APPIMAGE").is_some()),
                downloaded: 0,
                total: None,
                error: None,
            }),
            operation: Default::default(),
        }
    }
    fn snapshot(&self) -> Value {
        serde_json::to_value(self.status.lock().clone()).unwrap()
    }
}

fn candidate(
    data: &Value,
    current: &Version,
    previews: bool,
) -> Result<Option<(String, bool)>, String> {
    let list = data.as_array().ok_or("Invalid release response")?;
    let mut candidates = list
        .iter()
        .filter_map(|r| {
            if r["draft"] != false || (!previews && r["prerelease"] != false) {
                return None;
            }
            let tag = r["tag_name"].as_str()?;
            let version = Version::parse(tag.strip_prefix('v')?).ok()?;
            // Preview tags also stay out of the stable channel if release metadata is wrong.
            if version <= *current || (!previews && !version.pre.is_empty()) {
                return None;
            }
            Some((version, tag.to_owned(), r["prerelease"] == true))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(candidates
        .into_iter()
        .next()
        .map(|(_, tag, preview)| (tag, preview)))
}

fn trusted_asset(url: &str, tag: &str) -> bool {
    [RELEASES, LEGACY_RELEASES].iter().any(|releases| {
        let prefix = format!("{releases}/download/{tag}/conn-{tag}-");
        url.strip_prefix(&prefix)
            .is_some_and(|s| !s.is_empty() && !s.contains(['/', '?', '#', '%']))
    })
}

#[tauri::command]
pub async fn app_update(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, Updates>,
    action: String,
    previews: Option<bool>,
    confirmed: Option<bool>,
) -> Result<Value, String> {
    if window.label() != "main" {
        return Err("Updates are available in the main window only".into());
    }
    if action == "status" {
        return Ok(state.snapshot());
    }
    if !["check", "download", "install"].contains(&action.as_str()) {
        return Err("Unknown update action".into());
    }
    if action == "install" && confirmed != Some(true) {
        return Err("Confirm closing shell sessions before installing".into());
    }
    let mut payload = state
        .operation
        .try_lock()
        .map_err(|_| "An update operation is already running")?;
    state.status.lock().error = None;
    let result: Result<(), String> = async {
        match action.as_str() {
            "check" => {
                state.status.lock().phase = "checking";
                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(20))
                    .user_agent("Conn updater")
                    .build()
                    .map_err(|e| e.to_string())?;
                let data: Value = client
                    .get("https://api.github.com/repos/eggp-dev/conn/releases?per_page=100")
                    .send()
                    .await
                    .map_err(|_| "Could not reach GitHub. Try again later.")?
                    .error_for_status()
                    .map_err(|_| "GitHub update lookup failed or is rate limited.")?
                    .json()
                    .await
                    .map_err(|_| "Invalid release response")?;
                let current = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
                let picked = candidate(&data, &current, previews.unwrap_or(false))?;
                *payload = Payload::default();
                {
                    let mut s = state.status.lock();
                    s.version = None;
                    s.notes.clear();
                    s.downloaded = 0;
                    s.total = None;
                }
                if let Some((tag, preview)) = picked {
                    let release = data
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|r| r["tag_name"] == tag)
                        .unwrap();
                    {
                        let mut s = state.status.lock();
                        s.version = Some(tag.trim_start_matches('v').into());
                        s.preview = preview;
                        s.notes = release["body"].as_str().unwrap_or_default().into();
                        s.phase = "available";
                    }
                    if state.status.lock().supported {
                        let endpoint = format!("{RELEASES}/download/{tag}/latest.json");
                        let handle = app.clone();
                        let update = app
                            .updater_builder()
                            .endpoints(vec![endpoint.parse().unwrap()])
                            .map_err(|e| e.to_string())?
                            .timeout(Duration::from_secs(180))
                            .on_before_exit(move || {
                                handle.state::<Arc<conn_frontend::Harness>>().shutdown();
                                handle.cleanup_before_exit();
                            })
                            .build()
                            .map_err(|e| e.to_string())?
                            .check()
                            .await
                            .map_err(|e| e.to_string())?
                            .ok_or("Release metadata does not offer this update")?;
                        if format!("v{}", update.version) != tag
                            || !trusted_asset(update.download_url.as_str(), &tag)
                        {
                            return Err(
                                "Update metadata does not match the selected release".into()
                            );
                        }
                        payload.update = Some(update);
                    }
                } else {
                    state.status.lock().phase = "current";
                }
            }
            "download" => {
                let update = payload
                    .update
                    .as_ref()
                    .ok_or("Check for a supported update first")?;
                {
                    let mut s = state.status.lock();
                    s.phase = "downloading";
                    s.downloaded = 0;
                    s.total = None;
                }
                let bytes = update
                    .download(
                        |chunk, total| {
                            let mut s = state.status.lock();
                            s.downloaded += chunk;
                            s.total = total;
                        },
                        || {},
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                // download() verifies the signature before returning any installable bytes.
                payload.bytes = Some(bytes);
                state.status.lock().phase = "ready";
            }
            "install" => {
                let update = payload.update.as_ref().ok_or("No update selected")?;
                let bytes = payload
                    .bytes
                    .as_ref()
                    .ok_or("Download and verify the update first")?;
                state.status.lock().phase = "installing";
                update.install(bytes).map_err(|e| e.to_string())?;
                app.state::<Arc<conn_frontend::Harness>>().shutdown();
                app.restart();
            }
            _ => unreachable!(),
        }
        Ok(())
    }
    .await;
    if let Err(error) = result {
        let mut s = state.status.lock();
        s.phase = "error";
        s.error = Some(error);
    }
    Ok(state.snapshot())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn channels_never_downgrade_or_leak_previews() {
        let data = json!([
            {"tag_name":"v0.8.0", "draft":false,"prerelease":true},
            {"tag_name":"v0.7.0", "draft":false,"prerelease":false},
            {"tag_name":"v9.0.0", "draft":true,"prerelease":false},
            {"tag_name":"v0.9.0-rc.1", "draft":false,"prerelease":false}
        ]);
        let current = Version::parse("0.6.0").unwrap();
        assert_eq!(
            candidate(&data, &current, false).unwrap().unwrap().0,
            "v0.7.0"
        );
        assert_eq!(
            candidate(&data, &current, true).unwrap().unwrap().0,
            "v0.9.0-rc.1"
        );
        assert_eq!(
            candidate(&data, &Version::parse("1.0.0").unwrap(), true).unwrap(),
            None
        );
    }
    #[test]
    fn payload_must_belong_to_the_selected_repository_and_tag() {
        // The organization's address, and the one update manifests keep using for builds up to 0.8.1.
        for releases in [RELEASES, LEGACY_RELEASES] {
            assert!(trusted_asset(
                &format!("{releases}/download/v0.6.0/conn-v0.6.0-desktop.AppImage"),
                "v0.6.0"
            ));
        }
        assert_eq!(RELEASES, "https://github.com/eggp-dev/conn/releases");
        assert_eq!(LEGACY_RELEASES, "https://github.com/eggplantiny/conn/releases");
        for url in [
            "https://evil.example/conn-v0.6.0.exe",
            "https://github.com/someone-else/conn/releases/download/v0.6.0/conn-v0.6.0-desktop.AppImage",
            "https://github.com/eggp-dev/other/releases/download/v0.6.0/conn-v0.6.0-desktop.AppImage",
            "https://github.com/eggplantiny/conn/releases/download/v0.5.1/conn-v0.5.1.exe",
            "https://github.com/eggplantiny/conn/releases/download/v0.6.0/conn-v0.6.0-../../bad",
        ] {
            assert!(!trusted_asset(url, "v0.6.0"));
        }
    }
}
