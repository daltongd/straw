use mlua::LuaSerdeExt;

use crate::{config::Config, git::config::RepositoryConfig, result::Result};

pub fn run_git_config(lua: &mlua::Lua) -> Result<RepositoryConfig> {
    let config = lua
        .load(
            r#"
            local straw = require("straw");
            local config = straw.find_module("git")
            -- dump(config)
            return config
        "#,
        )
        .eval()?;

    let config: Option<RepositoryConfig> = lua.from_value(config)?;

    tracing::trace!("Git config: {config:?}");

    Ok(config.unwrap_or_default())
}

pub fn run_lua_config(lua: &mlua::Lua) -> Result<Config> {
    let config_value = lua
        .load(
            r#"
            local straw = require("straw");
            local test = straw.find_module("test")
            local config = straw.find_module("config")
            -- dump(config)
            return config
        "#,
        )
        .eval::<mlua::Value>()?;

    // setting `deny_unsupported_types` to false allows the config to be deserialized if the user
    // defined functions (e.g. `on_apply` callback) within the config table
    let options = mlua::serde::de::Options::default().deny_unsupported_types(false);
    let mut config: Option<Config> = lua.from_value_with(config_value.clone(), options)?;
    if let Some(config) = &mut config {
        config.value = Some(config_value);
    }

    tracing::trace!("{config:?}");

    Ok(config.unwrap_or_default())
}
