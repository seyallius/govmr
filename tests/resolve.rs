//! Tests for semver-aware version query resolution (the prefix-matching bug).

// Tests exist to fail loudly: panicking on a broken setup or a failed helper
// is exactly the desired behavior, so unwraps are idiomatic here.
#![allow(clippy::unwrap_used)]

use govmr::version::{GoVersion, resolve_version, version_matches};

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
