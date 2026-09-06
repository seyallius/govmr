//! Module help - Right-docked, scrollable panel listing every keyboard command.
//!
//! The panel is a single-column reference catalog docked along the right edge of
//! the dashboard. It is informational and non-blocking: the dashboard stays
//! usable behind it, and its scroll offset is clamped here on every draw so a
//! long jump (`g`/`G`) can never leave the window past the last row.

use super::widgets::clear_area;
use crate::{
    app::{AppState, COMMAND_REFERENCE, HelpEntry},
    theme::Theme,
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

// ------------------------------------- Public (crate) API ------------------------------------- //

/// Renders the right-docked keyboard help panel over `area`.
///
/// Only the rows that fit `area`'s height are drawn, starting at the stored
/// scroll offset. The offset is clamped to a valid window here — after every
/// key event triggers a redraw this keeps `command_help_scroll` in range even
/// when a key jumped far past the end of the catalogue.
pub(crate) fn render_command_help(
    frame: &mut Frame,
    area: Rect,
    state: &mut AppState,
    theme: &Theme,
) {
    clear_area(frame, area, theme);
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" ? ", Style::default().fg(theme.brand)),
            Span::styled("Keyboard Help", theme.title()),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border())
        .style(Style::default().bg(theme.bg));
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let content = chunks[0];
    let total = COMMAND_REFERENCE.len();
    let visible = content.height as usize;
    // The last valid start index shows the final row at the bottom edge.
    let max_offset = total.saturating_sub(visible.max(1));
    if state.command_help_scroll > max_offset {
        state.command_help_scroll = max_offset;
    }
    let start = state.command_help_scroll;

    let lines: Vec<Line<'static>> = COMMAND_REFERENCE
        .iter()
        .skip(start)
        .take(visible)
        .flat_map(|entry| help_line(*entry, theme))
        .collect();

    frame.render_widget(Paragraph::new(lines), content);
    render_help_statusline(frame, chunks[1], state, content.height, theme);
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Renders the panel's one-line status strip, showing whether more rows exist
/// above or below the visible window plus the keys that operate the panel.
fn render_help_statusline(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    content_height: u16,
    theme: &Theme,
) {
    let inner = content_height.max(1) as usize;
    let total = COMMAND_REFERENCE.len();
    let has_above = state.command_help_scroll > 0;
    let has_below = state.command_help_scroll + inner < total;
    let footer = vec![
        Span::styled(" ", theme.muted()),
        scroll_arrow(has_above, "↑", theme),
        Span::styled(" j/k ", theme.muted()),
        scroll_arrow(has_below, "↓", theme),
        Span::styled(" · ", theme.muted()),
        Span::styled("? / esc", theme.key_hint()),
        Span::styled(" close", theme.muted()),
    ];
    frame.render_widget(Paragraph::new(Line::from(footer)), area);
}

/// A dim `glyph` when `show` is false, otherwise a highlighted scroll arrow.
fn scroll_arrow(show: bool, glyph: &'static str, theme: &Theme) -> Span<'static> {
    if show {
        Span::styled(glyph, theme.warning())
    } else {
        Span::styled(" ", theme.muted())
    }
}

/// Styles one command-reference row as one or more (non-wrapping) text lines.
fn help_line(entry: HelpEntry, theme: &Theme) -> Vec<Line<'static>> {
    match entry {
        HelpEntry::Section(title) => vec![
            Line::from(Span::styled(format!(" {title} "), theme.brand_bold())),
            Line::from(Span::styled("─".repeat(30), theme.dim_border())), // Full width separator
        ],
        HelpEntry::Binding { keys, action } => vec![Line::from(vec![
            Span::styled(format!("   {keys:<10}"), theme.key_hint()),
            Span::styled("  ", theme.muted()),
            Span::styled(action, theme.modal_body()),
        ])],
    }
}
