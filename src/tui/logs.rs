//! Module logs - Docked, IDE-style operation log panel with live tailing,
//! focus-based key routing, follow mode, and optional word wrap.

use super::widgets::tilde_path;
use crate::{app::AppState, logging, theme::Theme};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::Modifier,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

// ------------------------------------- Public (crate) API ------------------------------------- //

/// Renders the operation log as a bottom-docked panel (IDE console style).
///
/// The panel stays live while the dashboard remains fully usable; pressing
/// `` ` `` moves keyboard focus into the panel for scroll/follow/wrap keys.
pub(crate) fn render_log_panel(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let focused = state.log_focus;
    let border_style = if focused {
        theme.border().add_modifier(Modifier::BOLD)
    } else {
        theme.dim_border()
    };

    let mut title = vec![
        Span::styled(" 📜 ", Style::default().fg(theme.brand)),
        Span::styled("Operation Log", theme.title()),
    ];
    if focused {
        title.push(Span::styled(" ─ focused ", theme.brand_bold()));
    }

    let block = Block::default()
        .title(Line::from(title))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
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

    render_log_lines(frame, chunks[0], state, theme);
    render_log_statusline(frame, chunks[1], state, theme);
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Renders the visible window of log lines, pinned to the newest entry.
///
/// When wrapping is enabled a logical line may occupy several terminal rows,
/// so the window budget is spent in *rendered rows* (unicode width ÷ panel
/// width) instead of raw line counts — keeping the anchor line on screen.
fn render_log_lines(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let total = state.log_lines.len();
    if total == 0 {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("∅ ", theme.muted()),
                Span::styled(
                    "No log entries yet — operations will appear here.",
                    theme.muted(),
                ),
            ]))
            .alignment(Alignment::Center),
            area,
        );
        return;
    }

    let width = area.width.max(1) as usize;
    let height = area.height as usize;
    let anchor_end = total.saturating_sub(state.log_scroll);

    // Walk backwards from the anchor, spending the row budget, so the newest
    // visible line always sits at the bottom of the panel.
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut used = 0usize;
    let mut idx = anchor_end;
    while idx > 0 && used < height {
        let line = colorize_log_line(&state.log_lines[idx - 1], theme);
        let cost = rendered_rows(&line, width, state.log_wrap);
        if !lines.is_empty() && used + cost > height {
            break; // including this line would push the anchor out of view
        }
        used += cost;
        lines.push(line);
        idx -= 1;
    }
    lines.reverse();

    let para = Paragraph::new(lines);
    let para = if state.log_wrap {
        para.wrap(Wrap { trim: false })
    } else {
        para
    };
    frame.render_widget(para, area);
}

/// Renders the panel's one-line status strip: follow/wrap toggles, scroll
/// offset, log path, and the keys that are active right now.
fn render_log_statusline(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let follow = if state.log_follow {
        Span::styled("● follow", theme.success())
    } else {
        Span::styled("○ follow", theme.warning())
    };
    let wrap = if state.log_wrap {
        Span::styled("● wrap", theme.success())
    } else {
        Span::styled("○ wrap", theme.muted())
    };
    let scrolled = if state.log_scroll > 0 {
        Span::styled(format!("↑{}", state.log_scroll), theme.warning())
    } else {
        Span::styled(" ", theme.muted())
    };
    let path = logging::default_log_path().map_or_else(
        || "~/.govmr/govmr.log".to_string(),
        |p| tilde_path(&p.to_string_lossy()),
    );
    let hints = if state.log_focus {
        " ↑↓ scroll · pgup/dn · g/G ends · f follow · w wrap · ` dashboard · L close"
    } else {
        " ` focus · L close"
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ", theme.muted()),
            follow,
            Span::styled("  ", theme.muted()),
            wrap,
            Span::styled("  ", theme.muted()),
            scrolled,
            Span::styled("  ·  ", theme.muted()),
            Span::styled(path, theme.brand_bold()),
            Span::styled(hints, theme.muted()),
        ])),
        area,
    );
}

/// Estimates how many terminal rows a styled log line will occupy.
fn rendered_rows(line: &Line<'_>, width: usize, wrap: bool) -> usize {
    if !wrap {
        return 1;
    }
    line.width().max(1).div_ceil(width)
}

/// Splits a log line into a dim timestamp and a level-colored remainder.
///
/// Expected format: `YYYY-MM-DD HH:MM:SSZ LEVEL message` (fixed 20-char ASCII
/// timestamp, a separating space, then the level tag).
fn colorize_log_line(line: &str, theme: &Theme) -> Line<'static> {
    let (ts, rest) = if line.len() > 21 {
        line.split_at(21)
    } else {
        ("", line)
    };
    let body_style = if rest.starts_with("ERROR") {
        theme.error()
    } else if rest.starts_with("WARN") {
        theme.warning()
    } else if rest.starts_with("DEBUG") {
        theme.muted()
    } else {
        theme.modal_body()
    };
    Line::from(vec![
        Span::styled(ts.to_string(), theme.muted()),
        Span::styled(rest.to_string(), body_style),
    ])
}
