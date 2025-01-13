use crate::args::Args;
use crate::result::Result;
use git2::BranchType;
use std::path::{Path, PathBuf};

pub use crate::git::status::get_status;

pub fn clone(args: &Args, url: &str, target_path: &Path) -> Result<()> {
    let display_path = crate::fs::path_to_string(target_path)?;
    tracing::info!("Clone: cloning {url} into {display_path}");

    if !args.preview {
        let auth = auth_git2::GitAuthenticator::default();
        let auth = auth.add_default_ssh_keys();
        auth.clone_repo(url, target_path)?;
    }
    Ok(())
}

pub fn init(path: &PathBuf, origin_url: Option<String>) -> Result<()> {
    let mut options = git2::RepositoryInitOptions::new();
    options.no_reinit(true);
    if let Some(origin_url) = origin_url {
        options.origin_url(&origin_url);
    }
    git2::Repository::init_opts(path, &options)?;
    Ok(())
}

pub fn fetch<'a>(
    repository: &'a git2::Repository,
    remote: &'a mut git2::Remote,
    refs: &[&str],
    depth: i32,
) -> Result<git2::AnnotatedCommit<'a>> {
    // git sync
    let auth = auth_git2::GitAuthenticator::default();
    let auth = auth.add_default_ssh_keys();

    let git_config = git2::Config::open_default()?;
    let mut callback = git2::RemoteCallbacks::new();

    callback.credentials(auth.credentials(&git_config));

    // Print out our transfer progress.
    callback.transfer_progress(|stats| {
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
    fo.remote_callbacks(callback);
    fo.depth(depth);
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

/// NOTE: Returns `Ok(true)` if any files were committed and `Ok(false)` if there was nothing to
/// commit
pub fn commit_changes(
    args: &Args,
    path: &PathBuf,
    repo_config: &crate::git::config::RepositoryConfig,
) -> Result<bool> {
    tracing::info!("Sync: Committing local changes");
    let repository = git2::Repository::open(path)?;

    let mut index = repository.index()?;
    index.add_all(
        ["*"].iter(),
        git2::IndexAddOption::DEFAULT,
        Some(&mut |_path: &Path, _: &[u8]| 0),
    )?;

    // TODO: Determine if there are any changes
    let mut status_options = git2::StatusOptions::default();
    status_options
        .include_untracked(true)
        .include_unmodified(false);

    let mut has_changes = false;
    for statuses in repository.statuses(Some(&mut status_options)).iter() {
        if !statuses.is_empty() {
            has_changes = true;
            break;
        }
        // for status in statuses.iter() {
        //     tracing::warn!("{}: {:?}", status.path().unwrap_or("huh"), status.status());
        // }
    }

    if !has_changes {
        // NOTE: no changes to commit
        return Ok(false);
    }
    if args.preview {
        return Ok(true);
    }
    index.write()?;

    let host = whoami::fallible::hostname()?;
    let mut message = format!("sync: push changes from {host}");
    if let Some(commit_message) = &args.message {
        message = commit_message.to_owned();
    } else if let Some(commit_message) = &repo_config.commit_message {
        message = commit_message.to_owned();
    }

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

    Ok(true)
}

/// NOTE: Returns `Ok(true)` if any changes were pushed and `Ok(false)` there was nothing to push
pub fn push_changes(
    args: &Args,
    path: &PathBuf,
    repo_config: &crate::git::config::RepositoryConfig,
) -> Result<bool> {
    tracing::info!("Sync: Pushing local changes");
    let repository = git2::Repository::open(path)?;
    let mut remote = repository.find_remote(&repo_config.remote)?;

    let local_branch = repository.find_branch(&repo_config.branch, BranchType::Local)?;
    let head = repository.head()?.target();
    if let Some(head) = head {
        let upstream_branch = local_branch.upstream()?;
        let upstream = upstream_branch.get().target();
        if let Some(upstream) = upstream {
            let (ahead, _behind) = repository.graph_ahead_behind(head, upstream)?;
            if ahead < 1 {
                // NOTE: no changes to push
                return Ok(false);
            }
        }
    }

    if args.preview {
        return Ok(true);
    }

    let auth = auth_git2::GitAuthenticator::default();
    let auth = auth.add_default_ssh_keys();

    let git_config = git2::Config::open_default()?;
    let mut callback = git2::RemoteCallbacks::new();
    callback.credentials(auth.credentials(&git_config));

    auth.push(
        &repository,
        &mut remote,
        &[&format!("refs/heads/{}", repo_config.branch)],
    )?;

    Ok(true)
}

pub fn update_dotfiles(
    args: &Args,
    path: &PathBuf,
    repo_config: &crate::git::config::RepositoryConfig,
) -> Result<()> {
    tracing::info!(
        "Sync: Updating .dotfiles from {} {}",
        repo_config.remote,
        repo_config.branch
    );
    if args.preview {
        return Ok(());
    }
    let repository = git2::Repository::open(path)?;
    let mut remote = repository.find_remote(&repo_config.remote)?;
    let head = fetch(&repository, &mut remote, &[&repo_config.branch], 1)?;
    crate::git::merge::merge(&repository, &head, repo_config)?;
    Ok(())
}

pub fn patch(
    args: &Args,
    url: impl AsRef<str>,
    remote_patch_path: impl AsRef<Path>,
    patch_target: impl AsRef<Path>,
    dotfiles_path: impl AsRef<Path>,
    force: bool,
) -> Result<()> {
    let url = url.as_ref();
    let url = if url.contains("https://") || url.contains("ssh://") || url.contains("http://") {
        url
    } else {
        &format!("https://github.com/{}.git", url)
    };

    let remote_patch_path = remote_patch_path.as_ref();
    let temp_root = dotfiles_path.as_ref().join(".straw/.tmp/patch");
    let temp_path = temp_root.join(&patch_target);
    let patch_target = dotfiles_path.as_ref().join(patch_target);

    let patch_target_string = crate::fs::path_to_string(&patch_target)?;
    let temp_path_string = crate::fs::path_to_string(&temp_path)?;
    let remote_patch_path_string = crate::fs::path_to_string(remote_patch_path)?;

    if !args.preview {
        if temp_path.exists() {
            if temp_path.is_dir() {
                std::fs::remove_dir_all(&temp_path)?;
            } else {
                std::fs::remove_file(&temp_path)?;
            }
        }

        if patch_target.exists() {
            if force {
                if patch_target.is_dir() {
                    std::fs::remove_dir_all(&patch_target)?;
                } else {
                    std::fs::remove_file(&patch_target)?;
                }
            } else {
                return Err(crate::result::StrawError::String(format!("Error: could not apply the patch. Path `{patch_target_string}` already exists. Use `--force` to overwrite.")));
            }
        }

        let auth = auth_git2::GitAuthenticator::default();
        let auth = auth.add_default_ssh_keys();

        let git_config = git2::Config::open_default()?;
        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.depth(1);
        let mut remote_callbacks = git2::RemoteCallbacks::new();

        let mut checkout_options = git2::build::CheckoutBuilder::new();
        checkout_options.path(remote_patch_path);

        let mut repo_builder = git2::build::RepoBuilder::new();
        repo_builder.with_checkout(checkout_options);

        remote_callbacks.credentials(auth.credentials(&git_config));
        fetch_options.remote_callbacks(remote_callbacks);
        repo_builder.fetch_options(fetch_options);

        tracing::info!(
            "Patch: cloning '{remote_patch_path_string}' from {url} into '{temp_path_string}'"
        );
        repo_builder.clone(url, &temp_path)?;

        tracing::info!("Patch: applying patch to '{patch_target_string}'");
        let from = temp_path.join(remote_patch_path);
        if from.is_dir() {
            crate::fs::copy_dir_all(&temp_path.join(remote_patch_path), &patch_target)?;
        } else {
            if !patch_target.exists() {
                std::fs::create_dir_all(&patch_target)?;
            }
            tracing::trace!(
                "Patch: copying '{}' -> '{}'",
                crate::fs::path_to_string(&from)?,
                crate::fs::path_to_string(&patch_target.join(remote_patch_path_string))?,
            );
            std::fs::copy(&from, patch_target.join(remote_patch_path))?;
        }
        std::fs::remove_dir_all(temp_root)?;
    }
    Ok(())
}
