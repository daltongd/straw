use crate::result::{Result, StrawError};

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
    repo_config: &crate::git::config::RepositoryConfig,
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
            crate::git::config::MergeFlavor::Rebase => {
                rebase(repository, &head_commit, fetch_commit)?
            }
            crate::git::config::MergeFlavor::MergeCommit => {
                normal_merge(repository, &head_commit, fetch_commit)?
            }
        }
    } else {
        tracing::info!("Sync: Already up-to-date")
    }
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
