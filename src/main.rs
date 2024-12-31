use std::path::PathBuf;

use clap::Parser;
use regex::Regex;
use straw::{
    args::{Args, Commands},
    cache::Cache,
    git::config::{MergeFlavor, RepositoryConfig},
    result::{Result, StrawError},
    state::StateInfo,
};

fn find_home() -> Result<String> {
    let home = home::home_dir().ok_or(StrawError::String(
        "Couldn't determine the home directory path.".into(),
    ))?;
    let home_dir = home.as_os_str().to_str().ok_or(StrawError::String(
        "Couldn't convert {home:?} to &str.".into(),
    ))?;

    let display_home = home_dir.replace("\\", "/");
    tracing::info!("Found home directory: {display_home}");

    Ok(home_dir.into())
}

fn resolve_repo_config(lua: &mlua::Lua, args: &Args) -> Result<RepositoryConfig> {
    let mut repo_config = straw::lua::run_git_config(lua)?;

    // apply args overrides
    if let Some(branch) = &args.branch {
        repo_config.branch = branch.to_owned();
    }
    if let Some(remote) = &args.remote {
        repo_config.remote = remote.to_owned();
    }

    if args.merge_commit && args.rebase {
        return Err(StrawError::String("The arguments --rebase and --merge-commit are mutually exclusive. Please use only one at a time.".into()));
    } else if args.rebase {
        repo_config.merge_flavor = Some(MergeFlavor::Rebase);
    } else if args.merge_commit {
        repo_config.merge_flavor = Some(MergeFlavor::MergeCommit);
    }

    Ok(repo_config)
}

fn main() -> Result<()> {
    let args = Args::parse();

    #[cfg(windows)]
    let env_var_regex = Regex::new(r"%(\w+)%")?;
    #[cfg(unix)]
    let env_var_regex = Regex::new(r"\$(\w+)")?;

    #[cfg(debug_assertions)]
    let max_log_level = tracing::Level::TRACE;
    #[cfg(not(debug_assertions))]
    let max_log_level = tracing::Level::INFO;

    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_max_level(max_log_level)
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

    let home_dir = find_home()?;
    let dotfiles_path: PathBuf = [&home_dir, straw::packages::DOTFILES_DIR].iter().collect();

    let lua = straw::lua::create_lua_runtime(&dotfiles_path, &args)?;

    let repo_config = match resolve_repo_config(&lua, &args) {
        Ok(repo_config) => repo_config,
        Err(e) => {
            tracing::error!("{e}");
            return Err(e);
        }
    };

    if let Some(command) = &args.command {
        match command {
            Commands::Clone { url } => {
                match straw::git::ops::clone(&args, url, &dotfiles_path, &home_dir) {
                    Ok(_) => {
                        tracing::info!(
                            "Clone: successfully cloned {url} repository into {}",
                            dotfiles_path.display()
                        );
                    }
                    Err(e) => {
                        tracing::error!("Error cloning the repository: {e}");
                        return Err(e);
                    }
                }
            }
            Commands::Init { url } => match straw::git::ops::init(&dotfiles_path, url.to_owned()) {
                Ok(_) => {
                    tracing::info!(
                        "Clone: successfully initialized a repository in {}",
                        dotfiles_path.display()
                    );
                    if let Some(origin_url) = url {
                        tracing::info!("Clone: origin url set to {origin_url}");
                    }
                }
                Err(e) => {
                    tracing::error!("Error initializng the repository: {e}");
                    return Err(e);
                }
            },
            Commands::Status => {
                let statuses = straw::git::ops::get_status(&dotfiles_path)?;
                tracing::info!("Sync: Status");
                if statuses.is_empty() {
                    tracing::info!("Status: Working directory clean.");
                } else {
                    for status in statuses {
                        tracing::info!("Status: {}", status)
                    }
                }
            }
            Commands::Adopt { path } => {
                straw::packages::adopt(&args, path, &home_dir, &dotfiles_path)?;
            }
        }
    } else {
        if args.sync || args.commit || args.message.is_some() {
            // commit changes
            match straw::git::ops::commit_changes(&args, &dotfiles_path, &repo_config) {
                Ok(any_changes_committed) => {
                    if any_changes_committed {
                        tracing::info!("Commit: Done committing local changes to .dotfiles")
                    } else {
                        tracing::info!("Commit: No local changes to commit");
                    }
                }
                Err(e) => tracing::error!(
                    "Commit: Failed to commit the local changes to .dotfiles with error: {e:?}"
                ),
            }
        }
        if args.sync || args.pull {
            // update git repo
            match straw::git::ops::update_dotfiles(&args, &dotfiles_path, &repo_config) {
                Ok(_) => tracing::info!("Update: .dotfiles updated"),
                Err(e) => tracing::error!("Sync: Failed to update the .dotfiles with error: {e:?}"),
            }
        }
        if args.sync || args.push {
            match straw::git::ops::push_changes(&args, &dotfiles_path, &repo_config) {
                Ok(any_changes_pushed) => {
                    if any_changes_pushed {
                        tracing::info!("Push: local changes pushed to {}", repo_config.remote)
                    } else {
                        tracing::info!("Push: No local changes to push")
                    }
                }
                Err(e) => {
                    tracing::error!("Push: Failed to push the local changes with error: {e:?}")
                }
            }
        }

        let config = straw::lua::run_config(&lua)?;

        let info = StateInfo {
            home_dir: &home_dir,
            dotfiles_path: dotfiles_path.clone(),
            args: &args,
            env_var_regex,
            config,
        };

        let cache_path = &dotfiles_path.join(".straw/.cache");
        let mut cache = if let Ok(cached_packages) = Cache::read(cache_path) {
            tracing::trace!("Cached packages found: {cached_packages:?}");
            cached_packages
        } else {
            Cache::new(cache_path)
        };

        let packages_specified = !args.packages.is_empty();

        if args.clear || args.apply || packages_specified {
            straw::packages::clear_deployed_packages(&mut cache, &info)?;
        }

        // prepare common directory structure before applying the packages onto the system
        if args.apply {
            let folded_paths = straw::folding::fold(&dotfiles_path, &info)?;
            for path in folded_paths {
                tracing::trace!("Folding: Found path {path:?}");
                if !path.exists() {
                    tracing::trace!("Folding: Creating common directory at {path:?}");
                    std::fs::create_dir_all(path)?;
                }
            }
        }

        if packages_specified {
            tracing::info!("Packages specified: {:?}", args.packages);
            straw::packages::apply_packages(&mut cache, &args.packages, &info)?;
        } else if args.apply {
            let mut packages = Vec::new();
            let dotfiles = std::fs::read_dir(&dotfiles_path)?;
            for path in dotfiles {
                let path = path?;
                if let Some(item_name) = path.file_name().to_str() {
                    if path.file_type()?.is_dir() && !item_name.starts_with(".") {
                        packages.push(item_name.to_owned());
                    }
                }
            }
            tracing::info!("Packages found: {packages:?}");
            straw::packages::apply_packages(&mut cache, &packages, &info)?;
        }

        if args.clear || args.apply || packages_specified {
            cache.save()?;
        }
    }

    Ok(())
}
