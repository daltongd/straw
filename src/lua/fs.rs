use std::{
    io::{Read, Write},
    path::PathBuf,
};

use crate::lua::macros::function;
use crate::result::StrawError;

function! {
    read_text_file(path: String) -> Option<String> {
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
}

function! {
    write_text_file(path: String, content: Option<String>) -> usize {
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
}

function! {
    read_bin_file(path: String) -> Option<Vec<u8>> {
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
}

function! {
    write_bin_file(path: String, bytes: Vec<u8>) -> usize {
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
}

function! {
    get_file_type(path: String) -> String {
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
}

function! {
    exists(path: String) -> bool {
        let path = crate::fs::expand_path(&path)?;
        Ok(PathBuf::from(path).exists())
    }
}

function! {
    remove_file(path: String) {
        let path = crate::fs::expand_path(&path)?;
        std::fs::remove_file(path)?;
    }
}

function! {
    create_dir(path: String, create_parents: bool) {
        let path = crate::fs::expand_path(&path)?;
        if create_parents {
            std::fs::create_dir_all(&path)?;
        } else {
            std::fs::create_dir(&path)?;
        }
    }
}

function! {
    remove_dir(path: String, remove_files: bool) {
        let path = crate::fs::expand_path(&path)?;
        if remove_files {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_dir(&path)?;
        }
    }
}

function! {
    create_symlink(path: String, target: String) {
        let path = crate::fs::expand_path(&path)?;
        let target = crate::fs::expand_path(&target)?;
        crate::fs::create_symlink(false, &PathBuf::from(&path), &PathBuf::from(&target))?;
    }
}
