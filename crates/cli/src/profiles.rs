use clap::Subcommand;
use conn_core::profiles::Profiles;
use std::path::{Path, PathBuf};
#[derive(Subcommand)]
pub enum ProfileCommand {
    List,
    /// Discover installed local shells and WSL distributions without changing saved profiles
    Detect,
    Export,
    /// Validate and atomically import a profiles JSON file
    Import {
        file: PathBuf,
    },
    SetDefault {
        id: String,
    },
    Enable {
        id: String,
    },
    Disable {
        id: String,
    },
    Test {
        id: String,
    },
}
pub fn run(command: ProfileCommand, path: &Path) -> Result<i32, String> {
    let enabled = matches!(&command, ProfileCommand::Enable { .. });
    let mut config = Profiles::load(path)?;
    let value = match command {
        ProfileCommand::List => serde_json::to_value(config.catalog()),
        ProfileCommand::Detect => serde_json::to_value(conn_core::profiles::discover()),
        ProfileCommand::Export => serde_json::to_value(config),
        ProfileCommand::Test { id } => {
            let profile = config.select(Some(&id))?;
            serde_json::to_value(conn_core::profiles::test(&profile)?)
        }
        ProfileCommand::Import { file } => {
            let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
            let mut imported: Profiles =
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            imported.revision = config.revision;
            serde_json::to_value(imported.save(path)?.catalog())
        }
        ProfileCommand::SetDefault { id } => {
            config.select(Some(&id))?;
            config.default_profile = id;
            serde_json::to_value(config.save(path)?.catalog())
        }
        ProfileCommand::Enable { id } | ProfileCommand::Disable { id } => {
            let p = config
                .profiles
                .iter_mut()
                .find(|p| p.id == id)
                .ok_or("Unknown profile")?;
            p.enabled = enabled;
            serde_json::to_value(config.save(path)?.catalog())
        }
    }
    .map_err(|e| e.to_string())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
    );
    Ok(0)
}
