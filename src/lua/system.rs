use crate::{args::Args, result::StrawError};

pub type Result<T> = std::result::Result<T, mlua::Error>;

pub fn exec(
    _lua: &mlua::Lua,
    args: &Args,
    (program, process_args, silent): (String, Vec<String>, bool),
) -> Result<String> {
    let output = std::process::Command::new(&program)
        .args(&process_args)
        .current_dir(&crate::utils::get_dotfiles_path(args)?)
        .output()?;

    let stdout = String::from_utf8(output.stdout).map_err(|e| StrawError::String(e.to_string()))?;
    if !silent {
        for line in stdout.lines() {
            tracing::info!("{line}");
        }

        let stderr =
            String::from_utf8(output.stderr).map_err(|e| StrawError::String(e.to_string()))?;
        for line in stderr.lines() {
            tracing::error!("{line}");
        }
    }

    if output.status.success() {
        tracing::info!("{program} exited with {}", output.status);
    } else {
        tracing::error!("{program} exited with {}", output.status);
    }

    Ok(stdout)
}
