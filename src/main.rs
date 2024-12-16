use std::{
    collections::HashMap,
    fs::ReadDir,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::Parser;
use regex::{Captures, Regex};
use straw::{
    args::{Args, Commands},
    config::{Config, Override},
    result::{Result, StrawError},
};

const DOTFILES_DIR: &str = ".dotfiles/";
const CONFIG_FILE: &str = "config.json";

// TODO: adopt

#[derive(Debug)]
struct StateInfo<'a> {
    home_dir: &'a str,
    dotfiles_path: PathBuf,
    args: &'a Args,
    env_var_regex: Regex,
    overrides: HashMap<String, Override>,
    ignores: Vec<String>,
}

fn fold_and_apply(name: &str, info: &StateInfo, from_dir: ReadDir, to: PathBuf) -> Result<()> {
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
                    let display_path = existing.path().display().to_string().replace("\\", "/");
                    tracing::info!("{name}: {display_path} exists");
                    let dir = std::fs::read_dir(item.path())?;
                    fold_and_apply(name, info, dir, to)?;
                }
                found_existing = true;
            }
        }

        if !found_existing {
            let from = item.path();
            let to = to.join(item.file_name());

            tracing::info!("{name}: Creating symlink");

            remove_symlink_if_exists(info, &to)?;
            create_symlink(info, from, to)?;
        }
    }

    Ok(())
}

fn create_symlink(info: &StateInfo, from: PathBuf, to: PathBuf) -> Result<()> {
    let display_from = from.display().to_string().replace("\\", "/");
    let display_to = to.display().to_string().replace("\\", "/");
    if from.is_dir() {
        if !info.args.preview {
            #[cfg(windows)]
            std::os::windows::fs::symlink_dir(from, to)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(from, to)?;
        }
        tracing::info!("\t\tCreated a directory link:\t{display_from} -> {display_to}");
    } else if from.is_file() {
        if !info.args.preview {
            #[cfg(windows)]
            std::os::windows::fs::symlink_file(from, to)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(from, to)?;
        }
        tracing::info!("\t\tCreated a file link:\t\t{display_from} -> {display_to}");
    } else if from.is_symlink() {
        return Err(StrawError::String(
            "Linking symlink to a symlink is not supported at the moment.".into(),
        ));
    } else {
        return Err(StrawError::String("Invalid path {from_path:?}".into()));
    }

    Ok(())
}

fn remove_symlink_if_exists(info: &StateInfo, link_path: &PathBuf) -> Result<()> {
    if link_path.exists() && link_path.is_symlink() {
        let display_path = link_path.display().to_string().replace("\\", "/");
        tracing::info!("\t\tRemoving the existing link:\t{display_path}");
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

fn create_dir(dir: &Path) -> Result<()> {
    if let Some(parent) = dir.parent() {
        if !parent.exists() {
            return create_dir(parent);
        }
    }

    std::fs::create_dir(dir).map_err(StrawError::Io)
}

fn apply_config(name: &str, info: &StateInfo) -> Result<()> {
    let config_path = &info.dotfiles_path;
    let config_path = config_path.join(name);

    let config_dir = std::fs::read_dir(&config_path)?;

    tracing::info!("config: {name}");
    if info.ignores.contains(&name.into()) {
        tracing::info!("Ignoring {name}");
        return Ok(());
    }

    if info.overrides.contains_key(name) {
        let path_override = &info.overrides[name];

        let from = path_override.from.clone();
        let to = path_override.to.clone();

        // NOTE: replace the environment variables in paths `%env_var%` on windows or `$env_var`
        // on unix systems
        let to = info
            .env_var_regex
            .replace_all(&to, |captures: &Captures| {
                if let Ok(value) = std::env::var(&captures[1]) {
                    value
                } else {
                    tracing::error!("Couldn't resolve '{}' env var.", &captures[0]);
                    String::from(&captures[0])
                }
            })
            .into_owned();
        let display_from = from.replace("\\", "/");
        let display_to = path_override.to.replace("\\", "/");
        tracing::info!("\t\tFound override:\t\t\t{display_from} -> {display_to}");

        let to = PathBuf::from_str(to.as_str())
            .map_err(|e| StrawError::String(format!("Can't resolve path: {to:?}. {e:?}")))?;

        remove_symlink_if_exists(info, &to)?;

        if to.exists() && !to.is_symlink() {
            let to = to.display().to_string().replace("\\", "/");
            tracing::warn!("{to} already exists, ignoring.");
        }

        // could be false if the file wasn't successfully removed
        if !to.exists() || info.args.preview {
            // check if the parent path exists
            if let Some(parent) = to.parent() {
                if !parent.exists() {
                    create_dir(parent)?;
                }
            } // else - `to` is root, ignore

            let from_path: PathBuf = [info.home_dir, DOTFILES_DIR, name, from.as_str()]
                .iter()
                .collect();
            create_symlink(info, from_path, to)?;
        }
    } else {
        fold_and_apply(name, info, config_dir, PathBuf::from(info.home_dir))?;
    }

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    #[cfg(windows)]
    let env_var_regex = Regex::new(r"%(\w+)%")?;
    #[cfg(unix)]
    let env_var_regex = Regex::new(r"\$(\w+)")?;

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

    if args.preview {
        tracing::warn!("Running in `preview` mode. No changes will be applied.");
    }

    let home = home::home_dir().ok_or(StrawError::String(
        "Couldn't determine the home directory path.".into(),
    ))?;
    let home_dir = home.as_os_str().to_str().ok_or(StrawError::String(
        "Couldn't convert {home:?} to &str.".into(),
    ))?;

    let display_home = home.display().to_string().replace("\\", "/");
    tracing::info!("Found home directory: {display_home}");
    let dotfiles_path: PathBuf = [home_dir, DOTFILES_DIR].iter().collect();
    let config_file_path: PathBuf = [home_dir, DOTFILES_DIR, CONFIG_FILE].iter().collect();

    let mut repo_config = straw::git::RepositoryConfig {
        remote: String::from("origin"),
        branch: String::from("main"),
    };
    if let Ok(config_file) = std::fs::read_to_string(&config_file_path) {
        let config: Config = serde_json::from_str(config_file.as_str())?;
        if let Some(git) = config.git {
            repo_config = git;
        }
    }

    if let Some(Commands::Clone { url }) = &args.command {
        match straw::git::clone(&args, url, &dotfiles_path) {
            Ok(_) => {
                tracing::info!("Clone: successfully cloned {url} repository into ~/.dotfiles");
            }
            Err(e) => {
                return Err(e);
            }
        }
    } else if args.sync {
        // commit changes
        let mut changes_committed = false;
        match straw::git::commit_changes(&args, &dotfiles_path, &repo_config) {
            Ok(any_changes_committed) => {
                if any_changes_committed {
                    changes_committed = true;
                    tracing::info!("Sync: Done committing local changes to .dotfiles")
                } else {
                    tracing::info!("Sync: No local changes to commit");
                }
            }
            Err(e) => tracing::error!(
                "Sync: Failed to commit the local changes to .dotfiles with error: {e:?}"
            ),
        }
        // update git repo
        match straw::git::update_dotfiles(&args, &dotfiles_path, &repo_config) {
            Ok(_) => tracing::info!("Sync: .dotfiles updated"),
            Err(e) => tracing::error!("Sync: Failed to update the .dotfiles with error: {e:?}"),
        }
        if changes_committed {
            match straw::git::push_changes(&args, &dotfiles_path, &repo_config) {
                Ok(_) => tracing::info!("Sync: local changes pushed to {}", repo_config.remote),
                Err(e) => {
                    tracing::error!("Sync: Failed to push the local changes with error: {e:?}")
                }
            }
        }
    }

    let mut overrides = HashMap::new();
    let mut ignores: Vec<String> = Vec::new();
    if let Ok(config_file) = std::fs::read_to_string(&config_file_path) {
        let display_config_path = config_file_path.display().to_string().replace("\\", "/");
        tracing::info!("Found straw config file: {display_config_path}");
        let straw_config_parsed: Config = serde_json::from_str(config_file.as_str())?;

        if let Some(general_overrides) = &straw_config_parsed.overrides {
            overrides.extend(general_overrides.clone());
        }

        #[cfg(windows)]
        if let Some(windows_overrides) = &straw_config_parsed.overrides_windows {
            overrides.extend(windows_overrides.clone());
        }

        #[cfg(unix)]
        if let Some(unix_overrides) = &straw_config_parsed.overrides_unix {
            overrides.extend(unix_overrides.clone());
        }

        if let Some(general_ignores) = &straw_config_parsed.ignores {
            ignores.extend(general_ignores.clone());
        }

        #[cfg(windows)]
        if let Some(windows_ignores) = &straw_config_parsed.ignores_windows {
            ignores.extend(windows_ignores.clone());
        }

        #[cfg(unix)]
        if let Some(unix_ignores) = &straw_config_parsed.ignores_unix {
            ignores.extend(unix_ignores.clone());
        }
    }

    let dotfiles = std::fs::read_dir(&dotfiles_path)?;
    let info = StateInfo {
        home_dir,
        dotfiles_path: dotfiles_path.clone(),
        args: &args,
        env_var_regex,
        overrides,
        ignores,
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

    if args.apply {
        for config in configs {
            apply_config(&config, &info)?;
        }
    }

    Ok(())
}
