//! Module widgets - Shared rendering primitives for TUI views.

use crate::theme::Theme;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Clear},
};

// --------------------------------- Types, Constants & Variables ------------------------------- //

/// Braille spinner frames cycled through by [`spinner_frame`].
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

// ------------------------------------- Public (crate) API ------------------------------------- //

/// Fills a modal area with the theme background so content beneath is wiped
/// (this is the theme-aware equivalent of `Clear` and keeps light schemes solid).
///
/// `Clear` first resets every cell of the area to a blank space, wiping any
/// dashboard glyphs sitting behind the modal; the bg-only `Block` then paints
/// the theme background on top. A bg-only `Block` alone would merely *restyle*
/// the existing cells, leaving stale symbols visible as noise (the ghost text
/// that bled through the theme picker's margins and empty spaces).
pub(crate) fn clear_area(frame: &mut Frame, area: Rect, theme: &Theme) {
    // 1) Erase symbols (Block::render only sets styles, never clears glyphs).
    frame.render_widget(Clear, area);
    // 2) Paint the theme background so light schemes stay solid.
    frame.render_widget(Block::default().style(Style::default().bg(theme.bg)), area);
}

/// A styled, animated spinner span (with a leading space).
pub(crate) fn spinner_span(tick: u64, theme: &Theme) -> Span<'static> {
    Span::styled(format!(" {} ", spinner_frame(tick)), theme.brand_bold())
}

/// Right-pads a line of spans so the selection highlight fills most of the row.
/// The padding deliberately stops short of the full width so it cannot paint
/// over a modal border rendered on top.
pub(crate) fn right_pad(mut spans: Vec<Span<'static>>, _width: u16) -> Line<'static> {
    spans.push(Span::raw(" ".repeat(120)));
    Line::from(spans)
}

/// Replaces a user's home directory prefix with `~`.
pub(crate) fn tilde_path(path: &str) -> String {
    match dirs::home_dir() {
        Some(home) => {
            let home = home.to_string_lossy();
            if let Some(rest) = path.strip_prefix(home.as_ref()) {
                format!("~{rest}")
            } else {
                path.to_string()
            }
        }
        None => path.to_string(),
    }
}

/// Percentage of a download completed, clamped to `0.0..=100.0`, or `0.0`
/// when the total size is unknown.
pub(crate) fn download_percent(downloaded: u64, total: u64) -> f64 {
    // Byte counters stay far below 2^53 for any realistic archive, so the
    // f64 casts cannot lose precision in practice.
    #[allow(clippy::cast_precision_loss)]
    let pct = if total > 0 {
        (downloaded as f64 / total as f64 * 100.0).min(100.0)
    } else {
        0.0
    };
    pct
}

/// Truncates a path from the left (keeping the tail) if it exceeds `max` characters.
pub(crate) fn shorten_path(path: &str, max: usize) -> String {
    let chars: Vec<char> = path.chars().collect();
    if chars.len() <= max {
        path.to_string()
    } else {
        let tail: String = chars[chars.len() - (max - 1)..].iter().collect();
        format!("…{tail}")
    }
}

/// Helper function to calculate a centered rectangle area for modal overlays.
pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Returns the current braille spinner glyph for the given animation tick.
fn spinner_frame(tick: u64) -> &'static str {
    // The modulo keeps the index in bounds regardless of pointer width.
    #[allow(clippy::cast_possible_truncation)]
    let idx = (tick as usize) % SPINNER.len();
    SPINNER[idx]
}
