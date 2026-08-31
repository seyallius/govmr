//! Copyright (c) 2026 SeyedAli
//! Licensed under the MIT License. See LICENSE file in the project root for details.
//
//! Module errors - Domain-specific error types for GoVMR operations.

use thiserror::Error;

/// Comprehensive error enumeration for GoVMR application lifecycle and runtime failures.
#[derive(Error, Debug)]
pub enum GovmError {
    /// Emitted when the user's home directory cannot be resolved from the environment.
    #[error("Home directory not found")]
    HomeNotFound,
    /// Emitted when attempting an operation on a Go version that is not installed locally.
    #[error("Version {0} is not installed")]
    NotInstalled(String),
    /// Emitted when attempting to delete the currently active Go version.
    #[error("Cannot delete active version {0}. Switch to another version first")]
    CannotDeleteActive(String),
    /// Emitted when no remote or local Go version matches the requested query.
    #[error("No Go version matching '{0}' found")]
    VersionNotFound(String),
    /// Emitted when an HTTP network request fails during manifest retrieval or binary download.
    #[error("HTTP error: {0}")]
    Network(#[from] reqwest::Error),
    /// Emitted when a filesystem or process IO operation fails.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Emitted when archive extraction (tar.gz or zip) encounters corrupt data or extraction errors.
    #[error("Archive extraction error: {0}")]
    Extraction(String),
}
