//! Module modals - Centered modal overlays: theme picker, install progress, and delete confirmation.

use super::widgets::{centered_rect, clear_area, download_percent, spinner_span};
use crate::theme::ThemePickerView;
use crate::{
    app::{AppState, BusyState, Phase},
    theme::{Theme, ThemeFamily, ThemeName},
    version::GoVersion,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, List, ListItem, ListState, Paragraph},
};

// ------------------------------------- Public (crate) API ------------------------------------- //

/// Renders the color-theme picker as a two-level 📁 browser.
///
/// The top level shows the Dark / Light folders; opening one shows only the
/// themes inside it. The live dashboard behind the overlay previews whichever
/// theme is highlighted, so the effect of a choice is visible before saving.
pub(crate) fn render_theme_picker(
    frame: &mut Frame,
    screen: Rect,
    state: &AppState,
    theme: &Theme,
) {
    match state.theme_picker.view {
        ThemePickerView::Categories => render_picker_categories(frame, screen, state, theme),
        ThemePickerView::Family(family) => {
            render_picker_family(frame, screen, state, family, theme);
        }
    }
}

/// Renders the centered installation progress modal with a live gauge.
pub(crate) fn render_install_modal(
    frame: &mut Frame,
    screen: Rect,
    busy: &BusyState,
    tick: u64,
    theme: &Theme,
) {
    let BusyState::Installing {
        version,
        phase,
        downloaded,
        total,
        speed,
        ..
    } = busy
    else {
        return;
    };
    let area = centered_rect(62, 38, screen);
    clear_area(frame, area, theme);
    let block = Block::default()
        .title(Span::styled(
            format!(" Installing Go {version} "),
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
            render_downloading_phase(frame, &rows, tick, *downloaded, *total, *speed, theme);
        }
        Phase::Extracting => {
            render_extracting_phase(frame, &rows, tick, theme);
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
    let Some(target) = &state.confirming_delete else {
        return;
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
                format!("Go {target}"),
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

/// Renders the folder level: one row per [`ThemeFamily`] with its theme count.
fn render_picker_categories(frame: &mut Frame, screen: Rect, state: &AppState, theme: &Theme) {
    let area = centered_rect(56, 36, screen);
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
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(inner);

    let items: Vec<ListItem> = ThemeFamily::ALL
        .iter()
        .enumerate()
        .map(|(i, family)| {
            let selected = i == state.theme_picker.family_cursor;
            let count = ThemeName::in_family(*family).len();
            let marker = if selected { "❯" } else { " " };
            let marker_style = if selected {
                theme.brand_bold()
            } else {
                theme.muted()
            };
            let name_style = if selected {
                Style::default()
                    .fg(theme.brand)
                    .bg(theme.brand_dark)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.fg)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {marker} "), marker_style),
                Span::styled(format!(" {} ", family.icon()), theme.muted()),
                Span::styled(format!("{:<6}", family.label()), name_style),
                Span::styled(format!("  {count} themes  →"), theme.muted()),
            ]))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(theme.selected_row())
        .highlight_symbol("");
    let mut list_state = ListState::default();
    list_state.select(Some(state.theme_picker.family_cursor));
    frame.render_stateful_widget(list, chunks[0], &mut list_state);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ↑↓", theme.key_hint()),
            Span::styled(" pick  ", theme.muted()),
            Span::styled("enter", theme.key_hint()),
            Span::styled(" open  ", theme.muted()),
            Span::styled("esc", theme.key_hint()),
            Span::styled(" cancel ", theme.muted()),
        ]))
        .alignment(Alignment::Center),
        chunks[1],
    );
}

/// Renders the theme level: the themes of one family with live accent swatches.
fn render_picker_family(
    frame: &mut Frame,
    screen: Rect,
    state: &AppState,
    family: ThemeFamily,
    theme: &Theme,
) {
    let area = centered_rect(66, 78, screen);
    clear_area(frame, area, theme);
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" 🎨 Color Theme ", theme.title()),
            Span::styled(format!("· {} ", family.label()), theme.muted()),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border())
        .style(Style::default().bg(theme.bg));
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(inner);

    let items: Vec<ListItem> = ThemeName::in_family(family)
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let selected = i == state.theme_picker.theme_cursor;
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
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {marker} "), marker_style),
                Span::styled("███ ", Style::default().fg(swatch.brand)),
                Span::styled(format!("{:<20}", name.title()), name_style),
                Span::styled(format!("  {}", theme_tagline(*name)), theme.muted()),
            ]))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(theme.selected_row())
        .highlight_symbol("");
    let mut list_state = ListState::default();
    list_state.select(Some(state.theme_picker.theme_cursor));
    frame.render_stateful_widget(list, chunks[0], &mut list_state);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ↑↓", theme.key_hint()),
            Span::styled(" preview  ", theme.muted()),
            Span::styled("enter", theme.key_hint()),
            Span::styled(" save  ", theme.muted()),
            Span::styled("←/esc", theme.key_hint()),
            Span::styled(" back ", theme.muted()),
        ]))
        .alignment(Alignment::Center),
        chunks[1],
    );
}

/// Renders the downloading phase: phase line, progress gauge, and stats row.
fn render_downloading_phase(
    frame: &mut Frame,
    rows: &[Rect],
    tick: u64,
    downloaded: u64,
    total: u64,
    speed: f64,
    theme: &Theme,
) {
    let pct = download_percent(downloaded, total);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            spinner_span(tick, theme),
            Span::styled(" Downloading archive…", theme.highlight()),
        ])),
        rows[0],
    );
    // pct is clamped to 0..=100, so the cast always fits the gauge's u16.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let percent = pct as u16;
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
        .percent(percent)
        .label(Span::styled(
            format!(" {pct:.1}% "),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ));
    frame.render_widget(gauge, rows[1]);

    let eta = if speed > 1.0 && total > downloaded {
        // Remaining bytes fit f64 exactly for any realistic archive size.
        #[allow(clippy::cast_precision_loss)]
        let remaining_secs = (total - downloaded) as f64 / speed;
        format!("{remaining_secs:.0}s")
    } else {
        "—".to_string()
    };
    // Download speed is always non-negative; truncation drops at most a byte.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let speed_bps = speed as u64;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(
                    "  {} / {}   ",
                    GoVersion::format_size(downloaded),
                    GoVersion::format_size(total)
                ),
                theme.muted(),
            ),
            Span::styled(
                format!("{}/s   ", GoVersion::format_size(speed_bps)),
                theme.highlight(),
            ),
            Span::styled(format!("eta {eta}"), theme.muted()),
        ]))
        .alignment(Alignment::Center),
        rows[2],
    );
}

/// Renders the extracting phase: phase line and the pulsing unpacking gauge.
fn render_extracting_phase(frame: &mut Frame, rows: &[Rect], tick: u64, theme: &Theme) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            spinner_span(tick, theme),
            Span::styled(" Download complete — extracting archive…", theme.success()),
        ])),
        rows[0],
    );
    // THE PULSE: Smoothly oscillates between 20% and 80%.
    // Tick counts stay far below 2^53, so the f64 cast cannot lose precision.
    #[allow(clippy::cast_precision_loss)]
    let wave = (tick as f64 * 0.15).sin(); // Generates a smooth wave from -1.0 to 1.0
    // Maps the wave to a 20% - 80% range that always fits the gauge's u16.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let pct = (wave * 30.0 + 50.0) as u16;
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

/// Short descriptive tagline for each theme.
fn theme_tagline(name: ThemeName) -> &'static str {
    match name {
        ThemeName::GoCyan => "brand cyan default",
        ThemeName::JetBrainsNewIsland => "JetBrains island blue",
        ThemeName::CursorDark => "cursor indigo night",
        ThemeName::Midnight => "deep indigo, low glare",
        ThemeName::TokyoNight => "neon tokyo skyline",
        ThemeName::CatppuccinMocha => "cozy pastel espresso",
        ThemeName::Nord => "snowstorm blue",
        ThemeName::Dracula => "dark purple",
        ThemeName::GruvboxDark => "warm retro earth",
        ThemeName::RosePine => "muted dusky rose",
        ThemeName::Matrix => "retro phosphor green",
        ThemeName::Amber => "warm solarized glow",
        ThemeName::Mono => "minimal greyscale",
        ThemeName::CursorLight => "clean cursor indigo",
        ThemeName::CatppuccinLatte => "soft pastel milk",
        ThemeName::GitHubLight => "github paper white",
        ThemeName::SolarizedLight => "solarized parchment",
        ThemeName::RosePineDawn => "gentle morning rose",
        ThemeName::Light => "bright high contrast",
    }
}
