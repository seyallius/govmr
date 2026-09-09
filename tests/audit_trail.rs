//! Post-mortem coverage tests for the operational log (issue #56).
//!
//! The contract under test is not "a function returned Ok" but *"a bug report
//! containing only `govmr.log` is enough to reconstruct what happened"*. So each
//! case drives the real code path and then reads the log back through the same
//! public API the TUI log viewer uses.
//!
//! Kept to a single `#[test]` on purpose: [`logging::init_in`] is
//! first-call-wins against a process-global handle, so one scenario per test
//! binary is what makes the assertions deterministic.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use govmr::{
    config::Config,
    logging,
    manager::check_archive_magic,
    shim::ShimManager,
    version::{GoVersion, resolve::resolve_version},
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Log areas where a second identical line means two layers reported the same
/// event (the duplicate-logging findings this test guards against).
const DEDUPED_AREAS: [&str; 6] = [
    "install", "download", "archive", "extract", "shim", "update",
];

/// Creates a uniquely-named scratch directory (no external test deps).
fn scratch(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!(
        "govmr-audit-{}-{}-{tag}",
        std::process::id(),
        nanos % 1_000_000
    ));
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Strips the `YYYY-MM-DD HH:MM:SSZ LEVEL ` prefix from every log line.
fn messages() -> Vec<String> {
    logging::read_lines()
        .into_iter()
        .map(|line| logging::message_of(&line).to_string())
        .collect()
}

// One long scenario on purpose: `logging::init_in` is first-call-wins against a
// process-global handle, so every assertion about *what was written* has to live
// in the single test that owns the logger in this binary.
#[allow(clippy::too_many_lines)]
#[allow(clippy::cognitive_complexity)]
#[test]
fn audit_trail_lets_a_reader_reconstruct_a_session() {
    let home = scratch("home");
    let base = home.join(".govmr");
    fs::create_dir_all(&base).expect(".govmr");

    // SAFETY: this binary runs exactly one test, so no other thread is reading
    // the environment while HOME is redirected into the scratch directory.
    // Redirecting is mandatory: `ShimManager::setup_shims_for_version` deletes
    // files in the shim dir, and a test must never touch a real ~/.govmr.
    unsafe { std::env::set_var("HOME", &home) };

    logging::init_in(&base.join("govmr.log"));
    assert!(
        base.join("govmr.log").exists(),
        "logging must be initialisable before anything else runs (init-ordering fix)"
    );

    // ---- 1. shim operations, previously completely silent ------------------ //
    let shim = ShimManager::new().expect("shim manager");
    let bin_dir = home.join("go-bin");
    fs::create_dir_all(&bin_dir).expect("bin dir");
    for name in ["go", "gofmt"] {
        fs::write(bin_dir.join(name), b"#!/bin/sh\n").expect("fake binary");
    }
    fs::write(bin_dir.join("LICENSE"), b"text\n").expect("non-binary file");
    shim.setup_shims_for_version(&bin_dir).expect("shims");
    let in_path = shim.is_in_path();

    let log = messages();
    assert!(
        log.iter().any(|l| l.starts_with("shim: ready dir=\"")
            && l.contains(&base.join("shim").display().to_string())),
        "expected a `shim: ready` line naming the shim dir; got:\n{log:#?}"
    );
    let created = log
        .iter()
        .find(|l| l.starts_with("shim: created "))
        .expect("`setup_shims_for_version` must log what it created");
    // `names=` lists every shim written, which is how you tell "no shims at all"
    // from "the wrong shims" in a bug report. (`readdir` order is not stable, so
    // the assertion is order-insensitive.)
    let names = created
        .split("names=[")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .unwrap_or_default();
    let names: Vec<&str> = names.split(", ").collect();
    assert!(
        created.contains("count=3")
            && names.contains(&"go")
            && names.contains(&"gofmt")
            && names.contains(&"LICENSE")
            && created.contains("elapsed_ms="),
        "creation line must report the count, the names and the duration, got: {created}"
    );
    assert!(
        log.iter()
            .any(|l| l.starts_with("shim: wrote name=go ") && l.contains("mode=755")),
        "each shim write should be recorded, got:\n{log:#?}"
    );
    assert!(
        log.iter().any(|l| {
            l.starts_with("shim: path check ")
                && l.contains(&format!("in_path={in_path}"))
                && l.contains("dir=\"")
        }),
        "the PATH check must record both the dir and the answer"
    );

    // ---- 2. config load / migration / save -------------------------------- //
    let fresh = Config::load(&base);
    assert_eq!(fresh.theme, govmr::theme::ThemeName::GoCyan);
    let mut cfg = Config::load(&base);
    cfg.set_theme(govmr::theme::ThemeName::Mono).expect("save");
    let reloaded = Config::load(&base);

    // A legacy plain-text config must be recognised, and the migration stated.
    let legacy_home = scratch("legacy");
    fs::create_dir_all(&legacy_home).expect("legacy base");
    fs::write(legacy_home.join("config"), "theme = cursordark\n").expect("legacy file");
    let migrated = Config::load(&legacy_home);

    // A bogus theme key must not fail silently.
    let bogus_home = scratch("bogus");
    fs::write(bogus_home.join("config.toml"), "theme = \"not-a-theme\"\n").expect("bogus config");
    let bogus = Config::load(&bogus_home);

    let log = messages();
    assert!(
        log.iter()
            .any(|l| l.starts_with("config: loaded ") && l.contains("source=defaults")),
        "loading with no file on disk must say so (source=defaults)"
    );
    assert!(
        log.iter()
            .any(|l| l.starts_with("config: written ") && l.contains("theme=mono")),
        "a successful save must be recorded, got:\n{log:#?}"
    );
    assert!(
        log.iter()
            .any(|l| { l.starts_with("config: loaded ") && l.contains("source=config.toml") }),
        "a reload from the TOML file must say where the value came from"
    );
    assert_eq!(reloaded.theme, govmr::theme::ThemeName::Mono);
    assert!(
        log.iter()
            .any(|l| l.starts_with("config: migrated legacy=") && l.contains("theme=cursordark")),
        "legacy migration must be logged, got:\n{log:#?}"
    );
    assert_eq!(migrated.theme, govmr::theme::ThemeName::CursorDark);
    assert!(
        log.iter().any(
            |l| l.starts_with("config: unknown theme value=\"not-a-theme\"")
                && l.contains("fallback=gocyan")
        ),
        "an unknown theme key must be reported with the fallback used"
    );
    assert_eq!(bogus.theme, govmr::theme::ThemeName::GoCyan);

    // ---- 3. version resolution --------------------------------------------- //
    let versions = vec![
        version("1.27.1", true),
        version("1.22.0", false),
        version("1.20.14", false),
    ];
    assert!(resolve_version("1.22.0", &versions).is_some());
    assert!(resolve_version("1.20", &versions).is_some());
    // Component-boundary rule: `1.2` must NOT match `1.22.0`.
    assert!(resolve_version("1.2", &versions).is_none());
    assert!(resolve_version("1.99", &versions).is_none());

    let log = messages();
    let exact = log
        .iter()
        .find(|l| l.contains("query=\"1.22.0\""))
        .expect("a resolve query must be logged");
    assert!(
        exact.contains("candidates=3")
            && exact.contains("version=1.22.0")
            && exact.contains("kind=exact"),
        "resolve lines must carry query, candidate count, match and kind: {exact}"
    );
    let prefix = log
        .iter()
        .find(|l| l.contains("query=\"1.20\""))
        .expect("prefix query must be logged");
    assert!(
        prefix.contains("kind=prefix") && prefix.contains("version=1.20.14"),
        "got: {prefix}"
    );
    // Both kinds of miss are recorded with the candidate count, which is what
    // separates "typo" from "our manifest is stale/short".
    for query in ["1.99", "1.2"] {
        let miss = log
            .iter()
            .find(|l| l.contains(&format!("query=\"{query}\"")))
            .unwrap_or_else(|| panic!("a failed resolve must be logged too ({query})"));
        assert!(
            miss.starts_with("resolve: no match") && miss.contains("candidates=3"),
            "the miss must report the query and how many candidates existed: {miss}"
        );
    }

    // ---- 4. archive validation: one line, at the source --------------------- //
    // An HTML error page served instead of a tarball - the classic "curl works,
    // install fails" support case.
    let html: Vec<u8> = b"<html><body>404</body></html>".to_vec();
    let err = check_archive_magic(&html[..8], true, "https://dl.example/go.tar.gz")
        .expect_err("HTML must be rejected as a tar.gz");
    assert!(matches!(err, govmr::errors::GovmError::NotAnArchive { .. }));
    // A real gzip head must pass, and pass *silently* (no error line).
    check_archive_magic(
        &[0x1f, 0x8b, 0x08, 0x00],
        true,
        "https://dl.example/ok.tar.gz",
    )
    .expect("gzip magic accepted");

    let log = messages();
    let rejected = log
        .iter()
        .filter(|l| l.contains("archive: rejected"))
        .collect::<Vec<_>>();
    assert_eq!(
        rejected.len(),
        1,
        "a rejected payload must be logged exactly once, got: {rejected:#?}"
    );
    let rejected = rejected[0];
    assert!(
        rejected.contains("kind=tar.gz")
            && rejected.contains("expected=\"1f 8b\"")
            && rejected.contains("head=\"3c 68 74 6d")
            && rejected.contains("url=https://dl.example/go.tar.gz"),
        "the rejection must name kind, expected magic, actual bytes and URL: {rejected}"
    );
    assert!(
        !log.iter().any(|l| l.contains("install: failed")),
        "the validation layer must not also log the user-facing failure line"
    );

    // ---- 5. housekeeping invariants over the whole file --------------------- //
    let raw = fs::read_to_string(base.join("govmr.log")).expect("log file");
    for line in raw.lines() {
        assert!(
            line.is_ascii(),
            "log lines must stay plain ASCII so they are machine-parseable: {line}"
        );
        let level = line
            .get(21..26)
            .unwrap_or_else(|| panic!("line is missing the timestamp prefix: {line}"));
        assert!(
            matches!(level, "INFO " | "WARN " | "ERROR" | "DEBUG"),
            "unknown level tag {level:?} in: {line}"
        );
        let msg = line.splitn(3, ' ').nth(2).unwrap_or_default();
        assert!(
            msg.contains(": "),
            "every message must start with an `<area> <verb>: ` prefix, got: {msg}"
        );
    }
    // Regression for the duplicate-logging findings: two back-to-back identical
    // lines in one of these areas means two layers reported the same event (the
    // old `archive head:` x2 and `install failed:` x2). Scoping the check to
    // these areas keeps legitimately repeated lines - e.g. two `config: loaded`
    // when a test re-reads the file - from tripping it.
    for pair in messages().windows(2) {
        let area = pair[0].split(':').next().unwrap_or_default();
        if DEDUPED_AREAS.contains(&area) {
            assert_ne!(
                pair[0], pair[1],
                "duplicate `{area}` line logged twice by two layers"
            );
        }
    }

    cleanup(&home);
}

/// Minimal [`GoVersion`] for resolver tests (no network, no filesystem).
fn version(raw: &str, installed: bool) -> GoVersion {
    GoVersion {
        raw_version: raw.to_string(),
        display_name: format!("go{raw}"),
        filename: format!("go{raw}.linux-amd64.tar.gz"),
        url: format!("https://go.dev/dl/go{raw}.linux-amd64.tar.gz"),
        size: 70_553_950,
        installed,
        active: false,
        path: if installed {
            Some(PathBuf::from("/tmp/does-not-matter"))
        } else {
            None
        },
        stable: true,
    }
}

fn cleanup(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}
