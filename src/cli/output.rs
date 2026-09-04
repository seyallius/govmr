//! Module output - Minimal ANSI color helpers for CLI command output.
//!
//! The escape sequences here are hardcoded on purpose: the CLI is the only
//! place that needs them, and pulling in a color crate for 7 constants would
//! be pure overhead.

// --------------------------------- Types, Constants & Variables ------------------------------- //

pub(crate) const CYAN: &str = "\x1b[36m";
pub(crate) const GREEN: &str = "\x1b[32m";
pub(crate) const RED: &str = "\x1b[31m";
pub(crate) const YELLOW: &str = "\x1b[33m";
pub(crate) const GREY: &str = "\x1b[90m";
pub(crate) const BOLD: &str = "\x1b[1m";
pub(crate) const RESET: &str = "\x1b[0m";

// ------------------------------------- Public (crate) API ------------------------------------- //

/// Wraps `text` in a bold, colored span and resets the terminal afterwards.
pub(crate) fn paint(color: &str, text: &str) -> String {
    format!("{}{}{}{}", color, BOLD, text, RESET)
}
