use std::{
    collections::HashMap,
    fs::ReadDir,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::Parser;
use regex::{Captures, Regex};
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

    ignores: Option<Vec<String>>,
    ignores_windows: Option<Vec<String>>,
    ignores_unix: Option<Vec<String>>,
}

#[derive(Debug)]
struct Info<'a> {
    home_dir: &'a str,
    dotfiles_path: PathBuf,
    args: &'a Args,
    env_var_regex: Regex,
    overrides: HashMap<String, Override>,
    ignores: Vec<String>,
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
                    tracing::info!("{name}: {existing:?} exists");
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
            #[cfg(windows)]
            std::os::windows::fs::symlink_dir(from, to)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(from, to)?;
        }
        tracing::info!("\t\tCreated a directory link");
    } else if from.is_file() {
        if !info.args.preview {
            #[cfg(windows)]
            std::os::windows::fs::symlink_file(from, to)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(from, to)?;
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

    tracing::info!("{name}");
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
        tracing::info!("\t    {from} -> {to}");

        let to = PathBuf::from_str(to.as_str())
            .map_err(|e| StrawError::String(format!("Can't resolve path: {to:?}. {e:?}")))?;

        remove_symlink_if_exists(info, &to)?;
        if to.exists() && !info.args.force {
            tracing::warn!("\t\t{to:?} already exists, ignoring. Run with --force to overwrite.");
        }

        // could be false if the file wasn't successfully removed
        if !to.exists() || info.args.preview {
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

fn fetch<'a>(
    repository: &'a git2::Repository,
    remote: &'a mut git2::Remote,
    refs: &[&str],
) -> Result<git2::AnnotatedCommit<'a>> {
    // git sync
    let auth = auth_git2::GitAuthenticator::default();
    let auth = auth.add_default_ssh_keys();

    let git_config = git2::Config::open_default()?;
    let mut cb = git2::RemoteCallbacks::new();

    cb.credentials(auth.credentials(&git_config));

    // Print out our transfer progress.
    cb.transfer_progress(|stats| {
        if stats.received_objects() == stats.total_objects() {
            tracing::info!(
                "Sync: Resolving deltas {}/{}\r",
                stats.indexed_deltas(),
                stats.total_deltas()
            );
        } else if stats.total_objects() > 0 {
            tracing::info!(
                "Sync: Received {}/{} objects ({}) in {} bytes\r",
                stats.received_objects(),
                stats.total_objects(),
                stats.indexed_objects(),
                stats.received_bytes()
            );
        }
        true
    });

    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(cb);
    // Always fetch all tags.
    // Perform a download and also update tips
    fo.download_tags(git2::AutotagOption::All);
    tracing::info!("Sync: Fetching {} for .dotfiles", remote.name().unwrap());
    remote.fetch(refs, Some(&mut fo), None)?;

    let stats = remote.stats();
    if stats.local_objects() > 0 {
        tracing::info!(
            "Sync: Received {}/{} objects in {} bytes (used {} local \
             objects)",
            stats.indexed_objects(),
            stats.total_objects(),
            stats.received_bytes(),
            stats.local_objects()
        );
    } else {
        tracing::info!(
            "Sync: Received {}/{} objects in {} bytes",
            stats.indexed_objects(),
            stats.total_objects(),
            stats.received_bytes()
        );
    }

    let fetch_head = repository.find_reference("FETCH_HEAD")?;

    Ok(repository.reference_to_annotated_commit(&fetch_head)?)
}

fn fast_forward_merge(
    repository: &git2::Repository,
    local_branch: &mut git2::Reference,
    remote_commit: &git2::AnnotatedCommit,
) -> Result<()> {
    let name = match local_branch.name() {
        Some(s) => s.to_string(),
        None => String::from_utf8_lossy(local_branch.name_bytes()).to_string(),
    };
    let msg = format!("Fast-forward: Setting {} to {}", name, remote_commit.id());
    tracing::info!("Sync: {msg}");
    local_branch.set_target(remote_commit.id(), &msg)?;
    repository.set_head(&name)?;
    repository.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
    Ok(())
}

fn normal_merge(
    repository: &git2::Repository,
    local_commit: &git2::AnnotatedCommit,
    remote_commit: &git2::AnnotatedCommit,
) -> Result<git2::Oid> {
    let local_tree = repository.find_commit(local_commit.id())?.tree()?;
    let remote_tree = repository.find_commit(remote_commit.id())?.tree()?;
    let ancestor_tree = repository
        .find_commit(repository.merge_base(local_commit.id(), remote_commit.id())?)?
        .tree()?;
    let mut index = repository.merge_trees(&ancestor_tree, &local_tree, &remote_tree, None)?;

    if index.has_conflicts() {
        // TODO: return error instead?
        repository.checkout_index(Some(&mut index), None)?;
        return Err(StrawError::String(
            "Merge conflicts detected. Manual intervention required.".into(),
        ));
    }

    let result_tree = repository.find_tree(index.write_tree_to(repository)?)?;

    // create merge commit
    let message = format!("merge: {} into {}", remote_commit.id(), local_commit.id());
    let signature = repository.signature()?;
    let local_commit = repository.find_commit(local_commit.id())?;
    let remote_commit = repository.find_commit(remote_commit.id())?;

    let merge_commit = repository.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &message,
        &result_tree,
        &[&local_commit, &remote_commit],
    )?;

    Ok(merge_commit)
}

fn merge(
    repository: &git2::Repository,
    fetch_commit: git2::AnnotatedCommit,
    branch: &str,
) -> Result<()> {
    let analysis = repository.merge_analysis(&[&fetch_commit])?;
    if analysis.0.is_fast_forward() {
        tracing::info!("Sync: Performing a fast-forward merge");
        let refs = format!("refs/heads/{branch}");
        match repository.find_reference(&refs) {
            Ok(mut refs) => fast_forward_merge(repository, &mut refs, &fetch_commit)?,
            Err(_) => {
                repository.reference(
                    &refs,
                    fetch_commit.id(),
                    true,
                    &format!("Setting {} to {}", branch, fetch_commit.id()),
                )?;
                repository.set_head(&refs)?;
                repository.checkout_head(Some(
                    git2::build::CheckoutBuilder::default()
                        .allow_conflicts(true)
                        .conflict_style_merge(true)
                        .force(),
                ))?;
            }
        }
    } else if analysis.0.is_normal() {
        tracing::info!("Sync: Performing merge");
        let head_commit = repository.reference_to_annotated_commit(&repository.head()?)?;
        normal_merge(repository, &head_commit, &fetch_commit)?;
    } else {
        tracing::info!("Sync: Already up-to-date")
    }
    Ok(())
}

fn commit_changes(path: &PathBuf, remote: &str, branch: &str) -> Result<()> {
    tracing::info!("Sync: Committing local changes");
    let repository = git2::Repository::open(path)?;

    let mut index = repository.index()?;
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;

    let host = hostname::get()?;
    let host = host.to_str().unwrap_or("unknown host");
    let message = format!("sync: push changes from {host}");

    let signature = repository.signature()?;
    let parent_commit = repository.head()?.peel_to_commit()?;

    let tree = repository.find_tree(index.write_tree()?)?;

    let update_ref = if let Ok(_head) = repository.head() {
        Some("HEAD")
    } else {
        None // no HEAD = first commit
    };

    let _commit = repository.commit(
        update_ref,
        &signature,
        &signature,
        &message,
        &tree,
        &[&parent_commit],
    )?;

    let mut remote = repository.find_remote(remote)?;

    let auth = auth_git2::GitAuthenticator::default();
    let auth = auth.add_default_ssh_keys();

    let git_config = git2::Config::open_default()?;
    let mut cb = git2::RemoteCallbacks::new();
    cb.credentials(auth.credentials(&git_config));

    auth.push(&repository, &mut remote, &[&format!("refs/heads/{branch}")])?;

    Ok(())
}

fn update_dotfiles(path: &PathBuf, remote: &str, branch: &str) -> Result<()> {
    let repository = git2::Repository::open(path)?;
    let mut remote = repository.find_remote(remote)?;
    let head = fetch(&repository, &mut remote, &[branch])?;
    merge(&repository, head, branch)?;
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

    let home = home::home_dir().ok_or(StrawError::String(
        "Couldn't determine the home directory path.".into(),
    ))?;
    let home_dir = home.as_os_str().to_str().ok_or(StrawError::String(
        "Couldn't convert {home:?} to &str.".into(),
    ))?;

    tracing::info!("Found home directory: {home:?}");
    let dotfiles_path: PathBuf = [home_dir, DOTFILES_DIR].iter().collect();
    let config_file_path: PathBuf = [home_dir, DOTFILES_DIR, CONFIG_FILE].iter().collect();

    // commit changes
    match commit_changes(&dotfiles_path, "origin", "main") {
        Ok(_) => tracing::info!("Sync: local changes to .dotfiles committed"),
        Err(e) => tracing::error!(
            "Sync: Failed to commit the local changes to .dotfiles with error: {e:?}"
        ),
    }
    // update git repo
    match update_dotfiles(&dotfiles_path, "origin", "main") {
        Ok(_) => tracing::info!("Sync: .dotfiles updated"),
        Err(e) => tracing::error!("Sync: Failed to update the .dotfiles with error: {e:?}"),
    }

    let mut overrides = HashMap::new();
    let mut ignores: Vec<String> = Vec::new();
    if let Ok(config_file) = std::fs::read_to_string(&config_file_path) {
        tracing::info!("Found config: {config_file_path:?}");
        let straw_config_parsed: Config = serde_json::from_str(config_file.as_str())?;

        if let Some(general_overrides) = &straw_config_parsed.overrides {
            overrides.extend(general_overrides.clone());
        }

        #[cfg(windows)]
        if let Some(windows_overrides) = &straw_config_parsed.overrides_windows {
            overrides.extend(windows_overrides.clone());
        }

        #[cfg(unix)]
        if let Some(unix_overrides) = &straw_config.overrides_unix {
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
    let info = Info {
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

    for config in configs {
        apply_config(&config, &info)?;
    }

    Ok(())
}
