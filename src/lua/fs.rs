use std::path::Path;

pub fn open_file(lua: &mlua::Lua, path: String) -> Result<(), mlua::Error> {
    // TODO: match with error
    let path = path.replace("~", &crate::utils::get_home_dir()?);
    if let Ok(file) = std::fs::File::open(path) {
        tracing::info!("{:?}", file.metadata()?);
    } else {
        tracing::info!("Cannot open file");
    }

    Ok(())
}
