use crate::{config::Config, git::config::RepositoryConfig, result::Result};

use super::runtime::LuaRuntime;

pub fn run_git_config(runtime: &LuaRuntime) -> Result<RepositoryConfig> {
    match runtime.run::<RepositoryConfig>(
        r#"
            local straw = require("straw");
            local config = straw.find_module("git")
            -- dump(config)
            return config
        "#,
    ) {
        Ok((config, _)) => {
            tracing::trace!("Git config: {config:?}");
            Ok(config.unwrap_or_default())
        }
        Err(e) => {
            tracing::error!("Lua Error: {e}");
            Err(crate::result::StrawError::String(
                "Failed to run the git config.".into(),
            ))
        }
    }
}

pub fn run_lua_config(runtime: &LuaRuntime) -> Result<Config> {
    match runtime.run::<Config>(
        r#"
            local straw = require("straw");
            local test = straw.find_module("test")
            local config = straw.find_module("config")
            -- dump(config)
            return config
        "#,
    ) {
        Ok((mut config, value)) => {
            if let Some(config) = &mut config {
                config.value = Some(value);
            }

            tracing::trace!("{config:?}");

            Ok(config.unwrap_or_default())
        }
        Err(e) => {
            tracing::error!("Lua Error: {e}");
            Err(crate::result::StrawError::String(
                "Failed to run the lua config.".into(),
            ))
        }
    }
}
