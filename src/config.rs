use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;

// callbacks are registered as string consts, this macro is here to prevent spelling mistakes
// the name saved in the const is later converted `.to_lowercase()` within `Config::call`
macro_rules! register_callback {
    ($i: ident) => {
        pub const $i: &str = stringify!($i);
    };
}

// NOTE: `register_callback!` will convert `CALLBACK_NAME` to
// pub const CALLBACK_NAME = "CALLBACK_NAME";
// see macro declaration above for more details
pub struct LuaCallbacks;
impl LuaCallbacks {
    register_callback!(ON_APPLY);
    register_callback!(ON_FILE_CREATED);
}

#[derive(Serialize, Deserialize, Debug, Clone, mlua::FromLua)]
pub struct Override {
    pub link: String,
    pub target: String,
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
        let function_name = function_name.to_lowercase();
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
