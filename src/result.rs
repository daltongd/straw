pub type Result<T> = std::result::Result<T, StrawError>;

#[derive(Debug)]
pub enum StrawError {
    Io(std::io::Error),
    String(String),
    Tracing(tracing::subscriber::SetGlobalDefaultError),
    Serde(serde_json::error::Error),
    Regex(regex::Error),
    Git(git2::Error),
    Lua(mlua::Error),
}

impl From<std::io::Error> for StrawError {
    fn from(value: std::io::Error) -> Self {
        StrawError::Io(value)
    }
}

impl From<String> for StrawError {
    fn from(value: String) -> Self {
        StrawError::String(value)
    }
}

impl From<tracing::subscriber::SetGlobalDefaultError> for StrawError {
    fn from(value: tracing::subscriber::SetGlobalDefaultError) -> Self {
        StrawError::Tracing(value)
    }
}

impl From<serde_json::error::Error> for StrawError {
    fn from(value: serde_json::error::Error) -> Self {
        StrawError::Serde(value)
    }
}

impl From<regex::Error> for StrawError {
    fn from(value: regex::Error) -> Self {
        StrawError::Regex(value)
    }
}

impl From<git2::Error> for StrawError {
    fn from(value: git2::Error) -> Self {
        StrawError::Git(value)
    }
}

impl From<mlua::Error> for StrawError {
    fn from(value: mlua::Error) -> Self {
        StrawError::Lua(value)
    }
}
