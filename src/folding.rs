use std::{collections::HashMap, fs::DirEntry, path::Path};

use crate::result::Result;

#[derive(Default, Debug)]
pub struct PackageDirEntry {
    children: HashMap<String, PackageDirEntry>,
    packages: Vec<String>,
}

pub fn fold(dotfiles_path: &Path) -> Result<HashMap<String, PackageDirEntry>> {
    let mut result = HashMap::new();
    let dir = std::fs::read_dir(dotfiles_path)?;

    for entry in dir {
        let entry = entry?;
        let package_name = entry.file_name().to_string_lossy().into_owned();
        if !package_name.starts_with(".") && entry.file_type()?.is_dir() {
            fold_dir(&mut result, &entry, &package_name)?;
        }
    }

    Ok(result)
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
