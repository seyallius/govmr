//! Copyright (c) 2026 SeyedAli
//! Licensed under the MIT License. See LICENSE file in the project root for details.
//
//! Module views - Main layout composition, widget rendering, and modal views for Ratatui.

use super::setup::draw_setup_modal;
use crate::app::{visible_indices, ActiveTab, AppState, BusyState, MsgKind, Phase};
use crate::models::GoVersion;
use crate::theme::{Theme, ThemeName};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Tabs,
    },
};

/// Braille spinner frames cycled through by [`spinner_frame`].
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

// ----------------------------------------- Public API ----------------------------------------- //

/// Primary render routine for the GoVMR dashboard interface.
///
/// # Arguments
/// * `frame` - Mutable drawing frame provided by Ratatui.
/// * `state` - Current mutable application state.
pub fn render(frame: &mut Frame, state: &mut AppState) {
    state.tick_count = state.tick_count.wrapping_add(1);
    let size = frame.area();
    let theme = state.theme;

    // ---- Outer branded container ------------------------------------------------------------ //
    let title = Line::from(vec![
        Span::styled(" 🔧 ", Style::default().fg(theme.brand)),
        Span::styled("GoVMR", theme.brand_bold()),
        Span::styled(" — Go Version Manager ", theme.muted()),
    ]);

    let right_title = state
        .versions
        .iter()
        .find(|v| v.active)
        .map(|v| {
            Line::from(vec![
                Span::styled(" active: ", theme.muted()),
                Span::styled(v.display_name.clone(), theme.badge_active()),
                Span::raw("  "),
            ])
        })
        .unwrap_or_else(|| Line::from(Span::raw("")));

    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border())
        .title(title)
        .title_alignment(Alignment::Left)
        .title(right_title)
        .title_alignment(Alignment::Right);
    frame.render_widget(main_block, size);

    let inner = size.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    // ---- Vertical layout --------------------------------------------------------------------- //
    let show_warning = !state.is_shim_in_path;
    let mut constraints = Vec::with_capacity(5);
    if show_warning {
        constraints.push(Constraint::Length(3)); // PATH warning banner
    }
    constraints.push(Constraint::Length(3)); // Tabs
    constraints.push(Constraint::Min(5)); // Main content
    constraints.push(Constraint::Length(3)); // Status bar
    constraints.push(Constraint::Length(1)); // Help footer

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    // A centered modal covers the chrome areas; hide the pieces that would
    // otherwise bleed through the modal edges.
    let modal_active = matches!(state.busy, Some(BusyState::Installing { .. }))
        || state.confirming_delete.is_some()
        || state.show_help
        || state.show_theme_picker;

    let mut idx = 0;
    if show_warning && !modal_active {
        render_warning(frame, chunks[idx], &theme);
        idx += 1;
    }
    let tabs_chunk = chunks[idx];
    idx += 1;
    let content_chunk = chunks[idx];
    idx += 1;
    let status_chunk = chunks[idx];
    idx += 1;
    let footer_chunk = chunks[idx];

    if !modal_active {
        render_tabs(frame, tabs_chunk, state, &theme);
    }
    render_content(frame, content_chunk, state, &theme);
    if !modal_active {
        render_status_bar(frame, status_chunk, state, &theme);
        render_footer(frame, footer_chunk, state, &theme);
    }
}

/// Draws top-level modal overlays (theme picker, install progress, delete, help).
pub fn render_overlays(frame: &mut Frame, state: &AppState) {
    let size = frame.area();
    let theme = state.theme;

    if let Some(busy) = &state.busy {
        if matches!(busy, BusyState::Installing { .. }) {
            render_install_modal(frame, size, busy, state.tick_count, &theme);
        }
    }

    if state.confirming_delete.is_some() {
        render_delete_modal(frame, size, state, &theme);
    }

    if state.show_help {
        draw_setup_modal(frame, size, &state.shim_path, &theme);
    }

    if state.show_theme_picker {
        render_theme_picker(frame, size, state, &theme);
    }
}

// ---------------------------------------- Chrome pieces --------------------------------------- //

/// Renders the amber PATH-warning banner.
fn render_warning(frame: &mut Frame, area: Rect, theme: &Theme) {
    let banner = Paragraph::new(Line::from(vec![
        Span::styled(" ⚠ ", theme.warning().add_modifier(Modifier::BOLD)),
        Span::styled(" GoVMR shim is not on your PATH — press ", theme.warning()),
        Span::styled("h", theme.key_hint()),
        Span::styled(" for setup help.", theme.warning()),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme.warning()),
    );
    frame.render_widget(banner, area);
}

/// Renders the tab strip with per-tab counts.
fn render_tabs(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let installed_count = state.versions.iter().filter(|v| v.installed).count();
    let available_count = state.versions.len();

    let tab = |name: &'static str, count: usize, active: bool| {
        let style = if active {
            theme.tab_active()
        } else {
            theme.tab_inactive()
        };
        Line::from(vec![
            Span::styled(if active { "● " } else { "○ " }, style),
            Span::styled(name.to_string(), style),
            Span::styled(
                format!(" ({})", count),
                if active {
                    theme.badge_active()
                } else {
                    theme.muted()
                },
            ),
        ])
    };

    let titles = vec![
        tab("Available", available_count, state.active_tab == ActiveTab::Available),
        tab("Installed", installed_count, state.active_tab == ActiveTab::Installed),
    ];

    let tabs = Tabs::new(titles)
        .select(match state.active_tab {
            ActiveTab::Available => 0,
            ActiveTab::Installed => 1,
        })
        .divider(Span::styled("│", theme.muted()))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(theme.border()),
        )
        .highlight_style(theme.brand_bold());
    frame.render_widget(tabs, area);
}

/// Renders the active tab's content list.
fn render_content(frame: &mut Frame, area: Rect, state: &mut AppState, theme: &Theme) {
    let visible: Vec<usize> = visible_indices(state);

    // While a modal is on screen, hide the list entirely so padded rows and
    // borders can't bleed through the modal (the modal clears its own area).
    let modal_up = matches!(state.busy, Some(BusyState::Installing { .. }))
        || state.confirming_delete.is_some()
        || state.show_help
        || state.show_theme_picker;
    if modal_up {
        return;
    }

    if visible.is_empty() {
        let msg = if state.filter.is_empty() {
            match state.active_tab {
                ActiveTab::Available => "No versions available.",
                ActiveTab::Installed => "No Go versions installed yet — press i to install one.",
            }
        } else {
            "No versions match your filter."
        };
        let empty = Paragraph::new(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("∅ ", theme.muted()),
            Span::styled(msg, theme.muted()),
        ]))
        .block(content_block(theme));
        frame.render_widget(empty, area);
        return;
    }

    // Usable text width inside the surrounding border block.
    let inner_width = area.width.saturating_sub(2);

    let items: Vec<ListItem> = visible
        .iter()
        .map(|&i| {
            let v = &state.versions[i];
            let line = match state.active_tab {
                ActiveTab::Available => available_line(v, inner_width, theme),
                ActiveTab::Installed => installed_line(v, inner_width, theme),
            };
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(content_block(theme))
        .highlight_style(theme.selected_row())
        .highlight_symbol(" ❯ ");

    let mut list_state: ListState = ListState::default();
    list_state.select(state.list_state.selected());
    frame.render_stateful_widget(list, area, &mut list_state);
}

/// Builds one row for the *Available* tab.
fn available_line(v: &GoVersion, width: u16, theme: &Theme) -> Line<'static> {
    let mut spans = vec![
        Span::styled(
            format!(" {:<10}", v.display_name),
            Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
        ),
        Span::styled(format!("{:>8}", GoVersion::format_size(v.size)), theme.muted()),
        Span::raw("  "),
    ];

    match GoVersion::prerelease_tag(&v.raw_version) {
        Some(tag) if !v.stable => {
            spans.push(Span::styled(format!("[{}]", tag), theme.badge_unstable()));
        }
        _ => {
            spans.push(Span::styled("[stable]", theme.success()));
        }
    }

    spans.push(Span::raw("  "));
    if v.active {
        spans.push(Span::styled("● active", theme.badge_active()));
    } else if v.installed {
        spans.push(Span::styled("✓ installed", theme.badge_installed()));
    } else {
        spans.push(Span::styled("· available", theme.muted()));
    }

    right_pad(spans, width)
}

/// Builds one row for the *Installed* tab.
fn installed_line(v: &GoVersion, width: u16, theme: &Theme) -> Line<'static> {
    let path = v
        .path
        .as_ref()
        .map(|p| tilde_path(&p.to_string_lossy()))
        .unwrap_or_default();

    let mut spans = vec![
        Span::styled(
            format!(" {:<10}", v.display_name),
            Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
        ),
        Span::raw(" "),
    ];

    if v.active {
        spans.push(Span::styled("● active  ", theme.badge_active()));
    } else {
        spans.push(Span::styled("  ready   ", theme.badge_installed()));
    }

    spans.push(Span::styled(shorten_path(&path, 34), theme.muted()));
    right_pad(spans, width)
}

/// The shared rounded border block used behind the content lists.
fn content_block(theme: &Theme) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.brand_dark))
}

/// Renders the bottom status bar (busy progress, filter editing, or status messages).
fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
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
                Span::styled(format!(" Switching to Go {}…", v), theme.highlight()),
            ]),
            BusyState::Deleting(v) => Line::from(vec![
                spinner_span(state.tick_count, theme),
                Span::styled(format!(" Removing Go {}…", v), theme.warning()),
            ]),
            BusyState::Installing {
                version,
                phase,
                downloaded,
                total,
                speed,
                ..
            } => {
                let pct = if *total > 0 {
                    (*downloaded as f64 / *total as f64 * 100.0).min(100.0)
                } else {
                    0.0
                };
                match phase {
                    Phase::Downloading => Line::from(vec![
                        spinner_span(state.tick_count, theme),
                        Span::styled(format!(" Downloading Go {} ", version), theme.highlight()),
                        Span::styled(
                            format!(
                                "{:.0}%  ({}/{}) {}/s",
                                pct,
                                GoVersion::format_size(*downloaded),
                                GoVersion::format_size(*total),
                                GoVersion::format_size(*speed as u64),
                            ),
                            theme.muted(),
                        ),
                    ]),
                    Phase::Extracting => Line::from(vec![
                        spinner_span(state.tick_count, theme),
                        Span::styled(format!(" Unpacking Go {}…", version), theme.highlight()),
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
            Span::styled(format!(" {} ", icon), style.add_modifier(Modifier::BOLD)),
            Span::styled(msg.text.clone(), style),
        ]);
        frame.render_widget(Paragraph::new(line).block(block), area);
        return;
    }

    let hint = match state.active_tab {
        ActiveTab::Available => "Browse official Go releases — i installs, u activates, / searches, T themes.",
        ActiveTab::Installed => "Your local toolchains — u activates, d removes, / searches, T themes.",
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(format!(" {}", hint), theme.muted())]))
            .block(block),
        area,
    );
}

/// Renders the keyboard-shortcut footer.
fn render_footer(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let hint = |key: &'static str, label: &'static str| {
        vec![
            Span::styled(format!(" {} ", key), theme.key_hint()),
            Span::styled(format!("{} ", label), theme.muted()),
        ]
    };

    let mut spans = Vec::new();
    if state.filter_mode {
        spans.extend(hint("enter", "apply"));
        spans.extend(hint("esc", "clear"));
    } else {
        spans.extend(hint("↑↓/jk", "move"));
        spans.extend(hint("tab", "switch"));
        spans.extend(hint("/", "filter"));
        spans.extend(hint("i", "install"));
        spans.extend(hint("u", "use"));
        spans.extend(hint("d", "delete"));
        spans.extend(hint("T", "theme"));
        spans.extend(hint("r", "refresh"));
        spans.extend(hint("q", "quit"));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        area,
    );
}

// ------------------------------------------ Modals -------------------------------------------- //

/// Renders the color-theme picker overlay with a live preview.
fn render_theme_picker(frame: &mut Frame, screen: Rect, state: &AppState, theme: &Theme) {
    let area = centered_rect(54, 56, screen);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(" 🎨 Color Theme ", theme.title()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border());
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
                Span::styled(
                    format!("  {}", theme_tagline(*name)),
                    if selected {
                        theme.muted()
                    } else {
                        theme.muted()
                    },
                ),
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

/// Short descriptive tagline for each theme.
fn theme_tagline(name: ThemeName) -> &'static str {
    match name {
        ThemeName::GoCyan => "brand cyan default",
        ThemeName::Midnight => "deep indigo, low glare",
        ThemeName::Matrix => "retro phosphor green",
        ThemeName::Amber => "warm solarized glow",
        ThemeName::Mono => "minimal greyscale",
    }
}

/// Renders the centered installation progress modal with a live gauge.
fn render_install_modal(frame: &mut Frame, screen: Rect, busy: &BusyState, tick: u64, theme: &Theme) {
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
    frame.render_widget(Clear, area);

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
        .border_style(theme.border());
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

            // Indeterminate "sweeping" gauge driven by the animation tick.
            let sweep = ((tick as f64 * 0.15).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
            let gauge = Gauge::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(theme.muted()),
                )
                .gauge_style(Style::default().fg(theme.success).bg(theme.brand_dark))
                .percent((sweep * 100.0) as u16)
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
fn render_delete_modal(frame: &mut Frame, screen: Rect, state: &AppState, theme: &Theme) {
    let target = match &state.confirming_delete {
        Some(t) => t,
        None => return,
    };

    let area = centered_rect(58, 30, screen);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(" ⚠ Confirm Deletion ", theme.warning()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.error());
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Remove ", theme.modal_body()),
            Span::styled(format!("Go {}", target), theme.error().add_modifier(Modifier::BOLD)),
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

/// Returns the current braille spinner glyph for the given animation tick.
fn spinner_frame(tick: u64) -> &'static str {
    SPINNER[(tick as usize) % SPINNER.len()]
}

/// A styled, animated spinner span (with a leading space).
fn spinner_span(tick: u64, theme: &Theme) -> Span<'static> {
    Span::styled(format!(" {} ", spinner_frame(tick)), theme.brand_bold())
}

/// Right-pads a line of spans so the selection highlight fills most of the row.
/// The padding deliberately stops short of the full width so it cannot paint
/// over a modal border rendered on top.
fn right_pad(mut spans: Vec<Span<'static>>, _width: u16) -> Line<'static> {
    spans.push(Span::raw(" ".repeat(120)));
    Line::from(spans)
}

/// Replaces a user's home directory prefix with `~`.
fn tilde_path(path: &str) -> String {
    match dirs::home_dir() {
        Some(home) => {
            let home = home.to_string_lossy();
            if let Some(rest) = path.strip_prefix(home.as_ref()) {
                format!("~{}", rest)
            } else {
                path.to_string()
            }
        }
        None => path.to_string(),
    }
}

/// Truncates a path from the left (keeping the tail) if it exceeds `max` characters.
fn shorten_path(path: &str, max: usize) -> String {
    let chars: Vec<char> = path.chars().collect();
    if chars.len() <= max {
        path.to_string()
    } else {
        let tail: String = chars[chars.len() - (max - 1)..].iter().collect();
        format!("…{}", tail)
    }
}

/// Helper function to calculate a centered rectangle area for modal overlays.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
