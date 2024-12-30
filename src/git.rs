use crate::args::Args;
use crate::fs;
use crate::result::{Result, StrawError};
use git2::BranchType;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::path::{Path, PathBuf};

// TODO: move remote and branch (and flavor?) to `GitDefaults`
// TODO: add optional overrides as a toml-based `.git-overrides`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepositoryConfig {
    pub remote: String,
    pub branch: String,
    pub merge_flavor: Option<MergeFlavor>,
    pub commit_message: Option<String>,
}

impl Default for RepositoryConfig {
    fn default() -> Self {
        Self {
            remote: "origin".into(),
            branch: "main".into(),
            merge_flavor: Some(Default::default()), // we want to default to Rebase
            commit_message: Default::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub enum MergeFlavor {
    #[default]
    Rebase,
    MergeCommit,
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
            MergeFlavor::MergeCommit => normal_merge(repository, &head_commit, fetch_commit)?,
        }
    } else {
        tracing::info!("Sync: Already up-to-date")
    }
    Ok(())
}

pub struct GitStatus {
    file: String,
    status: git2::Status,
}

impl GitStatus {
    pub fn unchanged(&self) -> bool {
        self.status == git2::Status::CURRENT
    }

    pub fn ignored(&self) -> bool {
        self.status == git2::Status::IGNORED
    }
}

impl Display for GitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let file_status = match self.status {
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
            git2::Status::IGNORED => "ignored",
            git2::Status::CURRENT => "unchanged",
            _ => "unknown",
        };
        write!(f, "{} ({})", self.file, file_status)
    }
}

pub fn get_status(path: &PathBuf) -> Result<Vec<GitStatus>> {
    let repository = git2::Repository::open(path)?;
    let mut status_options = git2::StatusOptions::default();
    status_options
        .include_untracked(true)
        .include_unmodified(false);

    let mut out_status = Vec::default();
    for statuses in repository.statuses(Some(&mut status_options)).iter() {
        for status in statuses.iter() {
            out_status.push(GitStatus {
                file: status.path().unwrap_or_default().into(),
                status: status.status(),
            });
        }
    }

    Ok(out_status)
}

/// NOTE: Returns `Ok(true)` if any files were committed and `Ok(false)` if there was nothing to
/// commit
pub fn commit_changes(args: &Args, path: &PathBuf, repo_config: &RepositoryConfig) -> Result<bool> {
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
pub fn push_changes(args: &Args, path: &PathBuf, repo_config: &RepositoryConfig) -> Result<bool> {
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
    let mut cb = git2::RemoteCallbacks::new();
    cb.credentials(auth.credentials(&git_config));

    auth.push(
        &repository,
        &mut remote,
        &[&format!("refs/heads/{}", repo_config.branch)],
    )?;

    Ok(true)
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
        "Sync: Updating .dotfiles from {} {}",
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

pub fn clone(args: &Args, url: &str, target_path: &Path, home_dir: &str) -> Result<()> {
    let display_path = fs::path_to_string(target_path, home_dir);
    tracing::info!("Clone: cloning {url} into {display_path}");

    if !args.preview {
        let auth = auth_git2::GitAuthenticator::default();
        let auth = auth.add_default_ssh_keys();
        auth.clone_repo(url, target_path)?;
    }
    Ok(())
}
