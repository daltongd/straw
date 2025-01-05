use std::path::{Path, PathBuf};

use mlua::LuaSerdeExt;

use crate::{args::Args, result::Result};

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

fn populate_fs_submodule(lua: &mlua::Lua, args: &Args) -> Result<mlua::Table> {
    let fs = crate::lua::utils::get_or_create_sub_module(lua, "fs")?;

    let paths = lua.create_table()?;
    fs.set("paths", &paths)?;
    let home_dir = crate::utils::get_home_dir()?;
    paths.set("home", crate::fs::sanitize_path(&home_dir)?)?;

    let dotfiles_dir = crate::utils::get_dotfiles_path(args)?;
    if let Some(dotfiles_dir) = dotfiles_dir.to_str() {
        paths.set("dotfiles", crate::fs::sanitize_path(dotfiles_dir)?)?;
    }

    fs.set("open", lua.create_function(crate::lua::fs::open_file)?)?;

    fs.set(
        "exists",
        lua.create_function(move |_, path: String| {
            let path = crate::fs::sanitize_path(&path)?;
            let path = path.replace("~", home_dir.as_str());
            Ok(PathBuf::from(path).exists())
        })?,
    )?;

    Ok(fs)
}

fn create_straw_module(lua: &mlua::Lua, args: &Args) -> Result<mlua::Table> {
    let module = crate::lua::utils::get_or_create_module(lua, "straw")?;

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

    populate_fs_submodule(lua, args)?;

    #[cfg(windows)]
    module.set("platform", "windows")?;
    #[cfg(unix)]
    module.set("platform", "unix")?;

    Ok(module)
}
