//! Module manifest - Response types of the official go.dev release manifest.
//!
//! Mirrors the JSON served by `https://go.dev/dl/?mode=json&include=all` so the
//! network response can be deserialized directly into typed structures.

use serde::Deserialize;

// ------------------------------------------ Types & Impls ------------------------------------- //

/// Represents a downloadable binary or source file listed in the official Go release manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseFile {
    /// The exact archive filename (e.g., `go1.22.0.linux-amd64.tar.gz`).
    pub filename: String,
    /// The target operating system (e.g., `linux`, `darwin`, `windows`).
    pub os: String,
    /// The target system architecture (e.g., `amd64`, `arm64`).
    pub arch: String,
    /// The size of the archive in bytes.
    pub size: usize,
}

/// Represents a Go release object retrieved from the `go.dev` API manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct GoRelease {
    /// The raw version string prefixed with 'go' (e.g., `go1.22.0`).
    pub version: String,
    /// Indicates whether this release is marked as stable.
    pub stable: bool,
    /// Collection of downloadable archive files available for this release.
    pub files: Vec<ReleaseFile>,
}
