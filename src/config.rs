use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[derive(Serialize, Deserialize, Debug, Clone, mlua::FromLua)]
pub struct Override {
    pub from: String,
    pub to: String,
}

#[serde_as]
#[derive(Default, Serialize, Deserialize, Debug, Clone, mlua::FromLua)]
pub struct Config {
    #[serde(default = "default_overrides")]
    pub overrides: HashMap<String, Override>,

    #[serde(default = "default_ignores")]
    pub ignores: Vec<String>,
}
fn default_overrides() -> HashMap<String, Override> {
    HashMap::default()
}

fn default_ignores() -> Vec<String> {
    Vec::default()
}
