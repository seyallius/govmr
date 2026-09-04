//! Module dashboard - Main dashboard layout composition, chrome, and version list rendering.

use super::{
    modals::{render_delete_modal, render_install_modal, render_theme_picker},
    setup::draw_setup_modal,
    status::render_status_bar,
    widgets::{right_pad, shorten_path, tilde_path},
};
use crate::{
    app::{visible_indices, ActiveTab, AppState, BusyState},
    theme::Theme,
    version::GoVersion,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Frame,
};

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

    // ---- Background fill (makes light themes solid) ----------------------------------------- //
    frame.render_widget(Block::default().style(Style::default().bg(theme.bg)), size);

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
        .style(Style::default().bg(theme.bg))
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
    // otherwise bleed through the modal edges. The theme picker is deliberately
    // NOT included here: it paints an opaque background itself, so the live
    // dashboard stays visible behind it as a real-time preview.
    let modal_active = matches!(state.busy, Some(BusyState::Installing { .. }))
        || state.confirming_delete.is_some()
        || state.show_help;

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

// -------------------------------------- Internal Helpers -------------------------------------- //

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
        tab(
            "Available",
            available_count,
            state.active_tab == ActiveTab::Available,
        ),
        tab(
            "Installed",
            installed_count,
            state.active_tab == ActiveTab::Installed,
        ),
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

/// Renders the active tab's content list, or a prominent centered loading spinner
/// if the initial version manifest is currently being fetched from the network.
fn render_content(frame: &mut Frame, area: Rect, state: &mut AppState, theme: &Theme) {
    let visible: Vec<usize> = visible_indices(state);

    // While a blocking modal is on screen, hide the list entirely so padded
    // rows and borders can't bleed through the modal.
    let modal_up = matches!(state.busy, Some(BusyState::Installing { .. }))
        || state.confirming_delete.is_some()
        || state.show_help;
    if modal_up {
        return;
    }

    if state.versions.is_empty() && matches!(state.busy, Some(BusyState::Refreshing)) {
        let block = content_block(theme);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Use layout constraints to vertically center the loading message
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(40),
                Constraint::Length(1),
                Constraint::Percentage(60),
            ])
            .split(inner);

        let center_spinner = [
            "0000", "0001", "0010", "0011", "0100", "0101", "0110", "0111", "1000", "1001", "1010",
            "1011", "1100", "1101", "1110", "1111",
        ];
        let spinner_char = center_spinner[(state.tick_count as usize) % center_spinner.len()];

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!(" {} ", spinner_char), theme.brand_bold()),
                Span::styled("Fetching Go releases, please wait...", theme.highlight()),
            ]))
            .alignment(Alignment::Center),
            chunks[1],
        );
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
        Span::styled(
            format!("{:>8}", GoVersion::format_size(v.size)),
            theme.muted(),
        ),
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
