use crate::result::{Result, StrawError};

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
    let module = get_or_create_module(lua, "straw")?;
    let sub = module.get(name)?;
    match sub {
        mlua::Value::Nil => {
            let submodule = lua.create_table()?;
            module.set(name, submodule.clone())?;
            Ok(submodule)
        }
        mlua::Value::Table(sub) => Ok(sub),
        wat => Err(StrawError::String(format!(
            "cannot register module straw.{name} as it is already set to a value of type {}",
            wat.type_name()
        ))),
    }
}
