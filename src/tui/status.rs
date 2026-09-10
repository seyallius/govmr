//! Module status - Bottom status bar rendering: busy states, filter editing, and messages.

use super::widgets::{download_percent, spinner_span};
use crate::{
    app::{ActiveTab, AppState, BusyState, MsgKind, Phase, StatusMessage},
    theme::Theme,
    version::GoVersion,
};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

// ------------------------------------- Public (crate) API ------------------------------------- //

/// Renders the bottom status bar (busy progress, filter editing, or status messages).
pub(crate) fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(theme.muted());

    if state.filter_mode {
        let line = Line::from(vec![
            Span::styled(" 🔎 ", theme.brand_bold()),
            Span::styled("Filter: ", theme.muted()),
            Span::styled(state.filter.clone(), theme.brand_bold()),
            Span::styled("▏", Style::default().fg(theme.brand)),
        ]);
        frame.render_widget(Paragraph::new(line).block(block), area);
        return;
    }

    if let Some(busy) = &state.busy {
        let line = busy_line(busy, state.tick_count, theme);
        frame.render_widget(Paragraph::new(line).block(block), area);
        return;
    }

    if let Some(msg) = &state.status_message {
        let line = status_message_line(msg, theme);
        frame.render_widget(Paragraph::new(line).block(block), area);
        return;
    }

    frame.render_widget(
        Paragraph::new(idle_hint(state.active_tab, theme)).block(block),
        area,
    );
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Builds the single-line description of a busy state (progress, spinner, etc.).
fn busy_line(busy: &BusyState, tick: u64, theme: &Theme) -> Line<'static> {
    match busy {
        BusyState::Refreshing => Line::from(vec![
            spinner_span(tick, theme),
            Span::styled(" Fetching release manifest from go.dev…", theme.highlight()),
        ]),
        BusyState::Switching(v) => Line::from(vec![
            spinner_span(tick, theme),
            Span::styled(format!(" Switching to Go {v}…"), theme.highlight()),
        ]),
        BusyState::Deleting(v) => Line::from(vec![
            spinner_span(tick, theme),
            Span::styled(format!(" Removing Go {v}…"), theme.warning()),
        ]),
        BusyState::Installing {
            version,
            phase,
            downloaded,
            total,
            speed,
            ..
        }
        | BusyState::Updating {
            version,
            phase,
            downloaded,
            total,
            speed,
            ..
        } => {
            let pct = download_percent(*downloaded, *total);
            match phase {
                Phase::Downloading => {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let speed_bps = *speed as u64;
                    Line::from(vec![
                        spinner_span(tick, theme),
                        Span::styled(format!(" Downloading Go {version} "), theme.highlight()),
                        Span::styled(
                            format!(
                                "{:.0}%  ({}/{}) {}/s",
                                pct,
                                GoVersion::format_size(*downloaded),
                                GoVersion::format_size(*total),
                                GoVersion::format_size(speed_bps),
                            ),
                            theme.muted(),
                        ),
                    ])
                }
                Phase::Extracting => Line::from(vec![
                    spinner_span(tick, theme),
                    Span::styled(format!(" Unpacking Go {version}…"), theme.highlight()),
                ]),
            }
        }
    }
}

/// Builds the status line for a transient success/error/info message.
fn status_message_line(msg: &StatusMessage, theme: &Theme) -> Line<'static> {
    let (icon, style) = match msg.kind {
        MsgKind::Success => ("✓", theme.success()),
        MsgKind::Error => ("✗", theme.error()),
        MsgKind::Info => ("ℹ", theme.highlight()),
    };
    Line::from(vec![
        Span::styled(format!(" {icon} "), style.add_modifier(Modifier::BOLD)),
        Span::styled(msg.text.clone(), style),
    ])
}

/// The idle hint shown when nothing else is happening.
fn idle_hint(active_tab: ActiveTab, theme: &Theme) -> Line<'static> {
    let hint = match active_tab {
        ActiveTab::Available => {
            "Browse official Go releases — i installs, u activates, / searches, T themes."
        }
        ActiveTab::Installed => {
            "Your local toolchains — u activates, d removes, / searches, T themes."
        }
    };
    Line::from(vec![Span::styled(format!(" {hint}"), theme.muted())])
}
