//! Module modals - Centered modal overlays: theme picker, install progress, and delete confirmation.

use super::widgets::{centered_rect, clear_area, spinner_span};
use crate::{
    app::{AppState, BusyState, Phase},
    theme::{Theme, ThemeName},
    version::GoVersion,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, List, ListItem, ListState, Paragraph},
    Frame,
};

// ------------------------------------- Public (crate) API ------------------------------------- //

/// Renders the color-theme picker overlay with a live preview.
pub(crate) fn render_theme_picker(
    frame: &mut Frame,
    screen: Rect,
    state: &AppState,
    theme: &Theme,
) {
    let area = centered_rect(58, 62, screen);
    clear_area(frame, area, theme);

    let block = Block::default()
        .title(Span::styled(" 🎨 Color Theme ", theme.title()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border())
        .style(Style::default().bg(theme.bg));
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let rows: Vec<ListItem> = ThemeName::ALL
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let selected = i == state.theme_picker_index;
            let swatch = Theme::for_name(*name);
            let marker = if selected { "❯" } else { " " };
            let marker_style = if selected {
                theme.brand_bold()
            } else {
                theme.muted()
            };
            let name_style = if selected {
                Style::default()
                    .fg(swatch.brand)
                    .bg(theme.brand_dark)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.fg)
            };
            // A small colored block previews the theme's accent.
            Line::from(vec![
                Span::styled(format!(" {} ", marker), marker_style),
                Span::styled("███ ", Style::default().fg(swatch.brand)),
                Span::styled(format!("{:<14}", name.title()), name_style),
                Span::styled(format!("  {}", theme_tagline(*name)), theme.muted()),
            ])
            .into()
        })
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(inner);

    let list = List::new(rows)
        .highlight_style(theme.selected_row())
        .highlight_symbol("");
    let mut list_state = ListState::default();
    list_state.select(Some(state.theme_picker_index));
    frame.render_stateful_widget(list, chunks[0], &mut list_state);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ↑↓", theme.key_hint()),
            Span::styled(" preview  ", theme.muted()),
            Span::styled("enter", theme.key_hint()),
            Span::styled(" save  ", theme.muted()),
            Span::styled("esc", theme.key_hint()),
            Span::styled(" cancel ", theme.muted()),
        ]))
        .alignment(Alignment::Center),
        chunks[1],
    );
}

/// Renders the centered installation progress modal with a live gauge.
pub(crate) fn render_install_modal(
    frame: &mut Frame,
    screen: Rect,
    busy: &BusyState,
    tick: u64,
    theme: &Theme,
) {
    let (version, phase, downloaded, total, speed, _started_at) = match busy {
        BusyState::Installing {
            version,
            phase,
            downloaded,
            total,
            speed,
            started_at,
        } => (version, phase, downloaded, total, speed, started_at),
        _ => return,
    };

    let area = centered_rect(62, 38, screen);
    clear_area(frame, area, theme);

    let pct = if *total > 0 {
        (*downloaded as f64 / *total as f64 * 100.0).min(100.0)
    } else {
        0.0
    };

    let block = Block::default()
        .title(Span::styled(
            format!(" Installing Go {} ", version),
            theme.title(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border())
        .style(Style::default().bg(theme.bg));
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // phase line
            Constraint::Length(3), // gauge
            Constraint::Length(1), // stats
            Constraint::Min(1),
        ])
        .split(inner);

    match phase {
        Phase::Downloading => {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    spinner_span(tick, theme),
                    Span::styled(" Downloading archive…", theme.highlight()),
                ])),
                rows[0],
            );

            let gauge = Gauge::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(theme.muted()),
                )
                .gauge_style(
                    Style::default()
                        .fg(theme.brand)
                        .bg(theme.brand_dark)
                        .add_modifier(Modifier::BOLD),
                )
                .percent(pct as u16)
                .label(Span::styled(
                    format!(" {:.1}% ", pct),
                    Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
                ));
            frame.render_widget(gauge, rows[1]);

            let eta = if *speed > 1.0 && *total > *downloaded {
                format!("{:.0}s", (*total - *downloaded) as f64 / speed)
            } else {
                "—".to_string()
            };
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        format!(
                            "  {} / {}   ",
                            GoVersion::format_size(*downloaded),
                            GoVersion::format_size(*total)
                        ),
                        theme.muted(),
                    ),
                    Span::styled(
                        format!("{}/s   ", GoVersion::format_size(*speed as u64)),
                        theme.highlight(),
                    ),
                    Span::styled(format!("eta {}", eta), theme.muted()),
                ]))
                .alignment(Alignment::Center),
                rows[2],
            );
        }
        Phase::Extracting => {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    spinner_span(tick, theme),
                    Span::styled(" Download complete — extracting archive…", theme.success()),
                ])),
                rows[0],
            );

            // THE PULSE: Smoothly oscillates between 20% and 80%.
            let wave = (tick as f64 * 0.15).sin(); // Generates a smooth wave from -1.0 to 1.0
            let pct = (wave * 30.0 + 50.0) as u16; // Maps the wave to a 20% - 80% range

            let gauge = Gauge::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(theme.muted()),
                )
                .gauge_style(Style::default().fg(theme.success).bg(theme.brand_dark))
                .percent(pct)
                .label(Span::styled(
                    " unpacking ",
                    Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
                ));
            frame.render_widget(gauge, rows[1]);

            frame.render_widget(
                Paragraph::new(Span::styled(
                    "  This can take a few seconds for large toolchains.",
                    theme.muted(),
                ))
                .alignment(Alignment::Center),
                rows[2],
            );
        }
    }
}

/// Renders the destructive-action confirmation modal.
pub(crate) fn render_delete_modal(
    frame: &mut Frame,
    screen: Rect,
    state: &AppState,
    theme: &Theme,
) {
    let target = match &state.confirming_delete {
        Some(t) => t,
        None => return,
    };

    let area = centered_rect(58, 30, screen);
    clear_area(frame, area, theme);

    let block = Block::default()
        .title(Span::styled(" ⚠ Confirm Deletion ", theme.warning()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.error())
        .style(Style::default().bg(theme.bg));
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Remove ", theme.modal_body()),
            Span::styled(
                format!("Go {}", target),
                theme.error().add_modifier(Modifier::BOLD),
            ),
            Span::styled(" from your machine?", theme.modal_body()),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  This permanently deletes the toolchain directory.",
            theme.muted(),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("   [y] ", theme.error().add_modifier(Modifier::BOLD)),
            Span::styled("Yes, delete it     ", theme.muted()),
            Span::styled("[n/esc] ", theme.key_hint()),
            Span::styled("Cancel", theme.muted()),
        ]),
    ];
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Left), inner);
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Short descriptive tagline for each theme.
fn theme_tagline(name: ThemeName) -> &'static str {
    match name {
        ThemeName::GoCyan => "brand cyan default",
        ThemeName::Midnight => "deep indigo, low glare",
        ThemeName::Matrix => "retro phosphor green",
        ThemeName::Amber => "warm solarized glow",
        ThemeName::Nord => "snowstorm blue",
        ThemeName::Dracula => "dark purple",
        ThemeName::Light => "bright high contrast",
        ThemeName::Mono => "minimal greyscale",
    }
}
