//! Tests for TOML config persistence and legacy migration.

// Tests exist to fail loudly: panicking on a broken setup or a failed helper
// is exactly the desired behavior, so unwraps are idiomatic here.
#![allow(clippy::unwrap_used)]

use govmr::config::Config;
use govmr::theme::ThemeName;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "govmr-cfg-test-{}-{}-{}",
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

#[test]
fn defaults_to_gocyan_when_absent() {
    let dir = temp_dir();
    let cfg = Config::load(&dir);
    assert_eq!(cfg.theme, ThemeName::GoCyan);
}

#[test]
fn saves_and_reloads_theme_via_toml() {
    let dir = temp_dir();
    {
        let mut cfg = Config::load(&dir);
        cfg.set_theme(ThemeName::Nord).unwrap();
    }
    // The file must exist with a .toml extension and contain TOML.
    let toml_path = dir.join("config.toml");
    let raw = std::fs::read_to_string(&toml_path).unwrap();
    assert!(raw.contains("theme"), "toml body: {raw}");
    assert!(raw.contains("nord"), "toml body: {raw}");

    // Reload picks it up.
    let cfg = Config::load(&dir);
    assert_eq!(cfg.theme, ThemeName::Nord);
}

#[test]
fn migrates_legacy_plain_text_config() {
    let dir = temp_dir();
    std::fs::write(dir.join("config"), "theme = midnight\n").unwrap();
    let cfg = Config::load(&dir);
    assert_eq!(cfg.theme, ThemeName::Midnight);
}

#[test]
fn corrupt_toml_falls_back_to_default() {
    let dir = temp_dir();
    std::fs::write(dir.join("config.toml"), "this is = = not valid toml [").unwrap();
    let cfg = Config::load(&dir);
    assert_eq!(cfg.theme, ThemeName::GoCyan);
}
