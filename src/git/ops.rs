use crate::args::Args;
use crate::result::{Result, StrawError};
use git2::BranchType;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

pub use crate::git::status::get_status;

/// Resolves the configured ssh key to an absolute path, ignoring (with a warning) keys that don't
/// exist so they don't break authentication with the default keys / ssh config.
fn ssh_key_path(args: &Args) -> Option<String> {
    let ssh_key = args.ssh_private_key.as_ref()?;
    let path = crate::fs::expand_path(ssh_key).ok()?;
    if Path::new(&path).is_file() {
        Some(path)
    } else {
        tracing::warn!("SSH: private key {ssh_key:?} doesn't exist, ignoring it");
        None
    }
}

fn add_ssh_keys(
    args: &Args,
    auth: auth_git2::GitAuthenticator,
) -> Result<auth_git2::GitAuthenticator> {
    let auth = auth.add_default_ssh_keys();
    if let Some(ssh_key_path) = ssh_key_path(args) {
        return Ok(auth.add_ssh_key_from_file(ssh_key_path, None));
    }
    Ok(auth)
}

/// Network operations (clone/fetch/push) go through the system `git` when it's available. It uses
/// OpenSSH, so `~/.ssh/config`, `known_hosts`, ssh-agent and hardware (`*-sk`) keys all work.
/// libgit2's libssh2 is only a fallback: on Windows it can't negotiate ed25519/ecdsa host keys
/// ("failed to set hostkey preference") and it doesn't support `*-sk` keys anywhere.
fn git_cli_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let available = Command::new("git")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !available {
            tracing::warn!("Git: `git` executable not found, falling back to libgit2");
        }
        available
    })
}

fn run_git<I, S>(args: &Args, git_args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new("git");
    command.args(git_args);
    if let Some(ssh_key_path) = ssh_key_path(args) {
        // don't override the user's own ssh command
        if std::env::var_os("GIT_SSH_COMMAND").is_none() && std::env::var_os("GIT_SSH").is_none()
        {
            command.env(
                "GIT_SSH_COMMAND",
                format!("ssh -i \"{}\"", ssh_key_path.replace("\\", "/")),
            );
        }
    }
    tracing::trace!("Git: running {command:?}");
    let status = command.status()?;
    if !status.success() {
        return Err(StrawError::String(format!(
            "Git: {command:?} failed with {status}"
        )));
    }
    Ok(())
}

pub fn clone(args: &Args, url: &str, target_path: &Path) -> Result<()> {
    let display_path = crate::fs::path_to_string(target_path)?;
    tracing::info!("Clone: cloning {url} into {display_path}");

    if !args.preview && git_cli_available() {
        run_git(args, [OsStr::new("clone"), OsStr::new(url), target_path.as_os_str()])?;
    } else if !args.preview {
        let auth = auth_git2::GitAuthenticator::default();
        let auth = add_ssh_keys(args, auth)?;
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
    args: &Args,
    repository: &'a git2::Repository,
    remote: &'a mut git2::Remote,
    refs: &[&str],
) -> Result<git2::AnnotatedCommit<'a>> {
    if git_cli_available() {
        let remote_name = remote.name().unwrap_or("origin");
        tracing::info!("Sync: Fetching {remote_name} for .dotfiles");
        let mut git_args: Vec<&OsStr> = vec![
            OsStr::new("--git-dir"),
            repository.path().as_os_str(),
            OsStr::new("fetch"),
            OsStr::new("--tags"),
        ];
        // NOTE: see below, merging and rebasing need the full shared history
        if repository.is_shallow() {
            git_args.push(OsStr::new("--unshallow"));
        }
        git_args.push(OsStr::new(remote_name));
        git_args.extend(refs.iter().map(OsStr::new));
        run_git(args, git_args)?;

        let fetch_head = repository.find_reference("FETCH_HEAD")?;
        return Ok(repository.reference_to_annotated_commit(&fetch_head)?);
    }

    // git sync
    let auth = auth_git2::GitAuthenticator::default();
    let auth = add_ssh_keys(args, auth)?;

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
    // NOTE: a shallow fetch cuts the history both sides share, which merging and rebasing need
    // to find their common base. Repositories made shallow by earlier versions get unshallowed.
    if repository.is_shallow() {
        fo.depth(i32::MAX);
    }
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

    let refspec = format!("refs/heads/{}", repo_config.branch);
    if git_cli_available() {
        run_git(
            args,
            [
                OsStr::new("--git-dir"),
                repository.path().as_os_str(),
                OsStr::new("push"),
                OsStr::new(&repo_config.remote),
                OsStr::new(&refspec),
            ],
        )?;
        return Ok(true);
    }

    let auth = auth_git2::GitAuthenticator::default();
    let auth = add_ssh_keys(args, auth)?;

    auth.push(
        &repository,
        &mut remote,
        &[&refspec],
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
    let head = fetch(args, &repository, &mut remote, &[&repo_config.branch])?;
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

    tracing::info!(
        "Patch: applying patch (remote path '{remote_patch_path_string}' from repository {url}) into local path '{temp_path_string}'"
    );

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

        tracing::info!(
            "Patch: cloning '{remote_patch_path_string}' from {url} into '{temp_path_string}'"
        );
        if git_cli_available() {
            run_git(
                args,
                [
                    OsStr::new("clone"),
                    OsStr::new("--depth=1"),
                    OsStr::new(url),
                    temp_path.as_os_str(),
                ],
            )?;
        } else {
            clone_patch_libgit2(args, url, remote_patch_path, &temp_path)?;
        }

        tracing::info!("Patch: applying patch to '{patch_target_string}'");
        let from = temp_path.join(remote_patch_path);
        if from.is_dir() {
            std::fs::rename(temp_path.join(remote_patch_path), &patch_target)?;
        } else {
            if !patch_target.exists() {
                std::fs::create_dir_all(&patch_target)?;
            }
            tracing::trace!(
                "Patch: copying '{}' -> '{}'",
                crate::fs::path_to_string(&from)?,
                crate::fs::path_to_string(&patch_target.join(remote_patch_path_string))?,
            );
            std::fs::rename(&from, patch_target.join(remote_patch_path))?;
        }
        std::fs::remove_dir_all(temp_root)?;
    }
    Ok(())
}

fn clone_patch_libgit2(
    args: &Args,
    url: &str,
    remote_patch_path: &Path,
    temp_path: &Path,
) -> Result<()> {
    let auth = auth_git2::GitAuthenticator::default();
    let auth = add_ssh_keys(args, auth)?;

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

    repo_builder.clone(url, temp_path)?;
    Ok(())
}
