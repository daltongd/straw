use std::fmt::Display;

pub type Result<T> = std::result::Result<T, StrawError>;
pub type LuaResult<T> = std::result::Result<T, mlua::Error>;

#[derive(Debug)]
pub enum StrawError {
    Io(std::io::Error),
    String(String),
    Tracing(tracing::subscriber::SetGlobalDefaultError),
    Git(git2::Error),
    Regex(lazy_regex::regex::Error),
    Lua(mlua::Error),
    TomlSerialize(toml::ser::Error),
    TomlDeserialize(toml::de::Error),
    StripPrefix(std::path::StripPrefixError),
}

impl Display for StrawError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StrawError::String(error) => {
                write!(f, "{error}")
            }
            _ => {
                write!(f, "{self:?}")
            }
        }
    }
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

impl From<lazy_regex::regex::Error> for StrawError {
    fn from(value: lazy_regex::regex::Error) -> Self {
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

impl From<toml::ser::Error> for StrawError {
    fn from(value: toml::ser::Error) -> Self {
        StrawError::TomlSerialize(value)
    }
}

impl From<toml::de::Error> for StrawError {
    fn from(value: toml::de::Error) -> Self {
        StrawError::TomlDeserialize(value)
    }
}

impl From<std::path::StripPrefixError> for StrawError {
    fn from(value: std::path::StripPrefixError) -> Self {
        StrawError::StripPrefix(value)
    }
}

impl From<StrawError> for mlua::Error {
    fn from(val: StrawError) -> Self {
        mlua::Error::runtime(val.to_string())
    }
}
