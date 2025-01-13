use std::path::PathBuf;

use crate::{
    args::Args,
    result::{Result, StrawError},
};

pub const DOTFILES_DIRS: [&str; 3] = [".dotfiles/", "dotfiles/", "dot-dotfiles/"];

// this is not free, but relatively low-cost. would be nice to cache the home_dir somehow though
pub fn get_home_dir() -> Result<String> {
    let home = home::home_dir().ok_or(StrawError::String(
        "Couldn't determine the home directory path.".into(),
    ))?;

    let home_dir = home.as_os_str().to_str().ok_or(StrawError::String(
        "Couldn't convert {home:?} to &str.".into(),
    ))?;

    Ok(home_dir.into())
}

pub fn find_target_dir(args: &Args) -> Result<String> {
    let home_dir = get_home_dir()?;

    if let Some(target_override) = &args.target {
        return Ok(target_override.replace("~", &home_dir).replace("\\", "/"));
    }

    Ok(home_dir)
}

pub fn get_dotfiles_path(args: &Args) -> Result<PathBuf> {
    let target_dir = find_target_dir(args)?;

    // TODO: attempt to use `.dotfiles` or `dotfiles` and check if exists

    let dotfiles_path: PathBuf = if let Some(dotfiles_override) = &args.dotfiles {
        PathBuf::from(dotfiles_override)
    } else {
        let mut result = None;
        for path in DOTFILES_DIRS {
            let path = [&target_dir, path].iter().collect::<PathBuf>();
            if path.exists() {
                result = Some(path);
                break;
            }
        }
        result.unwrap_or([&target_dir, *DOTFILES_DIRS.first().expect("DOTFILES_DIRS should never be empty.")].iter().collect::<PathBuf>())
    };

    Ok(dotfiles_path)
}
