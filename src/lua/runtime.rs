use std::path::Path;

use mlua::LuaSerdeExt;
use serde::de::DeserializeOwned;

use crate::{args::Args, result::Result};

pub struct LuaRuntime {
    lua: mlua::Lua,
}

impl LuaRuntime {
    pub fn new(dotfiles_path: &Path, args: &Args) -> Result<Self> {
        let lua = mlua::Lua::new();
        let runtime = Self { lua };
        runtime.setup_lua_path(dotfiles_path)?;
        runtime.create_straw_module(args)?;
        Ok(runtime)
    }

    pub fn lua(&self) -> &mlua::Lua {
        &self.lua
    }

    pub fn run<T>(&self, lua_script: &str) -> Result<(Option<T>, mlua::Value)>
    where
        T: DeserializeOwned,
    {
        let value = self.lua.load(lua_script).eval::<mlua::Value>()?;

        // setting `deny_unsupported_types` to false allows the config to be deserialized if the user
        // defined functions (e.g. `on_apply` callback) within the config table
        let options = mlua::serde::de::Options::default().deny_unsupported_types(false);

        let result: Option<T> = self.lua.from_value_with(value.clone(), options)?;
        Ok((result, value))
    }

    fn populate_fs_submodule(&self, args: &Args) -> Result<mlua::Table> {
        let fs = crate::lua::utils::get_or_create_sub_module(&self.lua, "fs")?;

        let paths = self.lua.create_table()?;
        fs.set("paths", &paths)?;
        let home_dir = crate::utils::get_home_dir()?;
        paths.set("home", crate::fs::sanitize_path(&home_dir)?)?;

        let dotfiles_dir = crate::utils::get_dotfiles_path(args)?;
        if let Some(dotfiles_dir) = dotfiles_dir.to_str() {
            paths.set("dotfiles", crate::fs::sanitize_path(dotfiles_dir)?)?;
        }

        fs.set(
            "create_symlink",
            self.lua.create_function(crate::lua::fs::create_symlink)?,
        )?;

        fs.set(
            "create_dir",
            self.lua.create_function(crate::lua::fs::create_dir)?,
        )?;
        fs.set(
            "remove_dir",
            self.lua.create_function(crate::lua::fs::remove_dir)?,
        )?;

        fs.set(
            "remove",
            self.lua.create_function(crate::lua::fs::remove_file)?,
        )?;

        fs.set(
            "read",
            self.lua.create_function(crate::lua::fs::read_text_file)?,
        )?;
        fs.set(
            "write",
            self.lua.create_function(crate::lua::fs::write_text_file)?,
        )?;
        fs.set(
            "read_bin",
            self.lua.create_function(crate::lua::fs::read_bin_file)?,
        )?;
        fs.set(
            "write_bin",
            self.lua.create_function(crate::lua::fs::write_bin_file)?,
        )?;
        fs.set(
            "get_file_type",
            self.lua.create_function(crate::lua::fs::get_file_type)?,
        )?;

        fs.set("exists", self.lua.create_function(crate::lua::fs::exists)?)?;

        Ok(fs)
    }

    fn create_straw_module(&self, args: &Args) -> Result<mlua::Table> {
        let module = crate::lua::utils::get_or_create_module(&self.lua, "straw")?;

        let inspect: mlua::Function = self
            .lua
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

        let find_module: mlua::Function = self
            .lua
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

        module.set("args", self.lua.to_value(&args)?)?;

        module.set("hostname", whoami::fallible::hostname()?)?;
        module.set("user", whoami::fallible::username()?)?;

        let straw_args = args.clone();
        module.set(
            "exec",
            self.lua.create_function(
                move |lua: &mlua::Lua,
                      (program, process_args, silent): (String, Vec<String>, bool)|
                      -> std::result::Result<String, mlua::Error> {
                    crate::lua::system::exec(lua, &straw_args, (program, process_args, silent))
                },
            )?,
        )?;

        self.populate_fs_submodule(args)?;

        #[cfg(windows)]
        module.set("platform", "windows")?;
        #[cfg(unix)]
        module.set("platform", "unix")?;

        Ok(module)
    }

    fn setup_lua_path(&self, dotfiles_path: &Path) -> Result<()> {
        let dotfiles_path = dotfiles_path.to_str().unwrap().replace("\\", "/");

        let globals = self.lua.globals();
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
}
