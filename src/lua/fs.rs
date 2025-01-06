use std::{io::Read, path::PathBuf};

use crate::result::StrawError;

pub type Result<T> = std::result::Result<T, mlua::Error>;

pub fn read_file(_lua: &mlua::Lua, path: String) -> Result<Option<String>> {
    match std::fs::File::open(path.replace("~", &crate::utils::get_home_dir()?)) {
        Ok(mut file) => {
            let mut buf = String::new();
            file.read_to_string(&mut buf)?;

            Ok(Some(buf))
        }
        Err(e) => Err(StrawError::String(format!(
            "Could not read '{path}' as string. Reason: {e}"
        ))
        .into()),
    }
}

pub fn get_file_type(_lua: &mlua::Lua, path: String) -> Result<String> {
    match std::fs::metadata(path.replace("~", &crate::utils::get_home_dir()?)) {
        Ok(metadata) => {
            if metadata.is_dir() {
                Ok(String::from("directory"))
            } else if metadata.is_file() {
                Ok(String::from("file"))
            } else if metadata.is_symlink() {
                Ok(String::from("symlink"))
            } else {
                Err(StrawError::String("File type unsupported.".into()).into())
            }
        }
        Err(e) => {
            Err(StrawError::String(format!("Could get file type for '{path}'. Reason: {e}")).into())
        }
    }
}

pub fn exists(_lua: &mlua::Lua, path: String) -> Result<bool> {
    let path = crate::fs::sanitize_path(&path)?;
    let path = path.replace("~", crate::utils::get_home_dir()?.as_str());
    Ok(PathBuf::from(path).exists())
}
