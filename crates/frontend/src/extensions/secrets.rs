//! OS credential storage. An unavailable/locked store is an error, never a file fallback.
use sha2::{Digest, Sha256};
use std::path::Path;
use zeroize::Zeroizing;

pub type Secret = Zeroizing<String>;
pub trait SecretStore: Send + Sync {
    fn get(&self) -> Result<Option<Secret>, String>;
    fn set(&self, key: &str) -> Result<(), String>;
    fn delete(&self) -> Result<(), String>;
}

pub struct OsSecretStore {
    account: String,
}
impl OsSecretStore {
    pub fn new(config_dir: &Path) -> Self {
        // Distinct dev/test/user configurations must not silently reuse one credential.
        let path = config_dir
            .canonicalize()
            .unwrap_or_else(|_| config_dir.to_path_buf());
        Self {
            account: format!(
                "openai-{:x}",
                Sha256::digest(path.to_string_lossy().as_bytes())
            ),
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn entry(&self) -> Result<keyring::Entry, String> {
        keyring::Entry::new("dev.eggp.conn.model-provider", &self.account)
            .map_err(|_| "OS credential store is unavailable".into())
    }
}
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
impl SecretStore for OsSecretStore {
    fn get(&self) -> Result<Option<Secret>, String> {
        match self.entry()?.get_password() {
            Ok(value) => Ok(Some(Zeroizing::new(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("OS credential store is unavailable or locked".into()),
        }
    }
    fn set(&self, key: &str) -> Result<(), String> {
        self.entry()?
            .set_password(key)
            .map_err(|_| "Could not save key in OS credential store".into())
    }
    fn delete(&self) -> Result<(), String> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("Could not delete key from OS credential store".into()),
        }
    }
}
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
impl SecretStore for OsSecretStore {
    fn get(&self) -> Result<Option<Secret>, String> {
        Err("OS credential store is unsupported".into())
    }
    fn set(&self, _: &str) -> Result<(), String> {
        Err("OS credential store is unsupported".into())
    }
    fn delete(&self) -> Result<(), String> {
        Err("OS credential store is unsupported".into())
    }
}
