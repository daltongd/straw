use std::{
    fs::ReadDir,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::Parser;
use regex::{Captures, Regex};
use straw::{
    args::{Args, Commands},
    cache::Cache,
    git::{MergeFlavor, RepositoryConfig},
    result::{Result, StrawError},
    state::StateInfo,
};

const DOTFILES_DIR: &str = ".dotfiles/";

fn fold_and_apply(
    cache: &mut Cache,
    name: &str,
    info: &StateInfo,
    from_dir: ReadDir,
    to: PathBuf,
) -> Result<()> {
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
                    tracing::trace!("{name}: {display_path} exists");
                    let dir = std::fs::read_dir(item.path())?;
                    fold_and_apply(cache, name, info, dir, to)?;
                }
                found_existing = true;
            }
        }

        if !found_existing {
            let from = item.path();
            let to = to.join(item.file_name());

            tracing::trace!("{name}: Creating symlink");

            straw::fs::remove_symlink_if_exists(info, &to)?;
            straw::fs::create_symlink(info, from, to.clone())?;
            cache.cache_package(name, &to);
        }
    }

    Ok(())
}

fn apply_override(cache: &mut Cache, name: &str, info: &StateInfo) -> Result<()> {
    let path_override = &info.config.overrides[name];

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
    let to = to.replace("~", info.home_dir);

    let display_from = from.replace("\\", "/");
    let display_to = path_override.to.replace("\\", "/");
    tracing::info!("\t\tFound override:\t\t\t{display_from} -> {display_to}");

    let to = PathBuf::from_str(to.as_str())
        .map_err(|e| StrawError::String(format!("Can't resolve path: {to:?}. {e:?}")))?;

    straw::fs::remove_symlink_if_exists(info, &to)?;

    if to.exists() && !to.is_symlink() {
        let to = to.display().to_string().replace("\\", "/");
        tracing::warn!("{to} already exists, ignoring");
    }

    // could be false if the file wasn't successfully removed
    if !to.exists() || info.args.preview {
        // check if the parent path exists
        if let Some(parent) = to.parent() {
            if !parent.exists() {
                straw::fs::create_dir(parent)?;
            }
        } // else - `to` is root, ignore

        let from_path: PathBuf = [info.home_dir, DOTFILES_DIR, name, from.as_str()]
            .iter()
            .collect();
        straw::fs::create_symlink(info, from_path, to.clone())?;
        cache.cache_package(name, &to);
    }

    Ok(())
}

fn apply_package(cache: &mut Cache, name: &str, info: &StateInfo) -> Result<()> {
    let package_path = &info.dotfiles_path;
    let package_path = package_path.join(name);

    let package_dir = std::fs::read_dir(&package_path)?;

    tracing::trace!("Applying package: {name}");
    if info.config.ignores.contains(&name.into()) || info.args.ignore.contains(&name.into()) {
        tracing::trace!("Ignoring {name}");
        return Ok(());
    }

    let deployed_links_for_package = cache.find_deployed_links(name);
    let deployed_links = cache.get_deployed_packages_mut();
    for (i, link) in deployed_links_for_package {
        straw::fs::remove_symlink_if_exists(info, &PathBuf::from(link))?;
        deployed_links.remove(i);
    }

    if info.config.overrides.contains_key(name) {
        apply_override(cache, name, info)?;
    } else {
        fold_and_apply(cache, name, info, package_dir, PathBuf::from(info.home_dir))?;
    }

    Ok(())
}

fn clear_deployed_packages(cache: &mut Cache, info: &StateInfo) -> Result<()> {
    for package in cache.get_deployed_packages() {
        straw::fs::remove_symlink_if_exists(info, &PathBuf::from(&package.path))?;
    }
    cache.clear();

    Ok(())
}

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

// TODO: evaluate if this shouldn't be replaced with some other file or console arguments
fn resolve_repo_config(args: &Args, dotfiles_path: &Path) -> Result<RepositoryConfig> {
    let mut repo_config = straw::git::RepositoryConfig {
        remote: String::from("origin"),
        branch: String::from("main"),
        merge_flavor: Some(MergeFlavor::default()),
        commit_message: None,
    };

    if let Ok(config) = straw::lua::run_lua_config(dotfiles_path) {
        repo_config = config.git;
    }

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
    let dotfiles_path: PathBuf = [&home_dir, DOTFILES_DIR].iter().collect();

    let repo_config = match resolve_repo_config(&args, &dotfiles_path) {
        Ok(repo_config) => repo_config,
        Err(e) => {
            tracing::error!("{e}");
            return Err(e);
        }
    };

    if let Some(Commands::Clone { url }) = &args.command {
        match straw::git::clone(&args, url, &dotfiles_path) {
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
    } else if let Some(Commands::Init { url }) = &args.command {
        match straw::git::init(&dotfiles_path, url.to_owned()) {
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
        }
    } else {
        if args.status {
            let status = straw::git::get_status(&dotfiles_path)?;
            tracing::info!("Sync: Status");
            if status.is_empty() {
                tracing::info!("Status: Working directory clean.");
            } else {
                for (path, file_status) in status {
                    let file_status = match file_status {
                        git2::Status::INDEX_NEW => "new",
                        git2::Status::INDEX_MODIFIED => "modified",
                        git2::Status::INDEX_DELETED => "deleted",
                        git2::Status::INDEX_RENAMED => "renamed",
                        git2::Status::INDEX_TYPECHANGE => "type change",
                        git2::Status::WT_NEW => "new",
                        git2::Status::WT_MODIFIED => "modified",
                        git2::Status::WT_DELETED => "deleted",
                        git2::Status::WT_RENAMED => "renamed",
                        git2::Status::WT_TYPECHANGE => "type change",
                        git2::Status::CONFLICTED => "conflict",
                        _ => continue,
                    };
                    tracing::info!("Status: {} ({})", path, file_status)
                }
            }
        }
        if args.sync || args.commit || args.message.is_some() {
            // commit changes
            match straw::git::commit_changes(&args, &dotfiles_path, &repo_config) {
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
            match straw::git::update_dotfiles(&args, &dotfiles_path, &repo_config) {
                Ok(_) => tracing::info!("Update: .dotfiles updated"),
                Err(e) => tracing::error!("Sync: Failed to update the .dotfiles with error: {e:?}"),
            }
        }
        if args.sync || args.push {
            match straw::git::push_changes(&args, &dotfiles_path, &repo_config) {
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
    }

    let config = straw::lua::run_lua_config(&dotfiles_path)?;

    let dotfiles = std::fs::read_dir(&dotfiles_path)?;
    let info = StateInfo {
        home_dir: &home_dir,
        dotfiles_path: dotfiles_path.clone(),
        args: &args,
        env_var_regex,
        config,
    };

    let mut packages = Vec::new();
    for path in dotfiles {
        let path = path?;
        if let Some(item_name) = path.file_name().to_str() {
            if path.file_type()?.is_dir() && !item_name.starts_with(".") {
                packages.push(item_name.to_owned());
            }
        }
    }
    tracing::info!("Packages found: {packages:?}");

    let cache_path = &dotfiles_path.join(".straw/.cache");
    let mut cache = Cache::default();
    if let Ok(cached_packages) = Cache::read(cache_path) {
        tracing::trace!("Cached packages found: {cached_packages:?}");
        cache = cached_packages;
    }

    if args.clear || args.apply {
        clear_deployed_packages(&mut cache, &info)?;
    }

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

    if !args.packages.is_empty() {
        for package in &args.packages {
            apply_package(&mut cache, package, &info)?;
        }
    } else if args.apply {
        for package in &packages {
            apply_package(&mut cache, package, &info)?;
        }
    }
    cache.save(cache_path)?;

    Ok(())
}
