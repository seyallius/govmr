//! Module logging. Dependency-free operation logger that appends timestamped
//! entries to ~/.govmr/govmr.log (single-generation rotation), giving the TUI and CLI a
//! silent, post-mortem-friendly audit trail.

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
pub enum Level {
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

// ----------------------------------------- Public API ----------------------------------------- //

/// Returns the default log file location: `~/.govmr/govmr.log`.
pub fn default_log_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".govmr").join("govmr.log"))
}

/// Returns every line currently in the log file, oldest first.
///
/// Best-effort: yields an empty list when the log doesn't exist yet or can't
/// be read, which the TUI log viewer renders as "no entries yet".
pub fn read_lines() -> Vec<String> {
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
pub fn init() {
    if let Some(path) = default_log_path() {
        init_in(&path);
    }
}

/// Initializes the global logger with an explicit file path (also used by tests).
///
/// First call wins: rotates an oversized existing log to `<path>.old`, then opens
/// the file in append mode. Later calls are ignored.
pub fn init_in(path: &Path) {
    LOGGER.get_or_init(|| Mutex::new(open_log(path)));
}

/// Moves an oversized log file aside so a fresh one can start.
///
/// `<path>` becomes `<path>.old` (overwriting any previous rotation). Files at or
/// under [`MAX_LOG_BYTES`] are left untouched.
pub fn rotate_if_oversized(path: &Path) {
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() > MAX_LOG_BYTES {
            let old = path.with_extension("log.old");
            let _ = fs::rename(path, old);
        }
    }
}

/// Appends one timestamped line to the log.
///
/// Never panics and swallows all IO failures: a logger must not take the app down
/// with it. No-op when the logger was never (successfully) initialized.
pub fn log(level: Level, message: &str) {
    let Some(logger) = LOGGER.get() else { return };
    let Ok(mut guard) = logger.lock() else { return };
    let Some(file) = guard.as_mut() else { return };
    let line = format!("{} {} {}\n", timestamp(), level.tag(), message);
    let _ = file.write_all(line.as_bytes());
    let _ = file.flush();
}

/// Logs a routine operational event.
pub fn info(message: &str) {
    log(Level::Info, message)
}

/// Logs an unusual but non-fatal situation.
pub fn warn(message: &str) {
    log(Level::Warn, message)
}

/// Logs a failed operation.
pub fn error(message: &str) {
    log(Level::Error, message)
}

/// Logs verbose diagnostics for debugging.
pub fn debug(message: &str) {
    log(Level::Debug, message)
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
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    format_unix(secs)
}

/// Converts Unix epoch seconds to a civil calendar string (UTC).
///
/// Uses Howard Hinnant's public-domain `civil_from_days` algorithm so we stay
/// dependency-free while producing human-friendly timestamps.
fn format_unix(total_secs: i64) -> String {
    let days = total_secs.div_euclid(86_400);
    let secs = total_secs.rem_euclid(86_400);
    let (h, mi, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let base_year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { base_year + 1 } else { base_year };

    format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02}Z")
}
