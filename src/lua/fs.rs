use std::{
    io::{Read, Write},
    path::PathBuf,
};

use crate::result::StrawError;

pub type Result<T> = std::result::Result<T, mlua::Error>;

pub fn read_text_file(_lua: &mlua::Lua, path: String) -> Result<Option<String>> {
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

pub fn write_text_file(
    _lua: &mlua::Lua,
    (path, content): (String, Option<String>),
) -> Result<usize> {
    match std::fs::File::create(crate::fs::expand_path(&path)?) {
        Ok(mut file) => {
            let bytes = if let Some(content) = content {
                file.write(content.as_bytes())?
            } else {
                0
            };

            Ok(bytes)
        }
        Err(e) => Err(StrawError::String(format!(
            "Could not read '{path}' as string. Reason: {e}"
        ))
        .into()),
    }
}

pub fn read_bin_file(_lua: &mlua::Lua, path: String) -> Result<Option<Vec<u8>>> {
    match std::fs::File::open(path.replace("~", &crate::utils::get_home_dir()?)) {
        Ok(mut file) => {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;

            Ok(Some(buf))
        }
        Err(e) => Err(StrawError::String(format!(
            "Could not read '{path}' as string. Reason: {e}"
        ))
        .into()),
    }
}

pub fn write_bin_file(_lua: &mlua::Lua, (path, bytes): (String, Vec<u8>)) -> Result<usize> {
    let path = std::path::PathBuf::from(crate::fs::expand_path(&path)?);
    match std::fs::File::create(&path) {
        Ok(mut file) => Ok(file.write(&bytes)?),
        Err(e) => Err(StrawError::String(format!(
            "Could not write to file '{}'. Reason: {e}",
            path.display()
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
    let path = crate::fs::expand_path(&path)?;
    Ok(PathBuf::from(path).exists())
}

pub fn remove_file(_lua: &mlua::Lua, path: String) -> Result<()> {
    let path = crate::fs::expand_path(&path)?;
    std::fs::remove_file(path)?;
    Ok(())
}

pub fn create_dir(_lua: &mlua::Lua, (path, create_parents): (String, bool)) -> Result<()> {
    let path = crate::fs::expand_path(&path)?;
    if create_parents {
        std::fs::create_dir_all(&path)?;
    } else {
        std::fs::create_dir(&path)?;
    }
    Ok(())
}

pub fn remove_dir(_lua: &mlua::Lua, (path, remove_files): (String, bool)) -> Result<()> {
    let path = crate::fs::expand_path(&path)?;
    if remove_files {
        std::fs::remove_dir_all(&path)?;
    } else {
        std::fs::remove_dir(&path)?;
    }
    Ok(())
}

pub fn create_symlink(_lua: &mlua::Lua, (path, target): (String, String)) -> Result<()> {
    let path = crate::fs::expand_path(&path)?;
    let target = crate::fs::expand_path(&target)?;
    crate::fs::create_symlink(false, &PathBuf::from(&path), &PathBuf::from(&target))?;
    Ok(())
}

// pub fn remove_synlink(_lua: &mlua::Lua, path: String) -> Result<()> {
//     let path = crate::fs::expand_path(&path)?;
//         crate::fs::remove_symlink_if_exists(&path)?;
//     Ok(())
// }
// TODO: mkdir
// TODO: create symlink
