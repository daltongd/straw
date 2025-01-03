use std::path::{Path, PathBuf};

use mlua::LuaSerdeExt;

use crate::{
    args::Args,
    config::Config,
    git::config::RepositoryConfig,
    result::{Result, StrawError},
};

fn create_fs_submodule(lua: &mlua::Lua, args: &Args) -> Result<mlua::Table> {
    let fs = lua.create_table()?;

    let paths = lua.create_table()?;
    fs.set("paths", &paths)?;
    let home_dir = crate::utils::get_home_dir()?;
    paths.set("home", crate::fs::sanitize_path(&home_dir)?)?;

    let dotfiles_dir = crate::utils::get_dotfiles_path(args)?;
    if let Some(dotfiles_dir) = dotfiles_dir.to_str() {
        paths.set("dotfiles", crate::fs::sanitize_path(dotfiles_dir)?)?;
    }

    fs.set(
        "exists",
        lua.create_function(move |_, path: String| {
            let path =
                crate::fs::sanitize_path(&path).map_err(|e| mlua::Error::runtime(e.to_string()))?;
            let path = path.replace("~", home_dir.as_str());
            Ok(PathBuf::from(path).exists())
        })?,
    )?;

    Ok(fs)
}

fn create_straw_module(lua: &mlua::Lua, args: &Args) -> Result<mlua::Table> {
    let module = get_or_create_module(lua, "straw")?;

    let inspect: mlua::Function = lua
        .load(
            r#"
                function inspect(tbl, indent)
                    indent = indent or 0
                    for k, v in pairs(tbl) do
                        print(string.rep(" ", indent) .. tostring(k) .. ":")
                        if type(v) == "table" then
                            inspect(v, indent + 2)
                        else
                            print(string.rep(" ", indent + 2) .. tostring(v))
                        end
                    end
                end

                return inspect
            "#,
        )
        .eval()?;
    module.set("inspect", inspect)?;

    let find_module: mlua::Function = lua
        .load(
            r#"
                function (name)
                    if package.loaded[name] then
                        return require(name)
                    else
                        for _, searcher in ipairs(package.searchers or package.loaders) do
                            local loader = searcher(name)
                            if type(loader) == "function" then
                                package.preload[name] = loader
                                return require(name)
                            end
                        end
                        return nil
                    end
                end
            "#,
        )
        .eval()?;
    module.set("find_module", find_module)?;

    module.set("args", lua.to_value(&args)?)?;

    module.set("hostname", whoami::fallible::hostname()?)?;
    module.set("user", whoami::fallible::username()?)?;

    module.set("fs", create_fs_submodule(lua, args)?)?;

    #[cfg(windows)]
    module.set("platform", "windows")?;
    #[cfg(unix)]
    module.set("platform", "unix")?;

    Ok(module)
}

fn setup_lua_path(lua: &mlua::Lua, dotfiles_path: &Path) -> Result<()> {
    let dotfiles_path = dotfiles_path.to_str().unwrap().replace("\\", "/");

    let globals = lua.globals();
    let package: mlua::Table = globals.get("package")?;

    let path: String = package.get("path")?;
    package.set("path", format!("{dotfiles_path}/.straw/?.lua;{path}"))?;

    let cpath: String = package.get("cpath")?;
    #[cfg(windows)]
    let ext = ".dll";
    #[cfg(unix)]
    let ext = ".so";
    package.set("cpath", format!("{dotfiles_path}/.straw/?{ext};{cpath}"))?;

    Ok(())
}

pub fn create_lua_runtime(dotfiles_path: &Path, args: &Args) -> Result<mlua::Lua> {
    let lua = mlua::Lua::new();
    setup_lua_path(&lua, dotfiles_path)?;
    create_straw_module(&lua, args)?;
    Ok(lua)
}

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
            local config = straw.find_module("test")
            local config = straw.find_module("config")
            -- dump(config)
            return config
        "#,
        )
        .eval::<mlua::Value>()?;

    // setting `deny_unsupported_types` to falls allows the config to be deserialized if the user
    // defined functions (e.g. `on_apply` callback) within the config table
    let options = mlua::serde::de::Options::default().deny_unsupported_types(false);
    let mut config: Option<Config> = lua.from_value_with(config_value.clone(), options)?;
    if let Some(config) = &mut config {
        config.value = Some(config_value);
    }

    tracing::trace!("{config:?}");

    Ok(config.unwrap_or_default())
}

// https://github.com/wez/wezterm/blob/main/config/src/lua.rs#L33
pub fn get_or_create_module(lua: &mlua::Lua, name: &str) -> Result<mlua::Table> {
    let globals = lua.globals();
    let package: mlua::Table = globals.get("package")?;
    let loaded: mlua::Table = package.get("loaded")?;

    let module = loaded.get(name)?;
    match module {
        mlua::Value::Nil => {
            let module = lua.create_table()?;
            loaded.set(name, module.clone())?;
            Ok(module)
        }
        mlua::Value::Table(table) => Ok(table),
        wat => Err(StrawError::String(format!(
            "cannot register module {} as package.loaded.{} is already set to a value of type {}",
            name,
            name,
            wat.type_name()
        ))),
    }
}

// https://github.com/wez/wezterm/blob/main/config/src/lua.rs#L55
pub fn get_or_create_sub_module(lua: &mlua::Lua, name: &str) -> Result<mlua::Table> {
    let wezterm_mod = get_or_create_module(lua, "wezterm")?;
    let sub = wezterm_mod.get(name)?;
    match sub {
        mlua::Value::Nil => {
            let sub = lua.create_table()?;
            wezterm_mod.set(name, sub.clone())?;
            Ok(sub)
        }
        mlua::Value::Table(sub) => Ok(sub),
        wat => Err(StrawError::String(format!(
            "cannot register module wezterm.{name} as it is already set to a value of type {}",
            wat.type_name()
        ))),
    }
}
