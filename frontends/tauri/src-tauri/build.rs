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
    tauri_build::build()
}
