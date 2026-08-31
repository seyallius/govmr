//! Copyright (c) 2026 SeyedAli
//! Licensed under the MIT License. See LICENSE file in the project root for details.
//!
//! Module models - Data structures representing Go releases and local state.

use serde::Deserialize;
use std::path::PathBuf;

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

/// Normalized representation of a Go version within GoVMR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoVersion {
    /// Stripped numeric version string (e.g., `1.22.0`).
    pub raw_version: String,
    /// Display-formatted version name (e.g., `go1.22.0`).
    pub display_name: String,
    /// Archive filename matching the host architecture.
    pub filename: String,
    /// Full remote URL from which the archive can be downloaded.
    pub url: String,
    /// Indicates whether this version is currently installed on the local machine.
    pub installed: bool,
    /// Indicates whether this version is the currently selected active version.
    pub active: bool,
    /// Filesystem path to the installed Go root directory, if installed.
    pub path: Option<PathBuf>,
    /// Indicates whether this release is marked as a stable release.
    pub stable: bool,
}
