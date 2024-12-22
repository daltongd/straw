use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::result::Result;

#[derive(Serialize, Deserialize, Debug)]
pub struct CachedPackage {
    pub name: String,
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Cache {
    deployed: Vec<CachedPackage>,
}

impl Cache {
    pub fn save(&self, path: &Path) -> Result<()> {
        let toml = toml::to_string(self)?;
        fs::write(path, toml)?;
        Ok(())
    }

    pub fn read(path: &Path) -> Result<Self> {
        let toml = fs::read_to_string(path)?;
        let cache: Cache = toml::from_str(&toml)?;
        Ok(cache)
    }

    pub fn cache_package(&mut self, name: &str, path: &Path) {
        let path = path.to_string_lossy().to_string().replace("\\", "/");
        self.deployed.push(CachedPackage {
            name: name.to_owned(),
            path,
        })
    }
}
