use std::path::PathBuf;

use crate::{
    args::Args,
    result::{Result, StrawError},
};

pub const DOTFILES_DIR: &str = ".dotfiles/";

pub fn find_target_dir(args: &Args) -> Result<String> {
    let home = home::home_dir().ok_or(StrawError::String(
        "Couldn't determine the home directory path.".into(),
    ))?;
    let home_dir = home.as_os_str().to_str().ok_or(StrawError::String(
        "Couldn't convert {home:?} to &str.".into(),
    ))?;

    let display_home = home_dir.replace("\\", "/");
    tracing::info!("Using target directory: {display_home}");

    if let Some(target_override) = &args.target {
        return Ok(target_override.replace("~", home_dir).replace("\\", "/"));
    }

    Ok(home_dir.into())
}

pub fn get_dotfiles_path(args: &Args) -> Result<PathBuf> {
    let target_dir = find_target_dir(args)?;

    // TODO: attempt to use `.dotfiles` or `dotfiles` and check if exists

    let dotfiles_path: PathBuf = if let Some(dotfiles_override) = &args.dotfiles {
        PathBuf::from(dotfiles_override)
    } else {
        let path = [&target_dir, DOTFILES_DIR].iter().collect::<PathBuf>();
        // if !path.exists() {
        //     return [
        //         &target_dir,
        //         DOTFILES_DIR.strip_prefix(".").unwrap_or_default(),
        //     ]
        //     .iter()
        //     .collect();
        // }
        path
    };

    Ok(dotfiles_path)
}
