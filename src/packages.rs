use std::{
    fs::ReadDir,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::{
    args::Args,
    cache::Cache,
    config::LuaCallbacks,
    result::{Result, StrawError},
    state::State,
};

pub fn copy_files(
    runtime: &crate::lua::runtime::LuaRuntime,
    package: &str,
    dotfiles_path: &Path,
    preview: bool,
    overwrite: bool,
) -> Result<()> {
    let home_dir = crate::utils::get_home_dir()?;
    let config = crate::lua::run_lua_config(runtime)?;
    if let Some(path_override) = &config.overrides.get(package) {
        let from = dotfiles_path.join(crate::fs::expand_path(&path_override.target)?);
        let to = PathBuf::from(crate::fs::expand_path(&path_override.link)?);
        tracing::info!(
            "Found override: {} -> {}",
            crate::fs::display_path(&from)?,
            crate::fs::display_path(&to)?,
        );
        crate::fs::copy_dir_all(&from, &to, preview, overwrite)?;
        Ok(())
    } else {
        let from = dotfiles_path.join(package);
        let to = PathBuf::from(home_dir);
        tracing::info!(
            "Copying: {} -> {}",
            crate::fs::display_path(&from)?,
            crate::fs::display_path(&to)?,
        );
        crate::fs::copy_dir_all(&from, &to, preview, overwrite)?;
        Ok(())
    }
}

fn fold_and_apply(
    cache: &mut Cache,
    name: &str,
    state: &State,
    target_dir: ReadDir,
    link_path: PathBuf,
) -> Result<()> {
    for item in target_dir {
        let item = item?;
        tracing::trace!("link_path: {}", link_path.display());
        let dir = std::fs::read_dir(&link_path)?;
        let mut found_existing = false;
        for existing in dir {
            let existing = existing?;
            if item.file_name() == existing.file_name() && !existing.file_type()?.is_symlink() {
                if existing.file_type()?.is_dir() {
                    let file_name = item.file_name();
                    let item_name = file_name.to_str().ok_or(StrawError::String(
                        "Can't convert '{item_name:?}' to str.".into(),
                    ))?;
                    let to: PathBuf = PathBuf::from(&link_path).join(item_name);
                    let display_path = crate::fs::path_to_string(&existing.path())?;
                    tracing::trace!("{name}: {display_path} exists");
                    let dir = std::fs::read_dir(item.path())?;
                    fold_and_apply(cache, name, state, dir, to)?;
                }
                found_existing = true;
            }
        }

        if !found_existing {
            let target_path = item.path();
            let link_path = link_path.join(item.file_name());

            tracing::trace!("{name}: Creating symlink");

            crate::fs::remove_symlink_if_exists(&link_path, state.args.preview)?;
            crate::fs::create_symlink(&target_path, &link_path.clone(), state.args.preview)?;
            cache.cache_package(name, &link_path);
            state.config.call(
                LuaCallbacks::ON_APPLY,
                (
                    name,
                    crate::fs::path_to_string(&link_path)?,
                    crate::fs::path_to_string(&target_path)?,
                ),
            );
        }
    }

    Ok(())
}

fn apply_override(cache: &mut Cache, name: &str, state: &State) -> Result<()> {
    let path_override = &state.config.overrides[name];

    let target_string = path_override.target.clone();
    let link_string = path_override.link.clone();

    // NOTE: replace the environment variables in paths `%env_var%` on windows or `$env_var`
    // on unix systems
    let link_path = crate::fs::expand_path(&link_string)?;

    let target_string = crate::fs::sanitize_path(&target_string)?;
    let link_string = crate::fs::sanitize_path(&link_path)?;
    tracing::info!("Found override:\t\t\t{link_string} -> {target_string}");

    let link_path = PathBuf::from_str(link_path.as_str())
        .map_err(|e| StrawError::String(format!("Can't resolve path: {link_path:?}. {e:?}")))?;

    crate::fs::remove_symlink_if_exists(&link_path, state.args.preview)?;

    if link_path.exists() && !link_path.is_symlink() {
        let link_path = link_path.display().to_string().replace("\\", "/");
        tracing::warn!("{link_path} already exists, ignoring");
    }

    // could be false if the file wasn't successfully removed
    if !link_path.exists() || state.args.preview {
        // check if the parent path exists
        if let Some(parent) = link_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        } // else - `to` is root, ignore

        let target_path: PathBuf = crate::utils::get_dotfiles_path(state.args)?
            .join(name)
            .join(&target_string);
        crate::fs::create_symlink(&target_path, &link_path.clone(), state.args.preview)?;
        cache.cache_package(name, &link_path);
    }
    state
        .config
        .call(LuaCallbacks::ON_APPLY, (name, link_string, target_string));

    Ok(())
}

fn apply_package(cache: &mut Cache, name: &str, state: &State) -> Result<()> {
    let package_path = &state.dotfiles_path;
    let package_path = package_path.join(name);

    let package_dir = std::fs::read_dir(&package_path)?;

    tracing::trace!("Applying package: {name}");
    let has_packages = !state.args.packages.is_empty();
    if (!has_packages && state.config.ignores.contains(&name.into()))
        || state.args.ignore.contains(&name.into())
    {
        tracing::trace!("Ignoring {name}");
        return Ok(());
    }

    let deployed_links_for_package = cache.find_deployed_links(name);
    let deployed_links = cache.get_deployed_packages_mut();
    for (i, link) in deployed_links_for_package.iter().rev() {
        crate::fs::remove_symlink_if_exists(&PathBuf::from(link), state.args.preview)?;
        deployed_links.remove(*i);
    }

    if state.config.overrides.contains_key(name) {
        apply_override(cache, name, state)?;
    } else {
        fold_and_apply(
            cache,
            name,
            state,
            package_dir,
            PathBuf::from(state.target_dir),
        )?;
    }

    Ok(())
}

pub fn apply_packages(cache: &mut Cache, packages: &Vec<String>, state: &State) -> Result<()> {
    for package in packages {
        apply_package(cache, package, state)?;
    }
    Ok(())
}

pub fn clear_deployed_packages(cache: &mut Cache, preview: bool) -> Result<()> {
    for package in cache.get_deployed_packages() {
        crate::fs::remove_symlink_if_exists(&PathBuf::from(&package.path), preview)?;
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
        crate::fs::create_symlink(&target_path, &source_path, args.preview)?;

        tracing::info!("Adopt: successfully adopted {:?}", package_name);
    } else {
        return Err(StrawError::String(
            "Adopt: unknown error. Couldn't determine the package name.".into(),
        ));
    }

    Ok(())
}
