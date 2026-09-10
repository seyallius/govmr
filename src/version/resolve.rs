//! Module resolve - User-query parsing, semver-aware matching, and version resolution.
//!
//! Turns user input like `1.22`, `1.21.6`, or `1.24rc1` into the concrete
//! [`GoVersion`] the caller wants, applying proper component-prefix matching
//! (so `1.2` never matches `1.20`).

use super::GoVersion;
use crate::logging;

// ------------------------------------- Public (crate) API ------------------------------------- //

/// Splits a version-ish string into leading numeric components and an
/// optional pre-release suffix.
///
/// * `1.22.0`   → `([1, 22, 0], None)`
/// * `1.24rc1`  → `([1, 24], Some("rc1"))`
/// * `1.21.beta2` → `([1, 21], Some("beta2"))`
#[must_use]
pub(crate) fn parse_version_query(raw: &str) -> (Vec<u64>, Option<String>) {
    let mut nums = Vec::new();
    let mut tag = None;
    for part in raw.split('.') {
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty()
            && let Ok(n) = digits.parse::<u64>()
        {
            nums.push(n);
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
#[must_use]
pub(crate) fn version_matches(query_raw: &str, version_raw: &str) -> bool {
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
#[must_use]
pub(crate) fn resolve_version<'a>(query: &str, versions: &'a [GoVersion]) -> Option<&'a GoVersion> {
    let clean = query.trim().trim_start_matches("go");

    // 1) Exact raw-version match always takes precedence.
    if let Some(found) = versions
        .iter()
        .find(|v| v.raw_version == clean || v.display_name == clean)
    {
        log_resolution(clean, versions.len(), Some((found, "exact")));
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
    log_resolution(clean, versions.len(), best.map(|v| (v, "prefix")));
    best
}

/// Records what the resolver was asked for and how it answered.
///
/// "Version not found" is far easier to debug when the log says which query was
/// matched against how many candidates — a stale/short manifest and a typo look
/// identical to the user. Kept at DEBUG because the caller owns the error line.
fn log_resolution(query: &str, candidates: usize, matched: Option<(&GoVersion, &str)>) {
    match matched {
        Some((version, kind)) => logging::debug(&format!(
            "resolve: matched query=\"{query}\" candidates={candidates} version={} kind={kind}",
            version.raw_version
        )),
        None => logging::debug(&format!(
            "resolve: no match query=\"{query}\" candidates={candidates} note=no_entry_matched_prefix",
        )),
    }
}

/// Numeric comparison of two version strings, used to sort versions newest-first.
///
/// Compares dot-separated numeric components only (pre-release suffixes are
/// ignored), so `1.10.0` sorts after `1.9.0`. Mirrors the component rules of
/// [`version_matches`] so sorting and matching stay consistent.
#[must_use]
pub(crate) fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|p| {
                p.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    .ok()
            })
            .collect()
    };
    parse(v1).cmp(&parse(v2))
}

// -------------------------------------- Internal Helpers -------------------------------------- //

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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn gv(raw: &str, stable: bool) -> GoVersion {
        GoVersion {
            raw_version: raw.to_string(),
            display_name: format!("go{raw}"),
            filename: format!("go{raw}.tar.gz"),
            url: String::new(),
            size: 0,
            installed: false,
            active: false,
            path: None,
            stable,
        }
    }

    #[test]
    fn component_prefixes_respect_boundaries() {
        // "1.2" must match the 1.2 line but NOT 1.20 / 1.21 / 1.24.
        assert!(version_matches("1.2", "1.2.0"));
        assert!(version_matches("1.2", "1.2.7"));
        assert!(!version_matches("1.2", "1.20.0"));
        assert!(!version_matches("1.2", "1.21.6"));
        assert!(!version_matches("1.2", "1.24rc1"));

        // "1.20" matches the 1.20 line but not a hypothetical 1.200.
        assert!(version_matches("1.20", "1.20.14"));
        assert!(!version_matches("1.20", "1.200.0"));
        assert!(!version_matches("1.20", "1.2.0"));
    }

    #[test]
    fn prerelease_queries_require_exact_match() {
        assert!(version_matches("1.24rc1", "1.24rc1"));
        assert!(!version_matches("1.24rc1", "1.24.0"));
        // Stable queries never resolve to prereleases.
        assert!(!version_matches("1.24", "1.24rc1"));
        assert!(version_matches("1.24", "1.24.0"));
    }

    #[test]
    fn resolver_picks_newest_stable_for_prefix() {
        // Ordered newest-first like fetch_versions returns.
        let versions = vec![
            gv("1.24.1", true),
            gv("1.24.0", true),
            gv("1.24rc1", false),
            gv("1.23.4", true),
            gv("1.22.6", true),
        ];

        let got = resolve_version("1.22", &versions).unwrap();
        assert_eq!(got.raw_version, "1.22.6");

        let got = resolve_version("1.24", &versions).unwrap();
        assert_eq!(got.raw_version, "1.24.1", "stable beats rc, newest wins");

        let got = resolve_version("1.24rc1", &versions).unwrap();
        assert_eq!(got.raw_version, "1.24rc1");

        assert!(
            resolve_version("1.2", &versions).is_none(),
            "no 1.2 line present"
        );
    }

    #[test]
    fn resolver_exact_match_wins() {
        let versions = vec![gv("1.22.6", true), gv("1.22.0", true)];
        let got = resolve_version("1.22.0", &versions).unwrap();
        assert_eq!(got.raw_version, "1.22.0");
    }
}
