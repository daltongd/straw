use std::path::Path;

use crate::{result::Result, straw_config::Config};

fn create_straw_table(lua: &mlua::Lua) -> Result<mlua::prelude::LuaTable> {
    let exports = lua.create_table()?;
    let f = lua.create_function(move |_, ()| {
        tracing::warn!("WORKS");
        Ok(())
    })?;
    exports.set("f", f)?;

    #[cfg(windows)]
    exports.set("platform", "windows")?;
    #[cfg(unix)]
    exports.set("platform", "unix")?;

    Ok(exports)
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

pub fn run_lua_config(dotfiles_path: &Path) -> Result<Config> {
    use mlua::prelude::*;

    let lua = Lua::new();

    setup_lua_path(&lua, dotfiles_path)?;

    let globals = lua.globals();
    globals.set("straw", create_straw_table(&lua)?)?;

    let config = lua
        .load(
            r#"
            function dump(tbl, indent)
                indent = indent or 0
                for k, v in pairs(tbl) do
                    print(string.rep(" ", indent) .. tostring(k) .. ":")
                    if type(v) == "table" then
                        dump(v, indent + 2)
                    else
                        print(string.rep(" ", indent + 2) .. tostring(v))
                    end
                end
            end

            function check_module(name)
                if package.loaded[name] then
                    return true
                else
                    for _, searcher in ipairs(package.searchers or package.loaders) do
                        local loader = searcher(name)
                        if type(loader) == "function" then
                            package.preload[name] = loader
                            return true
                        end
                    end
                    return false
                end
            end

            function find_module(name)
                if check_module(name) then
                    return require(name)
                else
                    return nil
                end
            end

            local config = require("config")
            -- dump(config)
            return config
        "#,
        )
        .eval()?;

    let config: Config = lua.from_value(config)?;

    tracing::info!("{config:?}");

    Ok(config)
}
