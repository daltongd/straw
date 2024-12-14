pub type Result<T> = std::result::Result<T, StrawError>;

#[derive(Debug)]
pub enum StrawError {
    Io(std::io::Error),
    String(String),
    Tracing(tracing::subscriber::SetGlobalDefaultError),
    Serde(serde_json::error::Error),
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
