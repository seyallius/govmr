use thiserror::Error;

#[derive(Error, Debug)]
pub enum GovmError {
    #[error("Home directory not found")]
    HomeNotFound,
    #[error("Version {0} is not installed")]
    NotInstalled(String),
    #[error("Cannot delete active version {0}. Switch to another version first")]
    CannotDeleteActive(String),
    #[error("No Go version matching '{0}' found")]
    VersionNotFound(String),
    #[error("HTTP error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Archive extraction error: {0}")]
    Extraction(String),
}
