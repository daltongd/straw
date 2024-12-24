use std::{
    collections::HashMap,
    fs::DirEntry,
    path::{Path, PathBuf},
};

use crate::{result::Result, state::StateInfo};

#[derive(Default, Debug)]
pub struct PackageDirEntry {
    children: HashMap<String, PackageDirEntry>,
    packages: Vec<String>,
}

pub fn fold(dotfiles_path: &Path, info: &StateInfo) -> Result<Vec<PathBuf>> {
    let mut map = HashMap::new();
    let dir = std::fs::read_dir(dotfiles_path)?;

    for entry in dir {
        let entry = entry?;
        let package_name = entry.file_name().to_string_lossy().into_owned();
        if !package_name.starts_with(".")
            && !info.config.ignores.contains(&package_name)
            && entry.file_type()?.is_dir()
        {
            fold_dir(&mut map, &entry, &package_name)?;
        }
    }

    let paths = find_common_paths(&map, dotfiles_path.to_path_buf())?;

    Ok(paths)
}

fn fold_dir(
    map: &mut HashMap<String, PackageDirEntry>,
    dir_entry: &DirEntry,
    package_name: &str,
) -> Result<()> {
    let dir = std::fs::read_dir(dir_entry.path())?;

    for entry in dir {
        let entry = entry?;
        let dir_name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_dir() {
            let package_dir_entry = map.entry(dir_name.clone()).or_default();
            fold_dir(&mut package_dir_entry.children, &entry, package_name)?;
            package_dir_entry.packages.push(package_name.into());
        }
    }

    Ok(())
}

fn extract_paths(
    entry: &PackageDirEntry,
    current_path: PathBuf,
    paths: &mut Vec<PathBuf>,
) -> Result<()> {
    if entry.packages.len() > 1 {
        for (dir, entry) in &entry.children {
            extract_paths(entry, current_path.join(dir), paths)?;
        }
        paths.push(current_path);
    }

    Ok(())
}

fn find_common_paths(
    map: &HashMap<String, PackageDirEntry>,
    dotfiles_path: PathBuf,
) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let mut parent_path = dotfiles_path;
    parent_path.pop();

    for (dir, entry) in map {
        let current_path =
            PathBuf::from(parent_path.to_string_lossy().into_owned().as_str()).join(dir);
        extract_paths(entry, current_path, &mut paths)?;
    }

    Ok(paths)
}
