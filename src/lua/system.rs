use crate::{
    args::Args,
    result::{LuaResult, StrawError},
};

pub fn exec(
    _lua: &mlua::Lua,
    args: &Args,
    (program, process_args, silent): (String, Vec<String>, bool),
) -> LuaResult<Option<i32>> {
    let output = std::process::Command::new(&program)
        .args(&process_args)
        .current_dir(&crate::utils::get_dotfiles_path(args)?)
        .output()?;

    let stdout = String::from_utf8(output.stdout).map_err(|e| StrawError::String(e.to_string()))?;
    if !silent {
        tracing::info!("Executing '{program}' with args {process_args:?}");
        for line in stdout.lines() {
            tracing::info!("{line}");
        }

        let stderr =
            String::from_utf8(output.stderr).map_err(|e| StrawError::String(e.to_string()))?;
        for line in stderr.lines() {
            tracing::error!("{line}");
        }

        if output.status.success() {
            tracing::info!("{program} exited with {}", output.status);
        } else {
            tracing::error!("{program} exited with {}", output.status);
        }
    }

    Ok(output.status.code())
}
