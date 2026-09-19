fn allowed(url: &str) -> bool {
    // The second address is where the repository lived before it moved; older release notes link there.
    let Some(path) = ["https://github.com/eggp-dev/conn/releases/", "https://github.com/eggplantiny/conn/releases/"]
        .iter().find_map(|base| url.strip_prefix(base)) else { return false };
    if path.is_empty() { return true; }
    let parts: Vec<_> = path.split('/').collect();
    matches!(parts.as_slice(), ["tag", _] | ["download", _, _])
        && parts.iter().skip(1).all(|s| !s.is_empty() && *s != "." && *s != ".."
            && s.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-+".contains(&c)))
}

pub fn open(url: &str) -> Result<(), String> {
    if !allowed(url) { return Err("Invalid Conn release URL".into()); }
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "linux")]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = std::process::Command::new("rundll32.exe");
        c.arg("url.dll,FileProtocolHandler");
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return Err("Unsupported platform".into());
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        let status = command.arg(url).status().map_err(|e| e.to_string())?;
        if status.success() { Ok(()) } else { Err("Could not open the browser".into()) }
    }
}

#[cfg(test)]
mod tests {
    use super::allowed;
    #[test]
    fn only_conn_release_links() {
        for url in ["https://github.com/eggplantiny/conn/releases/", "https://github.com/eggplantiny/conn/releases/tag/v0.4.0", "https://github.com/eggplantiny/conn/releases/download/v0.4.0/conn.AppImage"] { assert!(allowed(url)); }
        for url in ["https://github.com/eggp-dev/conn/releases/", "https://github.com/eggp-dev/conn/releases/tag/v0.8.2", "https://github.com/eggp-dev/conn/releases/download/v0.8.2/conn.AppImage"] { assert!(allowed(url)); }
        for url in ["https://github.com/someone-else/conn/releases/tag/v0.8.2", "https://github.com/eggp-dev/other/releases/tag/v0.8.2", "https://github.com/eggp-dev/conn/releases/tag/.."] { assert!(!allowed(url)); }
        for url in ["file:///tmp/x", "https://evil.test/", "https://github.com/eggplantiny/conn/releases/tag/..", "https://github.com/eggplantiny/conn/releases/tag/v1?x=1", "https://github.com/eggplantiny/conn/releases/download/v1/a%20b", "https://github.com/eggplantiny/conn/releases/tag/v1\n"] { assert!(!allowed(url)); }
    }
}
