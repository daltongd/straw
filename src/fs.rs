use std::path::{Path, PathBuf};

use crate::{
    result::{Result, StrawError},
    state::StateInfo,
};

pub fn path_to_string(path: &Path, home_dir: &str) -> String {
    path.display()
        .to_string()
        .replace("\\", "/")
        .replace(home_dir, "~")
}

pub fn create_symlink(info: &StateInfo, from: &PathBuf, to: &PathBuf) -> Result<()> {
    let display_from = path_to_string(from, info.home_dir);
    let display_to = path_to_string(to, info.home_dir);
    if from.is_dir() {
        if !info.args.preview {
            #[cfg(windows)]
            std::os::windows::fs::symlink_dir(from, to)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(from, to)?;
        }
        tracing::info!("Created a directory link:\t\t{display_from} -> {display_to}");
    } else if from.is_file() {
        if !info.args.preview {
            #[cfg(windows)]
            std::os::windows::fs::symlink_file(from, to)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(from, to)?;
        }
        tracing::info!("Created a file link:\t\t{display_from} -> {display_to}");
    } else if from.is_symlink() {
        return Err(StrawError::String(
            "Linking symlink to a symlink is not supported at the moment.".into(),
        ));
    } else {
        return Err(StrawError::String("Invalid path {from_path:?}".into()));
    }

    Ok(())
}

pub fn remove_symlink_if_exists(info: &StateInfo, link_path: &PathBuf) -> Result<()> {
    if link_path.exists() && link_path.is_symlink() {
        let display_path = path_to_string(link_path, info.home_dir);
        tracing::info!("Removing the existing link:\t{display_path}");
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

pub fn create_dir(dir: &Path) -> Result<()> {
    if let Some(parent) = dir.parent() {
        if !parent.exists() {
            return create_dir(parent);
        }
    }

    std::fs::create_dir(dir).map_err(StrawError::Io)
}
