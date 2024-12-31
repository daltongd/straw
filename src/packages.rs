use std::{
    fs::ReadDir,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::{
    args::Args,
    cache::Cache,
    result::{Result, StrawError},
    state::StateInfo,
};
use regex::Captures;

fn fold_and_apply(
    cache: &mut Cache,
    name: &str,
    info: &StateInfo,
    from_dir: ReadDir,
    to: PathBuf,
) -> Result<()> {
    for item in from_dir {
        let item = item?;
        let dir = std::fs::read_dir(&to)?;
        let mut found_existing = false;
        for existing in dir {
            let existing = existing?;
            if item.file_name() == existing.file_name() && !existing.file_type()?.is_symlink() {
                if existing.file_type()?.is_dir() {
                    let file_name = item.file_name();
                    let item_name = file_name.to_str().ok_or(StrawError::String(
                        "Can't convert '{item_name:?}' to str.".into(),
                    ))?;
                    let to: PathBuf = PathBuf::from(&to).join(item_name);
                    let display_path = crate::fs::path_to_string(&existing.path())?;
                    tracing::trace!("{name}: {display_path} exists");
                    let dir = std::fs::read_dir(item.path())?;
                    fold_and_apply(cache, name, info, dir, to)?;
                }
                found_existing = true;
            }
        }

        if !found_existing {
            let from = item.path();
            let to = to.join(item.file_name());

            tracing::trace!("{name}: Creating symlink");

            crate::fs::remove_symlink_if_exists(info, &to)?;
            crate::fs::create_symlink(info.args.preview, &from, &to.clone())?;
            cache.cache_package(name, &to);
        }
    }

    Ok(())
}

fn apply_override(cache: &mut Cache, name: &str, info: &StateInfo) -> Result<()> {
    let path_override = &info.config.overrides[name];

    let from = path_override.from.clone();
    let to = path_override.to.clone();

    // NOTE: replace the environment variables in paths `%env_var%` on windows or `$env_var`
    // on unix systems
    let to = info
        .env_var_regex
        .replace_all(&to, |captures: &Captures| {
            if let Ok(value) = std::env::var(&captures[1]) {
                value
            } else {
                tracing::error!("Couldn't resolve '{}' env var.", &captures[0]);
                String::from(&captures[0])
            }
        })
        .into_owned();
    let to = to.replace("~", info.target_dir);

    let display_from = crate::fs::path_to_string(&PathBuf::from(&from))?;
    let display_to = crate::fs::path_to_string(&PathBuf::from(&to))?;
    tracing::info!("Found override:\t\t\t{display_from} -> {display_to}");

    let to = PathBuf::from_str(to.as_str())
        .map_err(|e| StrawError::String(format!("Can't resolve path: {to:?}. {e:?}")))?;

    crate::fs::remove_symlink_if_exists(info, &to)?;

    if to.exists() && !to.is_symlink() {
        let to = to.display().to_string().replace("\\", "/");
        tracing::warn!("{to} already exists, ignoring");
    }

    // could be false if the file wasn't successfully removed
    if !to.exists() || info.args.preview {
        // check if the parent path exists
        if let Some(parent) = to.parent() {
            if !parent.exists() {
                crate::fs::create_dir(parent)?;
            }
        } // else - `to` is root, ignore

        let from_path: PathBuf = crate::utils::get_dotfiles_path(info.args)?;
        crate::fs::create_symlink(info.args.preview, &from_path, &to.clone())?;
        cache.cache_package(name, &to);
    }

    Ok(())
}

fn apply_package(cache: &mut Cache, name: &str, info: &StateInfo) -> Result<()> {
    let package_path = &info.dotfiles_path;
    let package_path = package_path.join(name);

    let package_dir = std::fs::read_dir(&package_path)?;

    tracing::trace!("Applying package: {name}");
    let has_packages = !info.args.packages.is_empty();
    if (!has_packages && info.config.ignores.contains(&name.into()))
        || info.args.ignore.contains(&name.into())
    {
        tracing::trace!("Ignoring {name}");
        return Ok(());
    }

    let deployed_links_for_package = cache.find_deployed_links(name);
    let deployed_links = cache.get_deployed_packages_mut();
    for (i, link) in deployed_links_for_package.iter().rev() {
        crate::fs::remove_symlink_if_exists(info, &PathBuf::from(link))?;
        deployed_links.remove(*i);
    }

    if info.config.overrides.contains_key(name) {
        apply_override(cache, name, info)?;
    } else {
        fold_and_apply(
            cache,
            name,
            info,
            package_dir,
            PathBuf::from(info.target_dir),
        )?;
    }

    Ok(())
}

pub fn apply_packages(cache: &mut Cache, packages: &Vec<String>, info: &StateInfo) -> Result<()> {
    for package in packages {
        apply_package(cache, package, info)?;
    }
    Ok(())
}

pub fn clear_deployed_packages(cache: &mut Cache, info: &StateInfo) -> Result<()> {
    for package in cache.get_deployed_packages() {
        crate::fs::remove_symlink_if_exists(info, &PathBuf::from(&package.path))?;
    }
    cache.clear();

    Ok(())
}

pub fn adopt(args: &Args, path: &str, target_dir: &str, dotfiles_path: &Path) -> Result<()> {
    let path = crate::fs::sanitize_path(path)?;
    let source_path = PathBuf::from(&path);
    if let Some(dir_name) = source_path.file_stem().unwrap_or_default().to_str() {
        let relative_to_target = source_path.strip_prefix(target_dir).map_err(|_| {
            StrawError::String("Adopt: only paths under the target directory are supported".into())
        })?;

        let package_name = if let Some(dir_name_stripped) = dir_name.strip_prefix(".") {
            dir_name_stripped
        } else {
            dir_name
        };

        if dotfiles_path.join(package_name).exists() {
            return Err(StrawError::String(format!(
                "Adopt: package named {package_name} already exists."
            )));
        }

        let target_path = dotfiles_path.join(package_name).join(relative_to_target);

        let display_target_path = crate::fs::path_to_string(&target_path)?;
        tracing::info!("Adopt: adopting {path} into {display_target_path}");

        if !args.preview {
            if let Some(parent_dir) = target_path.parent() {
                if !parent_dir.exists() {
                    std::fs::create_dir_all(parent_dir)?;
                }
            }
            std::fs::rename(&source_path, &target_path)?;
        }

        // `target_path` becomes `from` and `source_path` becomes `to`, since we just did the rename
        crate::fs::create_symlink(args.preview, &target_path, &source_path)?;

        tracing::info!("Adopt: successfully adopted {:?}", package_name);
    } else {
        return Err(StrawError::String(
            "Adopt: unknown error. Couldn't determine the package name.".into(),
        ));
    }

    Ok(())
}
