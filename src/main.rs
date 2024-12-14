use std::{collections::HashMap, path::PathBuf, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use straw::result::{Result, StrawError};

const DOTFILES_DIR: &str = ".dotfiles/";
const CONFIG_FILE: &str = "config.json";

#[serde_as]
#[derive(Serialize, Deserialize)]
struct Config {
    #[serde_as(as = "Vec<(_, _)>")]
    overrides: HashMap<String, String>,
}

fn main() -> Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_max_level(tracing::Level::INFO)
        .without_time()
        .with_level(true)
        .with_file(false)
        .with_line_number(false)
        .with_thread_names(false)
        .with_thread_ids(false)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let home = home::home_dir().ok_or(StrawError::String(
        "Couldn't determine the home directory path.".into(),
    ))?;
    let home_path = home.as_os_str().to_str().ok_or(StrawError::String(
        "Couldn't convert {home:?} to &str.".into(),
    ))?;

    tracing::info!("Found home directory: {home:?}");
    let dotfiles_path: PathBuf = [home_path, DOTFILES_DIR].iter().collect();
    let config_file_path: PathBuf = [home_path, DOTFILES_DIR, CONFIG_FILE].iter().collect();

    tracing::info!("Reading config: {config_file_path:?}");
    if let Ok(config_file) = std::fs::read_to_string(config_file_path) {
        let config: Config = serde_json::from_str(config_file.as_str())?;

        tracing::info!("Config loaded");
        tracing::info!("\tOverrides:");
        for (from, to) in config.overrides {
            // handle overrides
            tracing::info!("\t\t{from} -> {to}");

            let to = PathBuf::from_str(to.as_str())
                .map_err(|e| StrawError::String(format!("Can't resolve path: {to:?}. {e:?}")))?;
            if to.exists() {
                // TODO: AND --force ?
                tracing::info!("\t\tRemoving the existing link.");
                if to.is_dir() {
                    std::fs::remove_dir_all(&to)?;
                } else if to.is_file() {
                    std::fs::remove_file(&to)?;
                }
            }

            let from_path: PathBuf = [home_path, DOTFILES_DIR, from.as_str()].iter().collect();
            if from_path.is_dir() {
                std::os::windows::fs::symlink_dir(from_path, to)?;
                tracing::info!("\t\tCreated a directory link");
            } else if from_path.is_file() {
                std::os::windows::fs::symlink_file(from_path, to)?;
                tracing::info!("\t\tCreated a file link");
            } else if from_path.is_symlink() {
                return Err(StrawError::String(
                    "Linking symlink to a symlink is not supported at the moment.".into(),
                ));
            } else {
                return Err(StrawError::String("Invalid path {from_path:?}".into()));
            }
        }
    }
    // std::os::windows::fs::symlink_dir("", "")?;
    // std::os::windows::fs::symlink_file("", "")?;

    Ok(())
}
