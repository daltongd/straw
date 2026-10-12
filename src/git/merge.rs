use crate::result::Result;
use std::path::{Path, PathBuf};

/// Which stage of a conflicted index entry holds the local version of a file.
///
/// A merge treats the local commit as "ours", while a rebase replays the local commits on top of
/// the remote, which makes them "theirs".
#[derive(Clone, Copy)]
enum LocalSide {
    Ours,
    Theirs,
}

/// Points the branch at `target` and updates the working directory to match.
///
/// The checkout runs before the branch is moved and is a safe one, so files with uncommitted
/// changes make it fail instead of being overwritten.
fn move_branch_to(repository: &git2::Repository, branch: &str, target: git2::Oid) -> Result<()> {
    let refname = format!("refs/heads/{branch}");
    let target_object = repository.find_object(target, None)?;
    repository.checkout_tree(
        &target_object,
        Some(git2::build::CheckoutBuilder::default().safe()),
    )?;
    repository.reference(
        &refname,
        target,
        true,
        &format!("straw: moving to {target}"),
    )?;
    repository.set_head(&refname)?;
    Ok(())
}

/// Aborts a merge or a rebase left behind by an interrupted sync, they would block the next one
fn cleanup_unfinished_operations(repository: &git2::Repository) -> Result<()> {
    if repository.state() == git2::RepositoryState::Clean {
        return Ok(());
    }
    tracing::warn!(
        "Sync: Found an unfinished operation ({:?}), aborting it",
        repository.state()
    );
    if let Ok(mut rebase) = repository.open_rebase(None) {
        rebase.abort()?;
    }
    repository.cleanup_state()?;
    Ok(())
}

/// Time of the newest remote commit (not reachable from `merge_base`) that changed `path`
fn remote_change_time(
    repository: &git2::Repository,
    remote_tip: git2::Oid,
    merge_base: git2::Oid,
    path: &Path,
) -> Result<i64> {
    let mut revwalk = repository.revwalk()?;
    revwalk.set_sorting(git2::Sort::TIME)?;
    revwalk.push(remote_tip)?;
    revwalk.hide(merge_base)?;
    for oid in revwalk {
        let commit = repository.find_commit(oid?)?;
        let entry = commit.tree()?.get_path(path).ok().map(|entry| entry.id());
        let parent_entry = match commit.parent(0) {
            Ok(parent) => parent.tree()?.get_path(path).ok().map(|entry| entry.id()),
            Err(_) => None,
        };
        if entry != parent_entry {
            return Ok(commit.time().seconds());
        }
    }
    Ok(repository.find_commit(remote_tip)?.time().seconds())
}

/// Time of the last local edit of `path`. The local commits are created at sync time, so the
/// file's modification time says more about its age than the commit does.
fn local_change_time(repository: &git2::Repository, local_commit: git2::Oid, path: &Path) -> i64 {
    let modified = repository
        .workdir()
        .and_then(|workdir| std::fs::metadata(workdir.join(path)).ok())
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64);
    match modified {
        Some(modified) => modified,
        None => repository
            .find_commit(local_commit)
            .map(|commit| commit.time().seconds())
            .unwrap_or_default(),
    }
}

/// Resolves every conflict in `index` by taking the whole file from the side that changed it most
/// recently. Edits that don't overlap never get here, git merges those on its own.
fn resolve_conflicts(
    repository: &git2::Repository,
    index: &mut git2::Index,
    local_side: LocalSide,
    local_commit: git2::Oid,
    remote_tip: git2::Oid,
    merge_base: git2::Oid,
) -> Result<()> {
    let conflicts = index
        .conflicts()?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for conflict in conflicts {
        let (local, remote) = match local_side {
            LocalSide::Ours => (conflict.our, conflict.their),
            LocalSide::Theirs => (conflict.their, conflict.our),
        };
        let entry_path = |entry: &Option<git2::IndexEntry>| {
            entry
                .as_ref()
                .map(|entry| PathBuf::from(String::from_utf8_lossy(&entry.path).to_string()))
        };
        let local_path = entry_path(&local);
        let remote_path = entry_path(&remote);
        let ancestor_path = entry_path(&conflict.ancestor);
        let Some(path) = local_path
            .clone()
            .or(remote_path.clone())
            .or(ancestor_path.clone())
        else {
            continue;
        };

        let local_time = local_change_time(repository, local_commit, &path);
        let remote_time = remote_change_time(
            repository,
            remote_tip,
            merge_base,
            remote_path.as_deref().unwrap_or(&path),
        )?;
        let (winner, winner_name) = if local_time >= remote_time {
            (local, "local")
        } else {
            (remote, "remote")
        };
        tracing::warn!(
            "Sync: Conflict in '{}', taking the newer {winner_name} version",
            path.display()
        );

        for conflicted_path in [&ancestor_path, &local_path, &remote_path]
            .into_iter()
            .flatten()
        {
            for stage in 1..=3 {
                // NOTE: not every stage exists for every conflict
                let _ = index.remove(conflicted_path, stage);
            }
        }
        // NOTE: no winner entry means the newer side deleted the file
        if let Some(mut winner) = winner {
            winner.flags &= !0x3000; // clear the stage bits, the entry is no longer conflicted
            index.add(&winner)?;
        }
    }
    Ok(())
}

fn fast_forward_merge(
    repository: &git2::Repository,
    branch: &str,
    remote_commit: &git2::AnnotatedCommit,
) -> Result<()> {
    tracing::info!(
        "Sync: Fast-forward: Setting {branch} to {}",
        remote_commit.id()
    );
    move_branch_to(repository, branch, remote_commit.id())
}

fn normal_merge(
    repository: &git2::Repository,
    branch: &str,
    local_commit: &git2::AnnotatedCommit,
    remote_commit: &git2::AnnotatedCommit,
) -> Result<()> {
    let merge_base = repository.merge_base(local_commit.id(), remote_commit.id())?;
    let local_tree = repository.find_commit(local_commit.id())?.tree()?;
    let remote_tree = repository.find_commit(remote_commit.id())?.tree()?;
    let ancestor_tree = repository.find_commit(merge_base)?.tree()?;
    let mut index = repository.merge_trees(&ancestor_tree, &local_tree, &remote_tree, None)?;

    if index.has_conflicts() {
        resolve_conflicts(
            repository,
            &mut index,
            LocalSide::Ours,
            local_commit.id(),
            remote_commit.id(),
            merge_base,
        )?;
    }

    let result_tree = repository.find_tree(index.write_tree_to(repository)?)?;

    // create merge commit
    let message = format!("merge: {} into {}", remote_commit.id(), local_commit.id());
    let signature = repository.signature()?;
    let local_commit = repository.find_commit(local_commit.id())?;
    let remote_commit = repository.find_commit(remote_commit.id())?;

    let merge_commit = repository.commit(
        None,
        &signature,
        &signature,
        &message,
        &result_tree,
        &[&local_commit, &remote_commit],
    )?;

    move_branch_to(repository, branch, merge_commit)
}

pub fn merge(
    repository: &git2::Repository,
    fetch_commit: &git2::AnnotatedCommit,
    repo_config: &crate::git::config::RepositoryConfig,
) -> Result<()> {
    cleanup_unfinished_operations(repository)?;

    let analysis = repository.merge_analysis(&[fetch_commit])?;
    if analysis.0.is_fast_forward() {
        tracing::info!("Sync: Performing a fast-forward merge");
        fast_forward_merge(repository, &repo_config.branch, fetch_commit)?;
    } else if analysis.0.is_normal() {
        tracing::info!("Sync: Performing merge");
        let head_commit = repository.reference_to_annotated_commit(&repository.head()?)?;
        match repo_config.merge_flavor.clone().unwrap_or_default() {
            crate::git::config::MergeFlavor::Rebase => {
                rebase(repository, &repo_config.branch, &head_commit, fetch_commit)?
            }
            crate::git::config::MergeFlavor::MergeCommit => {
                normal_merge(repository, &repo_config.branch, &head_commit, fetch_commit)?
            }
        }
    } else {
        tracing::info!("Sync: Already up-to-date")
    }
    Ok(())
}

pub fn rebase(
    repository: &git2::Repository,
    branch: &str,
    local_commit: &git2::AnnotatedCommit,
    remote_commit: &git2::AnnotatedCommit,
) -> Result<()> {
    tracing::info!("Sync: Performing rebase");
    let merge_base = repository.merge_base(local_commit.id(), remote_commit.id())?;

    // NOTE: the rebase runs in memory, so a failure can't leave the working directory half-way
    // through it
    let mut rebase_opts = git2::RebaseOptions::default();
    rebase_opts.inmemory(true);
    let mut rebase = repository.rebase(
        Some(local_commit),
        Some(remote_commit),
        None,
        Some(&mut rebase_opts),
    )?;

    let signature = repository.signature()?;
    let mut rebased_head = remote_commit.id();
    while let Some(rebase_op) = rebase.next() {
        let rebase_op = rebase_op?;
        let mut index = rebase.inmemory_index()?;
        if index.has_conflicts() {
            resolve_conflicts(
                repository,
                &mut index,
                LocalSide::Theirs,
                rebase_op.id(),
                remote_commit.id(),
                merge_base,
            )?;
        }
        match rebase.commit(None, &signature, None) {
            Ok(commit) => rebased_head = commit,
            // NOTE: nothing left of this commit, the remote already has the same content
            Err(e) if e.code() == git2::ErrorCode::Applied => {}
            Err(e) => return Err(e.into()),
        }
    }
    rebase.finish(None)?;

    move_branch_to(repository, branch, rebased_head)?;

    tracing::info!("Sync: Rebase done");

    Ok(())
}
