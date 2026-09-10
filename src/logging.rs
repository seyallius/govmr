//! Module logging. Dependency-free operation logger that appends timestamped
//! entries to ~/.govmr/govmr.log (single-generation rotation), giving the TUI and CLI a
//! silent, post-mortem-friendly audit trail.
//!
//! # What a log line is for
//!
//! The bar is a *post-mortem*: given only `govmr.log`, a maintainer must be able
//! to answer (1) which govmr/OS/arch ran, (2) which operations were attempted
//! and in what order, (3) which succeeded or failed *and why*, and (4) what was
//! installed/active when the session ended. Every line exists to serve one of
//! those four questions.
//!
//! # Line format
//!
//! Each entry is rendered as `YYYY-MM-DD HH:MM:SSZ <LEVEL> <message>`, where the
//! message follows one convention so the file stays both human-readable and
//! machine-greppable:
//!
//! ```text
//! <area> <verb>: key=value key=value …
//! ```
//!
//! * `<area>` is the subsystem (`install`, `extract`, `shim`, `config`, `use`,
//!   `session`, …) and `<verb>` the lifecycle step (`started`, `complete`,
//!   `failed`, `rejected`, …). Grep the area to get one operation's story.
//! * Values are `key=value` pairs. Quote a value in double quotes whenever it
//!   can contain a space (paths, hex dumps, human sizes); never quote tokens
//!   that cannot (`version=`, `count=`, `ok=`).
//! * Plain ASCII only — no emoji. CLI stdout may use emoji; the log must not,
//!   so `grep`/`awk` pipelines keep working.
//! * An error is logged **once**, by the layer that *handles* it (the layer that
//!   shows it to the user). Code that merely returns an error stays silent, so
//!   a single failure never produces two lines. The exception is a line that
//!   carries context no other layer can see (e.g. the archive magic bytes).

use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

// --------------------------------- Types, Constants & Variables ------------------------------- //

/// Global log file handle. `None` means "logging disabled" (init failed or never
/// ran); every helper treats that as a silent no-op.
static LOGGER: OnceLock<Mutex<Option<File>>> = OnceLock::new();

/// Rotate the log once it grows past this size (1 MiB).
const MAX_LOG_BYTES: u64 = 1024 * 1024;

// ------------------------------------------ Types & Impls ------------------------------------- //

/// Severity of a log entry, rendered as a fixed-width tag.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Level {
    /// Routine operational events (install started, switch, refresh ok…).
    Info,
    /// Unusual but non-fatal situations.
    Warn,
    /// Failures of user-visible operations.
    Error,
    /// Verbose diagnostics (request URLs, archive header bytes…).
    Debug,
}
impl Level {
    /// Fixed-width (5 char) tag used in the line prefix.
    fn tag(self) -> &'static str {
        match self {
            Level::Info => "INFO ",
            Level::Warn => "WARN ",
            Level::Error => "ERROR",
            Level::Debug => "DEBUG",
        }
    }
}

// ------------------------------------- Public (crate) API ------------------------------------- //

/// Returns the default log file location: `~/.govmr/govmr.log`.
#[must_use]
pub(crate) fn default_log_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".govmr").join("govmr.log"))
}

/// Returns every line currently in the log file, oldest first.
///
/// Best-effort: yields an empty list when the log doesn't exist yet or can't
/// be read, which the TUI log viewer renders as "no entries yet".
#[must_use]
pub(crate) fn read_lines() -> Vec<String> {
    let Some(path) = default_log_path() else {
        return Vec::new();
    };
    fs::read_to_string(&path)
        .map(|contents| contents.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

/// Initializes the global logger at [`default_log_path`].
///
/// Best-effort by design: if the home dir can't be resolved or the file can't be
/// opened, logging simply stays disabled and the app runs unaffected.
pub(crate) fn init() {
    if let Some(path) = default_log_path() {
        init_in(&path);
    }
}

/// Initializes the global logger with an explicit file path (also used by tests).
///
/// First call wins: rotates an oversized existing log to `<path>.old`, then opens
/// the file in append mode. Later calls are ignored.
pub(crate) fn init_in(path: &Path) {
    LOGGER.get_or_init(|| Mutex::new(open_log(path)));
}

/// Moves an oversized log file aside so a fresh one can start.
///
/// `<path>` becomes `<path>.old` (overwriting any previous rotation). Files at or
/// under `MAX_LOG_BYTES` are left untouched.
pub(crate) fn rotate_if_oversized(path: &Path) {
    if let Ok(meta) = fs::metadata(path)
        && meta.len() > MAX_LOG_BYTES
    {
        let old = path.with_extension("log.old");
        let _ = fs::rename(path, old);
    }
}

/// Appends one timestamped line to the log.
///
/// Never panics and swallows all IO failures: a logger must not take the app down
/// with it. No-op when the logger was never (successfully) initialized.
pub(crate) fn log(level: Level, message: &str) {
    let Some(logger) = LOGGER.get() else { return };
    let Ok(mut guard) = logger.lock() else { return };
    let Some(file) = guard.as_mut() else { return };
    let line = format!("{} {} {}\n", timestamp(), level.tag(), message);
    let _ = file.write_all(line.as_bytes());
    let _ = file.flush();
}

/// Returns the message part of a rendered line, i.e. drops the
/// `YYYY-MM-DD HH:MM:SSZ <LEVEL> ` prefix.
///
/// Public because anything that reasons *about* the trail — the TUI log panel's
/// highlighting, the tests, a future `govmr logs --grep` — needs the message
/// without re-parsing the framing.
#[allow(dead_code)]
pub(crate) fn message_of(line: &str) -> &str {
    let rest = line.get(TIMESTAMP_LEN..).unwrap_or(line);
    for tag in [
        Level::Info.tag(),
        Level::Warn.tag(),
        Level::Error.tag(),
        Level::Debug.tag(),
    ] {
        if let Some(msg) = rest.strip_prefix(tag) {
            return msg.trim_start();
        }
    }
    rest
}

/// Length of the `YYYY-MM-DD HH:MM:SSZ ` prefix that opens every line.
#[allow(dead_code)]
const TIMESTAMP_LEN: usize = 21;

/// Logs a routine operational event.
pub(crate) fn info(message: &str) {
    log(Level::Info, message);
}

/// Logs an unusual but non-fatal situation.
pub(crate) fn warn(message: &str) {
    log(Level::Warn, message);
}

/// Logs a failed operation.
pub(crate) fn error(message: &str) {
    log(Level::Error, message);
}

/// Logs verbose diagnostics for debugging.
pub(crate) fn debug(message: &str) {
    log(Level::Debug, message);
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Rotates if needed, then opens the log file in append mode, creating it and any
/// missing parent directories. Returns `None` on failure (logging disabled).
fn open_log(path: &Path) -> Option<File> {
    rotate_if_oversized(path);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}

/// Renders *now* (UTC) as `YYYY-MM-DD HH:MM:SSZ` without pulling in a date crate.
fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        // Saturates instead of wrapping; epoch seconds never reach i64::MAX.
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    format_unix(secs)
}

/// Converts Unix epoch seconds to a civil calendar string (UTC).
///
/// Uses Howard Hinnant's public-domain `civil_from_days` algorithm so we stay
/// dependency-free while producing human-friendly timestamps.
fn format_unix(total_secs: i64) -> String {
    let days = total_secs.div_euclid(86_400);
    let secs = total_secs.rem_euclid(86_400);
    let (hour, minute, second) = (secs / 3600, (secs % 3600) / 60, secs % 60);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let base_year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { base_year + 1 } else { base_year };

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}Z")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
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
        init_in(&path); // first (and only) global init in this test binary
        info("operation: install go1.22.0");
        error("operation failed: invalid gzip header");

        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("INFO  operation: install go1.22.0"));
        assert!(raw.contains("ERROR operation failed: invalid gzip header"));

        // Timestamp shape: "YYYY-MM-DD HH:MM:SSZ "
        let first = raw.lines().next().unwrap();
        assert_eq!(&first[4..5], "-");
        assert_eq!(&first[7..8], "-");
        assert_eq!(&first[10..11], " ");
        assert_eq!(&first[13..14], ":");
        assert_eq!(first.get(19..20), Some("Z"));
    }

    #[test]
    fn rotation_moves_oversized_log_aside() {
        let dir = temp_dir();
        let path = dir.join("govmr.log");
        fs::write(&path, vec![b'x'; 1024 * 1024 + 1]).unwrap();

        rotate_if_oversized(&path);

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

        rotate_if_oversized(&path);

        assert!(path.exists(), "small logs must not rotate");
    }
}
