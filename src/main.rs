use std::{
    collections::HashMap,
    fs::{DirEntry, FileType, ReadDir},
    path::PathBuf,
    str::FromStr,
};

use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use straw::result::{Result, StrawError};

const DOTFILES_DIR: &str = ".dotfiles/";
const CONFIG_FILE: &str = "config.json";

#[derive(Parser, Debug, Clone, Copy)]
#[command(version, about, long_about = None)]
struct Args {
    /// Replace the existing symlinks with symlinks to .dotfiles
    #[clap(long, short, action)]
    preview: bool,

    /// Replace the existing files or symlinks with symlinks to .dotfiles
    #[clap(long, short, action)]
    force: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Override {
    from: String,
    to: String,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Config {
    #[serde_as(as = "Option<Vec<(_, _)>>")]
    overrides: Option<HashMap<String, Override>>,

    #[serde_as(as = "Option<Vec<(_, _)>>")]
    overrides_windows: Option<HashMap<String, Override>>,

    #[serde_as(as = "Option<Vec<(_, _)>>")]
    overrides_unix: Option<HashMap<String, Override>>,
}

#[derive(Debug)]
struct Info<'a> {
    home_dir: &'a str,
    dotfiles_path: PathBuf,
    straw_config: Option<Config>,
    args: &'a Args,
}

fn fold_and_apply(name: &str, info: &Info, from_dir: ReadDir, to: PathBuf) -> Result<()> {
    for item in from_dir {
        let item = item?;
        let home_dir = std::fs::read_dir(&to)?;
        let mut found_existing = false;
        for existing in home_dir {
            let existing = existing?;
            if item.file_name() == existing.file_name() && !existing.file_type()?.is_symlink() {
                if existing.file_type()?.is_dir() {
                    let file_name = item.file_name();
                    let item_name = file_name.to_str().ok_or(StrawError::String(
                        "Can't convert '{item_name:?}' to str.".into(),
                    ))?;
                    let to: PathBuf = PathBuf::from(&to).join(item_name);
                    tracing::warn!("{name}: {existing:?} exists");
                    let dir = std::fs::read_dir(item.path())?;
                    fold_and_apply(name, info, dir, to)?;
                }
                found_existing = true;
            }
        }

        if !found_existing {
            let from = item.path();
            let to = to.join(item.file_name());
            tracing::info!("{name}: Creating link {:?} -> {:?}", item, to);

            remove_symlink_if_exists(info, &to)?;
            create_symlink(info, from, to)?;
        }
    }

    Ok(())
}

fn create_symlink(info: &Info, from: PathBuf, to: PathBuf) -> Result<()> {
    tracing::info!("\t    {from:?} -> {to:?}");
    if from.is_dir() {
        if !info.args.preview {
            std::os::windows::fs::symlink_dir(from, to)?;
        }
        tracing::info!("\t\tCreated a directory link");
    } else if from.is_file() {
        if !info.args.preview {
            std::os::windows::fs::symlink_file(from, to)?;
        }
        tracing::info!("\t\tCreated a file link");
    } else if from.is_symlink() {
        return Err(StrawError::String(
            "Linking symlink to a symlink is not supported at the moment.".into(),
        ));
    } else {
        return Err(StrawError::String("Invalid path {from_path:?}".into()));
    }

    Ok(())
}

fn remove_symlink_if_exists(info: &Info, link_path: &PathBuf) -> Result<()> {
    if link_path.exists() && link_path.is_symlink() && info.args.force {
        tracing::info!("\t\tRemoving the existing link: {link_path:?}");
        if !info.args.preview {
            if link_path.is_dir() {
                std::fs::remove_dir_all(link_path)?;
            } else if link_path.is_file() {
                std::fs::remove_file(link_path)?;
            }
        }
    }

    Ok(())
}

fn apply_config(name: &str, info: &Info) -> Result<()> {
    let config_path = &info.dotfiles_path;
    let config_path = config_path.join(name);

    let config_dir = std::fs::read_dir(&config_path)?;

    let mut has_override = false;
    if let Some(straw_config) = &info.straw_config {
        let mut overrides: HashMap<String, Override> = HashMap::new();
        if let Some(general_overrides) = &straw_config.overrides {
            overrides.extend(general_overrides.clone());
        }

        #[cfg(windows)]
        if let Some(windows_overrides) = &straw_config.overrides_windows {
            overrides.extend(windows_overrides.clone());
        }

        #[cfg(unix)]
        if let Some(unix_overrides) = &straw_config.overrides_unix {
            overrides.extend(unix_overrides.clone());
        }

        if overrides.contains_key(name) {
            has_override = true;
            let path_override = &overrides[name];

            let from = path_override.from.clone();
            let to = path_override.to.clone();
            tracing::info!("\t    {from} -> {to}");

            let to = PathBuf::from_str(to.as_str())
                .map_err(|e| StrawError::String(format!("Can't resolve path: {to:?}. {e:?}")))?;

            remove_symlink_if_exists(info, &to)?;
            if to.exists() && !info.args.force {
                tracing::info!(
                    "\t\t{to:?} already exists, ignoring. Run with --force to overwrite."
                );
            }

            // could be false if the file wasn't successfully removed
            if !to.exists() || info.args.preview {
                let from_path: PathBuf = [info.home_dir, DOTFILES_DIR, name, from.as_str()]
                    .iter()
                    .collect();
                create_symlink(info, from_path, to)?;
            }
        }
    }

    if !has_override {
        fold_and_apply(name, info, config_dir, PathBuf::from(info.home_dir))?;
    }

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

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
    let home_dir = home.as_os_str().to_str().ok_or(StrawError::String(
        "Couldn't convert {home:?} to &str.".into(),
    ))?;

    tracing::info!("Found home directory: {home:?}");
    let dotfiles_path: PathBuf = [home_dir, DOTFILES_DIR].iter().collect();
    let config_file_path: PathBuf = [home_dir, DOTFILES_DIR, CONFIG_FILE].iter().collect();

    let mut straw_config = None;
    if let Ok(config_file) = std::fs::read_to_string(&config_file_path) {
        tracing::info!("Found config: {config_file_path:?}");
        let straw_config_parsed: Config = serde_json::from_str(config_file.as_str())?;
        straw_config = Some(straw_config_parsed);
    }

    let dotfiles = std::fs::read_dir(&dotfiles_path)?;
    let info = Info {
        home_dir,
        dotfiles_path: dotfiles_path.clone(),
        straw_config,
        args: &args,
    };

    let mut configs = Vec::new();
    for path in dotfiles {
        let path = path?;
        if let Some(item_name) = path.file_name().to_str() {
            if path.file_type()?.is_dir() && !item_name.starts_with(".") {
                configs.push(item_name.to_owned());
            }
        }
    }
    tracing::info!("Configs found: {configs:?}");

    for config in configs {
        // TODO: if the dir exists then go deeper or ignore if no dir inside
        // let from: PathBuf = [DOTFILES_DIR, config]
        // tracing::info!("\t    {from} -> {to}");
        apply_config(&config, &info)?;
    }

    Ok(())
}
