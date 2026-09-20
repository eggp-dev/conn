//! Local MCP + skill registration, shared by the native app and browser harness.
//! Clients own formats and paths; the engine owns preservation and lifecycle.
mod adapters;
mod config;
mod storage;
#[cfg(test)]
mod tests;

use adapters::{adapter, Adapter, Locations, ADAPTERS};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use storage::{read, Change};

const SKILL: &str = include_str!("../../../../plugin/skills/conn/SKILL.md");
struct SetupLock(fs::File);
impl Drop for SetupLock {
    fn drop(&mut self) {
        // A concurrently spawned PTY can briefly inherit the descriptor before
        // exec. Explicit unlock releases the shared lock even while that copy lives.
        let _ = self.0.unlock();
    }
}
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}
#[derive(Clone, Serialize, Deserialize)]
struct Registration {
    config: PathBuf,
    entry: Value,
    skill: PathBuf,
    skill_hash: String,
    skill_owned: bool,
}
#[derive(Default, Serialize, Deserialize)]
struct Registry {
    version: u32,
    clients: BTreeMap<String, Registration>,
}
fn registry(bytes: Option<&[u8]>) -> Result<Registry, String> {
    match bytes {
        Some(bytes) => {
            let r: Registry = serde_json::from_slice(bytes).map_err(|_| "invalid_registry")?;
            if r.version != 1 {
                return Err("invalid_registry".into());
            }
            Ok(r)
        }
        None => Ok(Registry {
            version: 1,
            ..Default::default()
        }),
    }
}
fn text(bytes: Option<&[u8]>, a: Adapter) -> Result<&str, String> {
    match bytes {
        Some(b) => std::str::from_utf8(b).map_err(|_| "invalid_config".into()),
        None => Ok(config::empty(a.format)),
    }
}
fn check_record(record: &Registration, paths: &(PathBuf, PathBuf)) -> Result<(), String> {
    // Do not use arbitrary paths from a manifest to write or delete files.
    if (&record.config, &record.skill) != (&paths.0, &paths.1) {
        return Err("location_changed".into());
    }
    Ok(())
}

pub(crate) fn catalog(
    config_dir: &Path,
    socket: &Path,
    setup_home: Option<&Path>,
    connected: &[String],
) -> Result<Value, String> {
    let locations = locations(setup_home)?;
    let receipt = registry(read(&config_dir.join("integrations.json"))?.as_deref())?;
    let cli = super::agent_setup::cli_status();
    let command = super::agent_setup::configuration_command_path(config_dir);
    let items: Vec<Value> = ADAPTERS
        .iter()
        .map(|&a| {
            let (config_path, skill_path) = locations.paths(a);
            let mut state = "not_configured";
            let mut error = None;
            let mut managed = false;
            let inspect = (|| -> Result<(), String> {
                let bytes = read(&config_path)?;
                let entry = config::read(text(bytes.as_deref(), a)?, a.format)?;
                let skill = read(&skill_path)?;
                if let Some(r) = receipt.clients.get(a.id) {
                    check_record(r, &(config_path.clone(), skill_path.clone()))?;
                    managed = true;
                    if entry.as_ref().is_some_and(|e| e != &r.entry) {
                        return Err("entry_conflict".into());
                    }
                    if skill.as_ref().is_some_and(|s| hash(s) != r.skill_hash) {
                        return Err("skill_conflict".into());
                    }
                    state = if entry.is_none() || skill.is_none() {
                        "needs_update"
                    } else {
                        "configured"
                    };
                    if hash(SKILL.as_bytes()) != r.skill_hash {
                        state = "needs_update";
                    }
                    if let Some(command) = &command {
                        if a.entry(&command.to_string_lossy(), &socket.to_string_lossy()) != r.entry
                        {
                            state = "needs_update";
                        }
                    }
                } else if entry.is_some() {
                    state = "external";
                } else if let Some(skill) = skill {
                    let current_hash = hash(&skill);
                    let known = current_hash == hash(SKILL.as_bytes())
                        || receipt
                            .clients
                            .values()
                            .any(|r| r.skill == skill_path && r.skill_hash == current_hash);
                    if !known {
                        return Err("skill_conflict".into());
                    }
                }
                Ok(())
            })();
            if let Err(e) = inspect {
                state = "conflict";
                error = Some(e);
            }
            json!({ "id": a.id, "name": a.name, "state": state, "managed": managed,
            "connected": connected.iter().any(|id| id == a.id), "error": error,
            "configPath": config_path, "skillPath": skill_path })
        })
        .collect();
    Ok(
        json!({ "clients": items, "canConfigure": cli["canConfigure"], "backupPath": config_dir.join("integration-backups") }),
    )
}

pub(crate) fn configure(
    config_dir: &Path,
    socket: &Path,
    setup_home: Option<&Path>,
    id: &str,
) -> Result<(), String> {
    let a = adapter(id)?;
    let generated = super::agent_setup::configuration(config_dir, socket)?;
    let entry = a.entry(
        generated["command"].as_str().ok_or("cli_missing")?,
        generated["endpoint"].as_str().ok_or("cli_missing")?,
    );
    update(config_dir, &locations(setup_home)?, a, Some(entry))
}
pub(crate) fn remove(config_dir: &Path, setup_home: Option<&Path>, id: &str) -> Result<(), String> {
    update(config_dir, &locations(setup_home)?, adapter(id)?, None)
}
pub(crate) fn manual(config_dir: &Path, socket: &Path, id: &str) -> Result<Value, String> {
    let a = adapter(id)?;
    let generated = super::agent_setup::configuration(config_dir, socket)?;
    let entry = a.entry(
        generated["command"].as_str().ok_or("cli_missing")?,
        generated["endpoint"].as_str().ok_or("cli_missing")?,
    );
    Ok(json!({"text": config::edit(config::empty(a.format), a.format, Some(&entry))?}))
}

fn locations(setup_home: Option<&Path>) -> Result<Locations, String> {
    match setup_home {
        Some(home) => Ok(Locations::isolated(home.to_path_buf())),
        None => Locations::system(),
    }
}

fn update(
    config_dir: &Path,
    locations: &Locations,
    a: Adapter,
    entry: Option<Value>,
) -> Result<(), String> {
    // Serializes calls across app instances; read-only catalog never takes a lock or writes.
    storage::safe_path(config_dir)?;
    fs::create_dir_all(config_dir).map_err(|_| "file_io")?;
    let lock_path = config_dir.join("integration-setup.lock");
    storage::safe_path(&lock_path)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|_| "file_io")?;
    lock.try_lock().map_err(|_| "setup_busy")?;
    let _lock = SetupLock(lock);
    let manifest = config_dir.join("integrations.json");
    let before_registry = read(&manifest)?;
    let mut registry = registry(before_registry.as_deref())?;
    let paths = locations.paths(a);
    let before_config = read(&paths.0)?;
    let before_skill = read(&paths.1)?;
    let source = text(before_config.as_deref(), a)?;
    let existing = config::read(source, a.format)?;
    let record = registry.clients.get(a.id).cloned();
    if let Some(r) = &record {
        check_record(r, &paths)?;
        if existing.as_ref().is_some_and(|v| v != &r.entry) {
            return Err("entry_conflict".into());
        }
        if before_skill
            .as_ref()
            .is_some_and(|v| hash(v) != r.skill_hash)
        {
            return Err("skill_conflict".into());
        }
    } else if existing.is_some() {
        return Err("entry_conflict".into());
    }

    let after_skill;
    if let Some(entry) = &entry {
        // A shared Copilot skill is removed only after the last managed client unlinks.
        let peer = registry.clients.values().find(|r| r.skill == paths.1);
        let expected = record.as_ref().or(peer).map(|r| r.skill_hash.as_str());
        if let Some(bytes) = &before_skill {
            let actual = hash(bytes);
            if actual != hash(SKILL.as_bytes()) && expected != Some(actual.as_str()) {
                return Err("skill_conflict".into());
            }
        }
        let owned = record
            .as_ref()
            .or(peer)
            .map(|r| r.skill_owned)
            .unwrap_or(before_skill.is_none());
        let skill_hash = hash(SKILL.as_bytes());
        // Updating a shared skill updates all its receipts, without touching their MCP configuration.
        for peer in registry.clients.values_mut().filter(|r| r.skill == paths.1) {
            peer.skill_hash = skill_hash.clone();
        }
        registry.clients.insert(
            a.id.into(),
            Registration {
                config: paths.0.clone(),
                entry: entry.clone(),
                skill: paths.1.clone(),
                skill_hash,
                skill_owned: owned,
            },
        );
        after_skill = Some(SKILL.as_bytes().to_vec());
    } else {
        let Some(r) = record else {
            return Err("not_managed".into());
        };
        registry.clients.remove(a.id);
        let shared = registry.clients.values().any(|peer| peer.skill == paths.1);
        after_skill = if r.skill_owned && !shared {
            None
        } else {
            before_skill.clone()
        };
    }
    let after_config = config::edit(source, a.format, entry.as_ref())?.into_bytes();
    let changes = vec![
        Change {
            path: paths.0,
            before: before_config.clone(),
            after: if before_config.is_none() && entry.is_none() {
                None
            } else {
                Some(after_config)
            },
        },
        Change {
            path: paths.1,
            before: before_skill,
            after: after_skill,
        },
        Change {
            path: manifest,
            before: before_registry,
            after: Some(serde_json::to_vec_pretty(&registry).map_err(|_| "invalid_registry")?),
        },
    ];
    storage::apply(&changes, &config_dir.join("integration-backups"))
}
