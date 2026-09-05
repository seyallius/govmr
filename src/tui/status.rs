//! Module status - Bottom status bar rendering: busy states, filter editing, and messages.

use super::widgets::{download_percent, spinner_span};
use crate::{
    app::{ActiveTab, AppState, BusyState, MsgKind, Phase},
    theme::Theme,
    version::GoVersion,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

// ---------------------------------------- Chrome pieces --------------------------------------- //

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
        let line = match busy {
            BusyState::Refreshing => Line::from(vec![
                spinner_span(state.tick_count, theme),
                Span::styled(" Fetching release manifest from go.dev…", theme.highlight()),
            ]),
            BusyState::Switching(v) => Line::from(vec![
                spinner_span(state.tick_count, theme),
                Span::styled(format!(" Switching to Go {v}…"), theme.highlight()),
            ]),
            BusyState::Deleting(v) => Line::from(vec![
                spinner_span(state.tick_count, theme),
                Span::styled(format!(" Removing Go {v}…"), theme.warning()),
            ]),
            BusyState::Installing {
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
                        // Download speed is always non-negative; truncation
                        // drops at most a single byte.
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        let speed_bps = *speed as u64;
                        Line::from(vec![
                            spinner_span(state.tick_count, theme),
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
                        spinner_span(state.tick_count, theme),
                        Span::styled(format!(" Unpacking Go {version}…"), theme.highlight()),
                    ]),
                }
            }
        };
        frame.render_widget(Paragraph::new(line).block(block), area);
        return;
    }

    if let Some(msg) = &state.status_message {
        let (icon, style) = match msg.kind {
            MsgKind::Success => ("✓", theme.success()),
            MsgKind::Error => ("✗", theme.error()),
            MsgKind::Info => ("ℹ", theme.highlight()),
        };
        let line = Line::from(vec![
            Span::styled(format!(" {icon} "), style.add_modifier(Modifier::BOLD)),
            Span::styled(msg.text.clone(), style),
        ]);
        frame.render_widget(Paragraph::new(line).block(block), area);
        return;
    }

    let hint = match state.active_tab {
        ActiveTab::Available => {
            "Browse official Go releases — i installs, u activates, / searches, T themes."
        }
        ActiveTab::Installed => {
            "Your local toolchains — u activates, d removes, / searches, T themes."
        }
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            format!(" {hint}"),
            theme.muted(),
        )]))
        .block(block),
        area,
    );
}
