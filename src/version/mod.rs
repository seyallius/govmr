//! Module version - Data structures and resolution logic for Go toolchain versions.

pub mod manifest;
pub mod resolve;

pub use manifest::{GoRelease, ReleaseFile};
pub use resolve::{compare_versions, parse_version_query, resolve_version, version_matches};

use std::path::PathBuf;

// ------------------------------------------ Types & Impls ------------------------------------- //

/// Normalized representation of a Go version within `GoVMR`.
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
    /// Size of the downloadable archive in bytes.
    pub size: u64,
    /// Indicates whether this version is currently installed on the local machine.
    pub installed: bool,
    /// Indicates whether this version is the currently selected active version.
    pub active: bool,
    /// Filesystem path to the installed Go root directory, if installed.
    pub path: Option<PathBuf>,
    /// Indicates whether this release is marked as a stable release.
    pub stable: bool,
}
impl GoVersion {
    /// Extracts the pre-release / unstable suffix of a version, if any.
    ///
    /// For example `1.24rc1` yields `Some("rc1")` and `1.22.0` yields `None`.
    #[must_use]
    pub fn prerelease_tag(raw: &str) -> Option<String> {
        let tag = raw.split('.').find_map(|part| {
            let idx = part.find(|c: char| c.is_ascii_alphabetic())?;
            Some(part[idx..].trim())
        })?;
        if tag.is_empty() {
            None
        } else {
            Some(tag.to_string())
        }
    }

    /// Formats a byte count into a compact, human-readable string (e.g. `72.4 MB`).
    #[must_use]
    pub fn format_size(bytes: u64) -> String {
        const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
        // Exact byte counts are uninteresting beyond the one decimal shown.
        #[allow(clippy::cast_precision_loss)]
        let mut value = bytes as f64;
        let mut unit = 0;
        while value >= 1024.0 && unit < UNITS.len() - 1 {
            value /= 1024.0;
            unit += 1;
        }
        if unit == 0 {
            format!("{} {}", bytes, UNITS[unit])
        } else {
            format!("{:.1} {}", value, UNITS[unit])
        }
    }
}
