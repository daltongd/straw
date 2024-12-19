use crate::args::Args;
use crate::result::{Result, StrawError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct RepositoryConfig {
    pub remote: String,
    pub branch: String,
    pub merge_flavor: Option<MergeFlavor>,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub enum MergeFlavor {
    #[default]
    Rebase,
    Merge,
}

pub fn fetch<'a>(
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

pub fn fast_forward_merge(
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

pub fn normal_merge(
    repository: &git2::Repository,
    local_commit: &git2::AnnotatedCommit,
    remote_commit: &git2::AnnotatedCommit,
) -> Result<()> {
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

    let _merge_commit = repository.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &message,
        &result_tree,
        &[&local_commit, &remote_commit],
    )?;

    Ok(())
}

pub fn merge(
    repository: &git2::Repository,
    fetch_commit: &git2::AnnotatedCommit,
    repo_config: &RepositoryConfig,
) -> Result<()> {
    let analysis = repository.merge_analysis(&[fetch_commit])?;
    if analysis.0.is_fast_forward() {
        tracing::info!("Sync: Performing a fast-forward merge");
        let refs = format!("refs/heads/{}", &repo_config.branch);
        match repository.find_reference(&refs) {
            Ok(mut refs) => fast_forward_merge(repository, &mut refs, fetch_commit)?,
            Err(_) => {
                repository.reference(
                    &refs,
                    fetch_commit.id(),
                    true,
                    &format!("Setting {} to {}", &repo_config.branch, fetch_commit.id()),
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
        match repo_config.merge_flavor.clone().unwrap_or_default() {
            MergeFlavor::Rebase => rebase(repository, &head_commit, fetch_commit)?,
            MergeFlavor::Merge => normal_merge(repository, &head_commit, fetch_commit)?,
        }
    } else {
        tracing::info!("Sync: Already up-to-date")
    }
    Ok(())
}

/// NOTE: Returns `Ok(true)` if any files were committed and `Ok(false)` otherwise
pub fn commit_changes(
    args: &Args,
    path: &PathBuf,
    _repo_config: &RepositoryConfig,
) -> Result<bool> {
    tracing::info!("Sync: Committing local changes");
    let repository = git2::Repository::open(path)?;

    let mut at_least_one_file_added = false;
    let mut index = repository.index()?;
    index.add_all(
        ["*"].iter(),
        git2::IndexAddOption::DEFAULT,
        Some(&mut |_path: &Path, _: &[u8]| {
            at_least_one_file_added = true;
            0
        }),
    )?;
    if !at_least_one_file_added {
        // NOTE: no changes to commit
        return Ok(false);
    }
    if args.preview {
        return Ok(true);
    }
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

    Ok(true)
}

pub fn push_changes(_args: &Args, path: &PathBuf, repo_config: &RepositoryConfig) -> Result<()> {
    tracing::info!("Sync: Pushing local changes");
    let repository = git2::Repository::open(path)?;
    let mut remote = repository.find_remote(&repo_config.remote)?;

    let auth = auth_git2::GitAuthenticator::default();
    let auth = auth.add_default_ssh_keys();

    let git_config = git2::Config::open_default()?;
    let mut cb = git2::RemoteCallbacks::new();
    cb.credentials(auth.credentials(&git_config));

    auth.push(
        &repository,
        &mut remote,
        &[&format!("refs/heads/{}", repo_config.branch)],
    )?;

    Ok(())
}

pub fn rebase(
    repository: &git2::Repository,
    local_commit: &git2::AnnotatedCommit,
    remote_commit: &git2::AnnotatedCommit,
) -> Result<()> {
    tracing::info!("Sync: Perfroming rebase");
    let mut rebase_opts = git2::RebaseOptions::default();
    let mut rebase = repository.rebase(
        Some(local_commit),
        Some(remote_commit),
        None,
        Some(&mut rebase_opts),
    )?;

    let signature = repository.signature()?;
    while let Some(Ok(_rebase_op)) = &rebase.next() {
        rebase.commit(None, &signature, None)?;
    }
    rebase.finish(None)?;

    tracing::info!("Sync: Rebase done");

    Ok(())
}

pub fn update_dotfiles(args: &Args, path: &PathBuf, repo_config: &RepositoryConfig) -> Result<()> {
    tracing::info!(
        "Sync: updating .dotfiles from {} {}",
        repo_config.remote,
        repo_config.branch
    );
    if args.preview {
        return Ok(());
    }
    let repository = git2::Repository::open(path)?;
    let mut remote = repository.find_remote(&repo_config.remote)?;
    let head = fetch(&repository, &mut remote, &[&repo_config.branch])?;
    merge(&repository, &head, repo_config)?;
    Ok(())
}

pub fn clone(args: &Args, url: &str, target_path: &Path) -> Result<()> {
    let display_path = target_path.display().to_string().replace("\\", "/");
    tracing::info!("Clone: cloning {url} into {display_path}");

    if !args.preview {
        let auth = auth_git2::GitAuthenticator::default();
        let auth = auth.add_default_ssh_keys();
        auth.clone_repo(url, target_path)?;
    }
    Ok(())
}
