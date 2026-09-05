//! Module resolve - User-query parsing, semver-aware matching, and version resolution.
//!
//! Turns user input like `1.22`, `1.21.6`, or `1.24rc1` into the concrete
//! [`GoVersion`](crate::version::GoVersion) the caller wants, applying proper
//! component-prefix matching (so `1.2` never matches `1.20`).

use super::GoVersion;

// ----------------------------------------- Public API ----------------------------------------- //

/// Splits a version-ish string into leading numeric components and an
/// optional pre-release suffix.
///
/// * `1.22.0`   → `([1, 22, 0], None)`
/// * `1.24rc1`  → `([1, 24], Some("rc1"))`
/// * `1.21.beta2` → `([1, 21], Some("beta2"))`
#[must_use]
pub fn parse_version_query(raw: &str) -> (Vec<u64>, Option<String>) {
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
#[must_use]
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

/// Numeric comparison of two version strings, used to sort versions newest-first.
///
/// Compares dot-separated numeric components only (pre-release suffixes are
/// ignored), so `1.10.0` sorts after `1.9.0`. Mirrors the component rules of
/// [`version_matches`] so sorting and matching stay consistent.
#[must_use]
pub fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
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
