//! Module logging. Verifies the operation logger writes formatted,

// Tests exist to fail loudly: panicking on a broken setup or a failed helper
// is exactly the desired behavior, so unwraps are idiomatic here.
#![allow(clippy::unwrap_used)]
//! timestamped lines and rotates oversized logs, using throwaway temp directories.

use govmr::logging;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Unique scratch directory per test (same pattern as tests/config.rs).
fn temp_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "govmr-log-test-{}-{}-{}",
        std::process::id(),
        n,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ----------------------------------------- Tests ----------------------------------------- //

#[test]
fn writes_timestamped_level_lines() {
    let path = temp_dir().join("govmr.log");
    logging::init_in(&path); // first (and only) global init in this test binary
    logging::info("operation: install go1.22.0");
    logging::error("operation failed: invalid gzip header");

    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(raw.contains("INFO  operation: install go1.22.0"));
    assert!(raw.contains("ERROR operation failed: invalid gzip header"));

    // Timestamp shape: "YYYY-MM-DD HH:MM:SSZ "
    let first = raw.lines().next().unwrap();
    assert_eq!(&first[4..5], "-");
    assert_eq!(&first[7..8], "-");
    assert_eq!(&first[10..11], " ");
    assert_eq!(&first[13..14], ":");
    assert!(first.get(19..20) == Some("Z"));
}

#[test]
fn rotation_moves_oversized_log_aside() {
    let dir = temp_dir();
    let path = dir.join("govmr.log");
    std::fs::write(&path, vec![b'x'; 1024 * 1024 + 1]).unwrap();

    logging::rotate_if_oversized(&path);

    assert!(!path.exists(), "oversized log must be rotated away");
    assert!(
        dir.join("govmr.log.old").exists(),
        "rotation target missing"
    );
}

#[test]
fn rotation_leaves_small_logs_alone() {
    let dir = temp_dir();
    let path = dir.join("govmr.log");
    std::fs::write(&path, "tiny log\n").unwrap();

    logging::rotate_if_oversized(&path);

    assert!(path.exists(), "small logs must not rotate");
}
