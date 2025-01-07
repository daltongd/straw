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

    #[serde(skip_deserializing)]
    pub value: Option<mlua::Value>,
}

impl Config {
    pub fn call(&self, function_name: &str, args: impl mlua::IntoLuaMulti) -> Option<mlua::Value> {
        if let Some(value) = &self.value {
            if let Some(config) = value.as_table() {
                if let Ok(function) = config.get::<mlua::Function>(function_name) {
                    return function.call::<mlua::Value>(args).ok();
                }
            }
        }
        None
    }
}

fn default_overrides() -> HashMap<String, Override> {
    HashMap::default()
}

fn default_ignores() -> Vec<String> {
    Vec::default()
}
