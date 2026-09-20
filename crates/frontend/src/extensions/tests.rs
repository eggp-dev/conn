use super::*;
use manifest::Capability;

fn fixture() -> (tempfile::TempDir, Extensions) {
    let dir = tempfile::tempdir().unwrap();
    let host = Extensions::load(dir.path());
    (dir, host)
}
fn user_theme() -> Manifest {
    let mut manifest = Registry::builtin().get(DEFAULT_THEME).unwrap().clone();
    manifest.id = "user.theme".into();
    manifest.name = "User".into();
    let theme = manifest.theme.as_mut().unwrap();
    theme.id = "user.theme".into();
    theme.name = "User".into();
    manifest
}
#[test]
fn builtin_registry_ships_themes_only() {
    let (_dir, host) = fixture();
    let snapshot = host.snapshot();
    assert_eq!(snapshot["apiVersion"], API_VERSION);
    assert!(snapshot["extensions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["kind"] == "theme"));
    assert!(snapshot.get("keyStatus").is_none());
}
#[test]
fn unknown_executable_manifest_or_excess_capability_is_rejected() {
    let registry = Registry::builtin();
    let value = json!({"apiVersion":1,"id":"thirdparty.provider","name":"Provider","kind":"provider","capabilities":["model_request"]});
    let manifest: Manifest = serde_json::from_value(value).unwrap();
    assert!(manifest.validate(false).is_err());
    assert!(manifest.validate(true).is_err());
    let mut theme = registry.get(DEFAULT_THEME).unwrap().clone();
    theme.capabilities.insert(Capability::VisibleFrame);
    assert!(theme.validate(false).is_err());
    theme.capabilities.remove(&Capability::VisibleFrame);
    theme.api_version = 999;
    assert!(theme.validate(false).is_err());
    let value = json!({"apiVersion":1,"id":"evil.theme","name":"Evil","kind":"theme","capabilities":["theme"],"entrypoint":"/bin/sh"});
    assert!(serde_json::from_value::<Manifest>(value).is_err());
}
#[test]
fn declarative_theme_persists_without_executable_or_ui_injection() {
    let (dir, host) = fixture();
    let mut manifest = user_theme();
    host.install_theme(manifest.clone()).unwrap();
    host.configure("user.theme", json!({"enabled":true}))
        .unwrap();
    let restored = Extensions::load(dir.path());
    assert_eq!(restored.settings().theme, "user.theme");
    assert!(host.install_theme(manifest.clone()).is_err());
    manifest.theme.as_mut().unwrap().foreground = "url(https://outside.invalid)".into();
    assert!(manifest.validate(false).is_err());
}
#[test]
fn unknown_configuration_fields_and_unknown_extensions_are_rejected() {
    let (_dir, host) = fixture();
    assert!(host
        .configure(DEFAULT_THEME, json!({"enabled":true,"apiKey":"secret"}))
        .is_err());
    assert!(host
        .configure("conn.completion", json!({"enabled":true}))
        .is_err());
}
#[test]
fn settings_saved_with_the_removed_provider_keep_the_selected_theme() {
    let dir = tempfile::tempdir().unwrap();
    let stored = json!({"settings":{"theme":"user.theme","model":"old-model","providerEnabled":true,"completionEnabled":true},"themes":[user_theme()]});
    std::fs::write(dir.path().join("extensions.json"), stored.to_string()).unwrap();
    let host = Extensions::load(dir.path());
    assert_eq!(host.settings().theme, "user.theme");
    host.configure("user.theme", json!({"enabled":true}))
        .unwrap();
    let disk = std::fs::read_to_string(dir.path().join("extensions.json")).unwrap();
    assert!(!disk.contains("model") && !disk.contains("providerEnabled"));
}
