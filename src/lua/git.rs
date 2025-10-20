use std::path::PathBuf;

use clap::Parser;

use crate::{
    args::Args,
    git::config::{MergeFlavor, RepositoryConfig},
    result::StrawError,
};

use super::macros::function;

function! {
    clone(url: String, target_path: String) {
        let args = Args::parse();
        let target_path = crate::fs::expand_path(&target_path)?;
        crate::git::ops::clone(&args, &url, &PathBuf::from(&target_path))?;
    }
}

function! {
    init(path: String, origin_url: Option<String>) {
        let target_path = crate::fs::expand_path(&path)?;
        crate::git::ops::init(&PathBuf::from(&target_path), origin_url)?;
    }
}

function! {
    commit(path: String, message: String) {
        let args = Args::parse();
        let target_path = crate::fs::expand_path(&path)?;
        crate::git::ops::commit_changes(&args, &PathBuf::from(&target_path), &RepositoryConfig { remote: String::default(), branch: String::default(), merge_flavor: None, commit_message: Some(message), ssh_private_key: args.ssh_private_key.clone() })?;
    }
}

function! {
    status(path: String) -> Vec<String> {
        let target_path = crate::fs::expand_path(&path)?;
        let statuses = crate::git::status::get_status(&PathBuf::from(&target_path))?;
        let mut out_statuses = Vec::default();
        for status in statuses {
            out_statuses.push(status.to_string());
        }
        Ok(out_statuses)
    }
}

function! {
    pull(path: String, remote_name: String, branch: String, merge_flavor: MergeFlavor) {
        let args = Args::parse();
        let repository = git2::Repository::open(path).map_err(StrawError::Git)?;
        let mut remote = repository.find_remote(&remote_name).map_err(StrawError::Git)?;
        let head = crate::git::ops::fetch(&args, &repository, &mut remote, &[&branch], 1)?;

        let repo_config = RepositoryConfig { remote: remote_name, branch, merge_flavor: Some(merge_flavor), commit_message: None, ssh_private_key: args.ssh_private_key };
        crate::git::merge::merge(&repository, &head, &repo_config)?;
    }
}

function! {
    push(path: String, remote: String, branch: String) {
        let args = Args::parse();
        let repo_config = RepositoryConfig { remote, branch, merge_flavor: None, commit_message: None, ssh_private_key: args.ssh_private_key.clone() };
        crate::git::ops::push_changes(&args, &PathBuf::from(path), &repo_config)?;
    }
}
