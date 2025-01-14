use std::path::PathBuf;

use clap::Parser;
use straw::{
    args::{Args, Commands},
    cache::Cache,
    git::config::{MergeFlavor, RepositoryConfig},
    lua::runtime::LuaRuntime,
    result::{Result, StrawError},
    state::State,
};

fn resolve_repo_config(runtime: &LuaRuntime, args: &Args) -> Result<RepositoryConfig> {
    let mut repo_config = straw::lua::run_git_config(runtime)?;

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
    let mut args = Args::parse();
    args.ssh_key_path = match args.ssh_key_path {
        Some(ssh_key_path) => Some(straw::fs::sanitize_path(&ssh_key_path)?),
        None => None,
    };

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

    let target_dir = straw::utils::find_target_dir(&args)?;
    let dotfiles_path: PathBuf = straw::utils::get_dotfiles_path(&args)?;

    let display_target = target_dir.replace("\\", "/");
    tracing::info!("Using target directory: {display_target}");
    tracing::info!(
        "Using dotfiles path: {}",
        straw::fs::path_to_string(&dotfiles_path)?
    );

    let runtime = LuaRuntime::new(&dotfiles_path, &args)?;

    // Load and execute the git lua config
    let repo_config = match resolve_repo_config(&runtime, &args) {
        Ok(repo_config) => repo_config,
        Err(e) => {
            tracing::error!("{e}");
            return Err(e);
        }
    };

    // Handle the subcommands
    if let Some(command) = &args.command {
        match command {
            Commands::Clone { url } => match straw::git::ops::clone(&args, url, &dotfiles_path) {
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
            },
            Commands::Init { url } => match straw::git::ops::init(&dotfiles_path, url.to_owned()) {
                Ok(_) => {
                    tracing::info!(
                        "Clone: successfully initialized a repository in {}",
                        dotfiles_path.display()
                    );
                    if let Some(origin_url) = url {
                        tracing::info!("Clone: origin url set to {origin_url}");
                    }
                    return Ok(());
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
                return Ok(());
            }
            Commands::Sync => {
                args.sync_repo = true;
                args.apply = true;
            }
            Commands::Adopt { path } => {
                straw::packages::adopt(&args, path, &target_dir, &dotfiles_path)?;
                return Ok(());
            }
            Commands::Run { module } => {
                tracing::info!("Run: Running {module}");
                runtime.run::<()>(&format!(
                    r#"
                        require("{module}")
                        return nil
                    "#
                ))?;
                tracing::info!("Run: Done running {module}");
                return Ok(());
            }
            Commands::Execute { script } => {
                tracing::info!("Executing the script");
                runtime.run::<()>(&format!(
                    r#"
                        local straw = require("straw")
                        {script}
                        return nil
                    "#
                ))?;
                tracing::info!("Execute: Done executing the script");
                return Ok(());
            }
            Commands::Patch {
                url,
                remote_root,
                patch_path,
                force,
            } => {
                straw::git::ops::patch(&args, url, remote_root, patch_path, dotfiles_path, *force)?;
                return Ok(());
            }
            Commands::CopyFiles { packages } => {
                for package in packages {
                    tracing::info!("{package}");
                    // straw::packages::fold_and_copy(package, args, target_dir)
                }

                todo!();
                // return Ok(());
            }
        }
    }

    // Sync - Step 1/3: commit local changes
    if args.sync_repo || args.commit || args.message.is_some() {
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

    // Sync - Step 2/3: update the local dotfiles
    if args.sync_repo || args.pull {
        // update git repo
        match straw::git::ops::update_dotfiles(&args, &dotfiles_path, &repo_config) {
            Ok(_) => tracing::info!("Update: .dotfiles updated"),
            Err(e) => tracing::error!("Sync: Failed to update the .dotfiles with error: {e:?}"),
        }
    }

    // Sync - Step 3/3: push local changes to the repo
    if args.sync_repo || args.push {
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

    // Load and execute the lua config
    let config = straw::lua::run_lua_config(&runtime)?;

    let state = State {
        target_dir: &target_dir,
        dotfiles_path: dotfiles_path.clone(),
        args: &args,
        config,
    };

    // Load cache
    let cache_path = &dotfiles_path.join(".straw/.cache");
    let mut cache = if let Ok(cached_packages) = Cache::read(cache_path) {
        tracing::trace!("Cached packages found: {cached_packages:?}");
        cached_packages
    } else {
        Cache::new(cache_path)
    };

    let packages_specified = !args.packages.is_empty();

    // Apply - Step 1/4: clear deployed packages
    if args.clear || args.apply || packages_specified {
        straw::packages::clear_deployed_packages(&mut cache, &state)?;
    }

    // Apply - Step 2/4: prepare common directory structure before applying the packages onto the system
    if args.apply {
        let folded_paths = straw::folding::fold_dotfiles(&dotfiles_path, &state)?;
        for path in folded_paths {
            tracing::trace!("Folding: Found path {path:?}");
            if !path.exists() {
                tracing::trace!("Folding: Creating common directory at {path:?}");
                std::fs::create_dir_all(path)?;
            }
        }
    }

    // Apply - Step 3/4: apply the packages onto the system
    if packages_specified {
        tracing::info!("Packages specified: {:?}", args.packages);
        straw::packages::apply_packages(&mut cache, &args.packages, &state)?;
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
        straw::packages::apply_packages(&mut cache, &packages, &state)?;
    }

    // Apply - Step 4/4: save the cache
    if args.clear || args.apply || packages_specified {
        cache.save()?;
    }

    Ok(())
}
