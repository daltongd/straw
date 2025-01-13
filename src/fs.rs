use std::path::{Path, PathBuf};

use lazy_regex::{Captures, Regex};

use crate::{
    result::{Result, StrawError},
    state::State,
};

#[cfg(windows)]
pub static ENV_VAR_REGEX: lazy_regex::Lazy<Regex> = lazy_regex::lazy_regex!(r"%(\w+)%");
#[cfg(unix)]
pub static ENV_VAR_REGEX: lazy_regex::Lazy<Regex> = lazy_regex::lazy_regex!(r"\$(\w+)");

pub fn expand_path(path: &str) -> Result<String> {
    let path = ENV_VAR_REGEX
        .replace_all(path, |captures: &Captures| {
            if let Ok(value) = std::env::var(&captures[1]) {
                value
            } else {
                tracing::error!("Couldn't resolve env var: '{}'", &captures[0]);
                String::from(&captures[0])
            }
        })
        .into_owned();

    let home_dir = crate::utils::get_home_dir()?;
    let path = path.replace("~", &home_dir);

    Ok(path)
}

pub fn sanitize_path(path: &str) -> Result<String> {
    // there no guarantees about `home_dir` or `path` so we have to perform
    // the `replace("\\", "/")` on both paths before replacing `home_dir` with `~`
    let home_dir = crate::utils::get_home_dir()?.replace("\\", "/");
    Ok(path.replace("\\", "/").replace(&home_dir, "~"))
}

pub fn path_to_string(path: &Path) -> Result<String> {
    sanitize_path(&path.display().to_string())
}

pub fn create_symlink(preview: bool, target_path: &PathBuf, link_path: &PathBuf) -> Result<()> {
    let link_string = path_to_string(link_path)?;
    let target_string = path_to_string(target_path)?;
    if target_path.is_dir() {
        if !preview {
            #[cfg(windows)]
            std::os::windows::fs::symlink_dir(target_path, link_path)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(target_path, link_path)?;
        }
        tracing::info!("Created a directory link:\t\t{link_string} -> {target_string}");
    } else if target_path.is_file() {
        if !preview {
            #[cfg(windows)]
            std::os::windows::fs::symlink_file(target_path, link_path)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(target_path, link_path)?;
        }
        tracing::info!("Created a file link:\t\t{link_string} -> {target_string}");
    } else if target_path.is_symlink() {
        return Err(StrawError::String(
            "Linking symlink to a symlink is not supported.".into(),
        ));
    } else {
        return Err(StrawError::String(format!("Invalid path {link_string}")));
    }

    Ok(())
}

pub fn remove_symlink_if_exists(state: &State, link_path: &PathBuf) -> Result<()> {
    if link_path.exists() && link_path.is_symlink() {
        let display_path = path_to_string(link_path)?;
        tracing::info!("Removing the existing link:\t{display_path}");
        if !state.args.preview {
            if link_path.is_dir() {
                std::fs::remove_dir_all(link_path)?;
            } else if link_path.is_file() {
                std::fs::remove_file(link_path)?;
            }
        }
    }

    Ok(())
}

pub fn copy_dir_all(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for dir_entry in std::fs::read_dir(from)? {
        let entry = dir_entry?;
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &to.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), to.join(entry.file_name()))?;
        }
    }
    Ok(())
}
