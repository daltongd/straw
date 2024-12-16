use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Override {
    pub from: String,
    pub to: String,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub git: Option<crate::git::RepositoryConfig>,

    #[serde_as(as = "Option<Vec<(_, _)>>")]
    pub overrides: Option<HashMap<String, Override>>,

    #[serde_as(as = "Option<Vec<(_, _)>>")]
    pub overrides_windows: Option<HashMap<String, Override>>,

    #[serde_as(as = "Option<Vec<(_, _)>>")]
    pub overrides_unix: Option<HashMap<String, Override>>,

    pub ignores: Option<Vec<String>>,
    pub ignores_windows: Option<Vec<String>>,
    pub ignores_unix: Option<Vec<String>>,
}
