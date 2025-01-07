use crate::{config::Config, git::config::RepositoryConfig, result::Result};

use super::runtime::LuaRuntime;

pub fn run_git_config(runtime: &LuaRuntime) -> Result<RepositoryConfig> {
    let (config, _) = runtime.run::<RepositoryConfig>(
        r#"
            local straw = require("straw");
            local config = straw.find_module("git")
            -- dump(config)
            return config
        "#,
    )?;

    tracing::trace!("Git config: {config:?}");

    Ok(config.unwrap_or_default())
}

pub fn run_lua_config(runtime: &LuaRuntime) -> Result<Config> {
    let (mut config, value) = runtime.run::<Config>(
        r#"
            local straw = require("straw");
            local test = straw.find_module("test")
            local config = straw.find_module("config")
            -- dump(config)
            return config
        "#,
    )?;

    if let Some(config) = &mut config {
        config.value = Some(value);
    }

    tracing::trace!("{config:?}");

    Ok(config.unwrap_or_default())
}
