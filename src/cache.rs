use std::{fs, path::Path};

use crate::result::Result;

#[derive(Serialize, Deserialize)]
pub struct CachedPackage {
    pub name: String,
    pub path: Path,
}

#[derive(Serialize, Deserialize)]
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

    pub fn cache_package(&mut self, name: String, path: &Path) {
        self.deployed.push(CachedPackage { name, path })
    }
}
