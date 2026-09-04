//! Module models - Data structures representing Go releases and local state.

use serde::Deserialize;
use std::path::PathBuf;

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
    pub fn prerelease_tag(raw: &str) -> Option<String> {
        let tag = raw
            .split('.')
            .filter_map(|part| {
                let idx = part.find(|c: char| c.is_ascii_alphabetic())?;
                Some(part[idx..].trim())
            })
            .next()?;
        if tag.is_empty() {
            None
        } else {
            Some(tag.to_string())
        }
    }

    /// Formats a byte count into a compact, human-readable string (e.g. `72.4 MB`).
    pub fn format_size(bytes: u64) -> String {
        const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
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

/// Splits a version-ish string into leading numeric components and an
/// optional pre-release suffix.
///
/// * `1.22.0`   → `([1, 22, 0], None)`
/// * `1.24rc1`  → `([1, 24], Some("rc1"))`
/// * `1.21.beta2` → `([1, 21], Some("beta2"))`
pub fn parse_version_query(raw: &str) -> (Vec<u64>, Option<String>) {
    let mut nums = Vec::new();
    let mut tag = None;
    for part in raw.split('.') {
        let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            if let Ok(n) = digits.parse::<u64>() {
                nums.push(n);
            }
        }
        let idx = part.find(|c: char| c.is_ascii_alphabetic());
        if let Some(i) = idx {
            tag = Some(part[i..].to_string());
        }
    }
    (nums, tag)
}

/// Reports whether a concrete version matches a user query using proper
/// semver-aware *component prefix* matching.
///
/// Rules:
/// * The numeric components of the query must equal the leading components of
///   the version. This respects component boundaries, so `1.2` matches `1.2.x`
///   but **not** `1.20.x`, and `1.20` never matches a future `1.200.x`.
/// * A pre-release suffix on the query (e.g. `rc1`) must match exactly.
/// * When the query has no suffix, only stable releases are considered.
pub fn version_matches(query_raw: &str, version_raw: &str) -> bool {
    let (q_nums, q_tag) = parse_version_query(query_raw);
    let (v_nums, v_tag) = parse_version_query(version_raw);

    if q_nums.is_empty() || q_nums.len() > v_nums.len() {
        return false;
    }
    if v_nums[..q_nums.len()] != q_nums[..] {
        return false;
    }

    match q_tag {
        Some(qt) => v_tag.as_deref() == Some(qt.as_str()),
        None => v_tag.is_none(),
    }
}

/// Resolves a user query (`"1.22"`, `"1.21.6"`, `"1.24rc1"`) against a list of
/// versions (assumed to be ordered newest-first).
///
/// Exact matches win; otherwise the newest stable release matching the prefix
/// is returned. Prerelease queries require an exact pre-release match.
pub fn resolve_version<'a>(query: &str, versions: &'a [GoVersion]) -> Option<&'a GoVersion> {
    let clean = query.trim().trim_start_matches("go");

    // 1) Exact raw-version match always takes precedence.
    if let Some(found) = versions
        .iter()
        .find(|v| v.raw_version == clean || v.display_name == clean)
    {
        return Some(found);
    }

    // 2) Best (newest) semver-prefix match.
    let mut best: Option<&GoVersion> = None;
    for v in versions {
        if version_matches(clean, &v.raw_version) {
            match best {
                None => best = Some(v),
                Some(b) => {
                    // Keep the newer of the two (list is newest-first, but be
                    // explicit in case callers pass unsorted lists).
                    if is_newer(&v.raw_version, &b.raw_version) {
                        best = Some(v);
                    }
                }
            }
        }
    }
    best
}

/// Numeric version comparison used by the resolver.
fn is_newer(candidate: &str, than: &str) -> bool {
    let (a, _) = parse_version_query(candidate);
    let (b, _) = parse_version_query(than);
    for (x, y) in a.iter().zip(b.iter()) {
        if x != y {
            return x > y;
        }
    }
    a.len() > b.len()
}
