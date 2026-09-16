fn main() {
    println!("cargo:rerun-if-env-changed=CONN_UPDATER_ENABLED");
    println!("cargo:rerun-if-env-changed=CONN_RELEASE_CHANNEL");
    println!("cargo:rerun-if-changed=src/macos/scripting.m");
    println!("cargo:rerun-if-changed=Conn.sdef");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new().file("src/macos/scripting.m").flag("-fobjc-arc").compile("conn_scripting");
        println!("cargo:rustc-link-lib=framework=Cocoa");
        println!("cargo:rustc-link-lib=framework=Carbon");
    }
    let mut attributes = tauri_build::Attributes::new();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // tauri-winres embeds the manifest in binaries but not lib test runners.
        // Both need Common Controls v6: https://github.com/tauri-apps/tauri/issues/13419
        attributes = attributes.windows_attributes(
            tauri_build::WindowsAttributes::new_without_app_manifest(),
        );
        let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
    tauri_build::try_build(attributes).expect("Tauri build configuration")
}
