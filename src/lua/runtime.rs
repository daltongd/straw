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

    pub fn register_func<F, A, R>(
        &self,
        module: &mlua::Table,
        key: impl mlua::IntoLua,
        func: F,
    ) -> Result<mlua::Function>
    where
        F: Fn(&mlua::Lua, A) -> mlua::Result<R> + mlua::MaybeSend + 'static,
        A: mlua::FromLuaMulti,
        R: mlua::IntoLuaMulti,
    {
        let function = self.lua.create_function(func)?;
        module.set(key, &function)?;
        Ok(function)
    }

    pub fn register_func_from_lua(
        &self,
        module: &mlua::Table,
        key: impl mlua::IntoLua,
        lua_script: &str,
    ) -> Result<mlua::Function> {
        let function: mlua::Function = self.lua.load(lua_script).eval()?;
        module.set(key, &function)?;
        Ok(function)
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

        self.register_func(&fs, "create_symlink", crate::lua::fs::create_symlink)?;

        self.register_func(&fs, "create_dir", crate::lua::fs::create_dir)?;
        self.register_func(&fs, "remove_dir", crate::lua::fs::remove_dir)?;

        self.register_func(&fs, "remove", crate::lua::fs::remove_file)?;

        self.register_func(&fs, "read", crate::lua::fs::read_text_file)?;
        self.register_func(&fs, "write", crate::lua::fs::write_text_file)?;

        self.register_func(&fs, "read_bin", crate::lua::fs::read_bin_file)?;
        self.register_func(&fs, "write_bin", crate::lua::fs::write_bin_file)?;

        self.register_func(&fs, "exists", crate::lua::fs::exists)?;
        self.register_func(&fs, "get_file_type", crate::lua::fs::get_file_type)?;

        Ok(fs)
    }

    fn populate_git_submodule(&self) -> Result<mlua::Table> {
        let git = crate::lua::utils::get_or_create_sub_module(&self.lua, "git")?;

        self.register_func(&git, "clone", crate::lua::git::clone)?;
        self.register_func(&git, "init", crate::lua::git::init)?;
        self.register_func(&git, "commit", crate::lua::git::commit)?;
        self.register_func(&git, "status", crate::lua::git::status)?;
        self.register_func(&git, "pull", crate::lua::git::pull)?;
        self.register_func(&git, "push", crate::lua::git::push)?;

        Ok(git)
    }

    fn create_straw_module(&self, args: &Args) -> Result<mlua::Table> {
        let module = crate::lua::utils::get_or_create_module(&self.lua, "straw")?;

        self.register_func_from_lua(
            &module,
            "inspect",
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
        )?;

        self.register_func_from_lua(
            &module,
            "find_module",
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
        )?;

        module.set("args", self.lua.to_value(&args)?)?;

        module.set("hostname", whoami::fallible::hostname()?)?;
        module.set("user", whoami::fallible::username()?)?;

        let straw_args = args.clone();
        self.register_func(
            &module,
            "exec",
            move |lua: &mlua::Lua,
                  (program, process_args, silent): (String, Vec<String>, bool)|
                  -> crate::result::LuaResult<Option<i32>> {
                crate::lua::system::exec(lua, &straw_args, (program, process_args, silent))
            },
        )?;

        self.populate_fs_submodule(args)?;
        self.populate_git_submodule()?;

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
