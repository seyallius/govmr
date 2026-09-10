//! Module dashboard - Main dashboard layout composition, chrome, and version list rendering.

use super::{
    help::render_command_help,
    logs::render_log_panel,
    modals::{
        render_delete_modal, render_progress_modal, render_system_prompt, render_theme_picker,
    },
    setup::draw_setup_modal,
    status::render_status_bar,
    widgets::{right_pad, shorten_path, tilde_path},
};
use crate::{
    app::{ActiveTab, AppState, BusyState, visible_indices},
    theme::Theme,
    version::GoVersion,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Tabs},
};

// ------------------------------------- Public (crate) API ------------------------------------- //

/// Primary render routine for the `GoVMR` dashboard interface.
///
/// # Arguments
/// * `frame` - Mutable drawing frame provided by Ratatui.
/// * `state` - Current mutable application state.
pub(crate) fn render(frame: &mut Frame, state: &mut AppState) {
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

    let right_title = state.versions.iter().find(|v| v.active).map_or_else(
        || Line::from(Span::raw("")),
        |v| {
            Line::from(vec![
                Span::styled(" active: ", theme.muted()),
                Span::styled(v.display_name.clone(), theme.badge_active()),
                Span::raw("  "),
            ])
        },
    );

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

    // Dock the IDE-style log panel to the bottom when open; the dashboard
    // shrinks to make room but stays fully interactive.
    let (dash_inner, log_area) = if state.show_logs {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Percentage(38)])
            .split(inner);
        (split[0], Some(split[1]))
    } else {
        (inner, None)
    };

    // Dock the keyboard help panel to the right when open; the dashboard body
    // keeps the left-hand two thirds and stays fully interactive underneath.
    let (body_area, help_area) = if state.show_command_help {
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(5), Constraint::Percentage(44)])
            .split(dash_inner);
        (split[0], Some(split[1]))
    } else {
        (dash_inner, None)
    };

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
        .split(body_area);

    // A centered modal covers the chrome areas; hide the pieces that would
    // otherwise bleed through the modal edges. The theme picker is deliberately
    // NOT included here: it paints an opaque background itself, so the live
    // dashboard stays visible behind it as a real-time preview.
    let modal_active = matches!(
        state.busy,
        Some(BusyState::Installing { .. } | BusyState::Updating { .. })
    ) || state.confirming_delete.is_some()
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

    if let Some(area) = log_area {
        render_log_panel(frame, area, state, &theme);
    }

    if state.show_command_help {
        dim_area(frame, size, &theme);
        if let Some(area) = help_area {
            render_command_help(frame, area, state, &theme);
        }
    }
}

/// Draws top-level modal overlays (theme picker, install progress, delete, help).
pub(crate) fn render_overlays(frame: &mut Frame, state: &AppState) {
    let size = frame.area();
    let theme = state.theme;

    if state.system_prompt.is_some() {
        dim_area(frame, size, &theme);
    }

    if let Some(busy) = &state.busy
        && matches!(
            busy,
            BusyState::Installing { .. } | BusyState::Updating { .. }
        )
    {
        render_progress_modal(frame, size, busy, state.tick_count, &theme);
    }

    if state.confirming_delete.is_some() {
        render_delete_modal(frame, size, state, &theme);
    }

    if state.show_help {
        draw_setup_modal(
            frame,
            size,
            &state.shim_path,
            &theme,
            state.path_fix_notice.as_deref(),
        );
    }

    if state.show_theme_picker {
        render_theme_picker(frame, size, state, &theme);
    }

    if let Some(prompt) = state.system_prompt {
        render_system_prompt(frame, size, prompt, &theme);
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
        if active {
            // Neon box: full-brand rails, dark glass body, bright text.
            let fg = theme.brand;
            let accent = theme.success; // The "neon glow" color (e.g., bright green/cyan)

            let pill_style = Style::default().fg(fg).add_modifier(Modifier::BOLD);
            let accent_style = Style::default().fg(accent);
            let count_style = Style::default().fg(fg).add_modifier(Modifier::BOLD);

            Line::from(vec![
                Span::styled(" ● ", accent_style), // Glowing neon power indicator
                Span::styled(name, pill_style),
                Span::styled(format!(" ({count}) "), count_style),
            ])
        } else {
            // One notch quieter than before: plain gray, no marker glow.
            let dim = Style::default().fg(theme.grey);
            Line::from(vec![
                Span::styled("  ○ ", dim),
                Span::styled(name, dim),
                Span::styled(format!(" ({count})  "), dim),
            ])
        }
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
        .divider(Span::styled("│", theme.dim_border()))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(theme.dim_border()),
        )
        // 🚨 CRITICAL: we style the spans ourselves; a non-default highlight
        // style would patch over the pill and flatten the contrast.
        .highlight_style(Style::default());
    frame.render_widget(tabs, area);
}

/// Renders the active tab's content list, or a prominent centered loading spinner
/// if the initial version manifest is currently being fetched from the network.
fn render_content(frame: &mut Frame, area: Rect, state: &mut AppState, theme: &Theme) {
    let visible: Vec<usize> = visible_indices(state);

    // While a blocking modal is on screen, hide the list entirely so padded
    // rows and borders can't bleed through the modal.
    let modal_up = matches!(
        state.busy,
        Some(BusyState::Installing { .. } | BusyState::Updating { .. })
    ) || state.confirming_delete.is_some()
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
        // The modulo keeps the index inside the frame table on any pointer width.
        #[allow(clippy::cast_possible_truncation)]
        let spinner_idx = (state.tick_count as usize) % center_spinner.len();
        let spinner_char = center_spinner[spinner_idx];

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!(" {spinner_char} "), theme.brand_bold()),
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
            spans.push(Span::styled(format!("[{tag}]"), theme.badge_unstable()));
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
        .border_style(theme.dim_border())
}

/// Renders the keyboard-shortcut footer.
fn render_footer(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let hint = |key: &'static str, label: &'static str| {
        vec![
            Span::styled(format!(" {key} "), theme.key_hint()),
            Span::styled(format!("{label} "), theme.muted()),
        ]
    };

    let mut spans = Vec::new();
    if state.filter_mode {
        spans.extend(hint("enter", "apply"));
        spans.extend(hint("esc", "clear"));
    } else if state.cancel_install.is_some() {
        spans.extend(hint("esc", "cancel install"));
        spans.extend(hint("q", "quit"));
    } else {
        spans.extend(hint("↑↓/jk", "move"));
        spans.extend(hint("tab", "switch"));
        spans.extend(hint("/", "filter"));
        spans.extend(hint("i", "install"));
        spans.extend(hint("u", "use"));
        spans.extend(hint("d", "delete"));
        spans.extend(hint("T", "theme"));
        spans.extend(hint("L", "logs"));
        if state.show_logs {
            spans.extend(hint("`", "focus"));
        }
        spans.extend(hint("r", "refresh"));
        if state.show_command_help {
            spans.extend(hint("esc", "close"));
        } else {
            spans.extend(hint("?", "help"));
        }
        spans.extend(hint("q", "quit"));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        area,
    );
}

/// Heavily dims every cell inside `area` (ghost-text effect) so an overlay
/// panel visually floats above the dashboard.
///
/// Glyphs are kept but recolored to the theme's quiet chrome color on the
/// plain background — a faint afterimage on both dark and light schemes.
fn dim_area(frame: &mut Frame, area: Rect, theme: &Theme) {
    let buf = frame.buffer_mut();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            buf[(x, y)].set_style(Style::default().fg(theme.dim).bg(theme.bg));
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::{
        app::{ActiveTab, AppState, BusyState, Phase},
        theme::{Theme, ThemeFamily, ThemeName, ThemePickerView},
        tui::dashboard::{render, render_overlays},
        version::GoVersion,
    };
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    fn make_terminal() -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(100, 30)).unwrap()
    }

    fn versions_fixture() -> Vec<GoVersion> {
        vec![
            GoVersion {
                raw_version: "1.22.0".into(),
                display_name: "go1.22.0".into(),
                filename: "go1.22.0.tar.gz".into(),
                url: "https://example.com/go1.22.0.tar.gz".into(),
                size: 68_000_000,
                installed: false,
                active: false,
                path: None,
                stable: true,
            },
            GoVersion {
                raw_version: "1.24rc1".into(),
                display_name: "go1.24rc1".into(),
                filename: "go1.24rc1.tar.gz".into(),
                url: "https://example.com/go1.24rc1.tar.gz".into(),
                size: 72_000_000,
                installed: true,
                active: true,
                path: Some(std::path::PathBuf::from(
                    "/home/tester/.govmr/versions/go1.24rc1",
                )),
                stable: false,
            },
            GoVersion {
                raw_version: "1.21.6".into(),
                display_name: "go1.21.6".into(),
                filename: "go1.21.6.tar.gz".into(),
                url: "https://example.com/go1.21.6.tar.gz".into(),
                size: 65_000_000,
                installed: true,
                active: false,
                path: Some(std::path::PathBuf::from(
                    "/home/tester/.govmr/versions/go1.21.6",
                )),
                stable: true,
            },
        ]
    }

    #[test]
    fn renders_available_tab() {
        let mut terminal = make_terminal();
        let mut state = AppState::from_versions(versions_fixture(), true);
        terminal
            .draw(|f| {
                render(f, &mut state);
                render_overlays(f, &state);
            })
            .unwrap();

        let text = buffer_as_text(terminal.backend().buffer());
        assert!(text.contains("GoVMR"), "brand title should render");
        assert!(text.contains("Available"), "available tab should render");
        assert!(text.contains("Installed"), "installed tab should render");
        assert!(text.contains("go1.22.0"), "version rows should render");
        assert!(text.contains("active"), "active badge should render");
    }

    #[test]
    fn renders_installed_tab_with_paths() {
        let mut terminal = make_terminal();
        let mut state = AppState::from_versions(versions_fixture(), true);
        state.active_tab = ActiveTab::Installed;
        state.list_state.select(Some(0));
        terminal
            .draw(|f| {
                render(f, &mut state);
                render_overlays(f, &state);
            })
            .unwrap();

        let text = buffer_as_text(terminal.backend().buffer());
        assert!(text.contains("go1.24rc1"), "installed rows should render");
        assert!(text.contains(".govmr"), "install path should render");
    }

    #[test]
    fn renders_install_download_modal_with_gauge() {
        let mut terminal = make_terminal();
        let mut state = AppState::from_versions(versions_fixture(), true);
        state.busy = Some(BusyState::Installing {
            version: "1.22.0".into(),
            phase: Phase::Downloading,
            downloaded: 34_000_000,
            total: 68_000_000,
            speed: 5_000_000.0,
            started_at: std::time::Instant::now(),
        });
        terminal
            .draw(|f| {
                render(f, &mut state);
                render_overlays(f, &state);
            })
            .unwrap();

        let text = buffer_as_text(terminal.backend().buffer());
        assert!(text.contains("Installing Go 1.22.0"), "modal title");
        assert!(text.contains("50.0%"), "gauge percentage should show");
    }

    #[test]
    fn renders_extraction_phase_modal() {
        let mut terminal = make_terminal();
        let mut state = AppState::from_versions(versions_fixture(), true);
        state.busy = Some(BusyState::Installing {
            version: "1.22.0".into(),
            phase: Phase::Extracting,
            downloaded: 68_000_000,
            total: 68_000_000,
            speed: 0.0,
            started_at: std::time::Instant::now(),
        });
        terminal
            .draw(|f| {
                render(f, &mut state);
                render_overlays(f, &state);
            })
            .unwrap();

        let text = buffer_as_text(terminal.backend().buffer());
        assert!(text.contains("extracting archive"), "extraction phase");
    }

    #[test]
    fn renders_delete_confirmation_modal() {
        let mut terminal = make_terminal();
        let mut state = AppState::from_versions(versions_fixture(), true);
        state.confirming_delete = Some("1.21.6".into());
        terminal
            .draw(|f| {
                render(f, &mut state);
                render_overlays(f, &state);
            })
            .unwrap();

        let text = buffer_as_text(terminal.backend().buffer());
        assert!(text.contains("Deletion"), "delete modal title");
        assert!(text.contains("1.21.6"), "delete target shown");
    }

    #[test]
    fn renders_filter_mode_and_filters_rows() {
        let mut terminal = make_terminal();
        let mut state = AppState::from_versions(versions_fixture(), true);
        state.filter_mode = true;
        state.filter = "1.22".into();
        state.list_state.select(Some(0));
        terminal
            .draw(|f| {
                render(f, &mut state);
                render_overlays(f, &state);
            })
            .unwrap();

        let text = buffer_as_text(terminal.backend().buffer());
        assert!(text.contains("Filter"), "filter prompt shown");
        assert!(text.contains("go1.22.0"), "matching row shown");
    }

    #[test]
    fn renders_path_warning_when_shim_missing() {
        let mut terminal = make_terminal();
        let mut state = AppState::from_versions(versions_fixture(), false);
        terminal
            .draw(|f| {
                render(f, &mut state);
                render_overlays(f, &state);
            })
            .unwrap();

        let text = buffer_as_text(terminal.backend().buffer());
        assert!(text.contains("PATH"), "path warning banner shown");
    }

    #[test]
    fn renders_theme_picker_with_all_schemes() {
        // The picker is two-level: opening a folder lists only that family's
        // themes, so iterate both folders to confirm every shipped scheme renders.
        for family in ThemeFamily::ALL {
            let mut terminal = make_terminal();
            let mut state = AppState::from_versions(versions_fixture(), true);
            state.show_theme_picker = true;
            state.theme = Theme::for_name(ThemeName::Midnight);
            state.theme_picker.view = ThemePickerView::Family(family);
            terminal
                .draw(|f| {
                    render(f, &mut state);
                    render_overlays(f, &state);
                })
                .unwrap();

            let text = buffer_as_text(terminal.backend().buffer());
            assert!(text.contains("Color Theme"), "picker title");
            for name in ThemeName::in_family(family) {
                assert!(
                    text.contains(name.title()),
                    "theme {} should be listed inside {family:?}",
                    name.title()
                );
            }
        }
    }

    #[test]
    fn alternate_theme_still_renders_without_panicking() {
        for name in ThemeName::ALL {
            let mut terminal = make_terminal();
            let mut state = AppState::from_versions(versions_fixture(), true);
            state.theme = Theme::for_name(name);
            terminal
                .draw(|f| {
                    render(f, &mut state);
                })
                .unwrap();
        }
    }

    #[test]
    fn light_theme_fills_screen_with_light_background() {
        use ratatui::style::Color;
        let mut terminal = make_terminal();
        let mut state = AppState::from_versions(versions_fixture(), true);
        state.theme = Theme::for_name(ThemeName::Light);
        terminal.draw(|f| render(f, &mut state)).unwrap();

        let area = terminal.backend().buffer().area();
        // Sample several cells; they should all carry the light background.
        let mut seen_light_bg = false;
        for y in 0..area.height {
            for x in 0..area.width {
                if terminal.backend().buffer()[(x, y)].bg == Color::Rgb(250, 250, 248) {
                    seen_light_bg = true;
                }
            }
        }
        assert!(
            seen_light_bg,
            "light theme should paint a solid light background"
        );
    }

    #[test]
    fn renders_theme_picker_shows_both_family_folders() {
        let mut terminal = make_terminal();
        let mut state = AppState::from_versions(versions_fixture(), true);
        state.show_theme_picker = true;
        terminal
            .draw(|f| {
                render(f, &mut state);
                render_overlays(f, &state);
            })
            .unwrap();

        let text = buffer_as_text(terminal.backend().buffer());
        assert!(text.contains("Color Theme"), "picker title");
        for family in ThemeFamily::ALL {
            assert!(
                text.contains(family.label()),
                "folder {} should appear on the picker's top level",
                family.label()
            );
        }
    }

    #[test]
    fn selection_navigation_wraps_within_visible_list() {
        let mut state = AppState::from_versions(versions_fixture(), true);
        state.next_item();
        state.next_item();
        assert_eq!(state.list_state.selected(), Some(2));
        state.next_item();
        assert_eq!(state.list_state.selected(), Some(0), "wraps to top");
        state.previous_item();
        assert_eq!(state.list_state.selected(), Some(2), "wraps to bottom");
    }

    #[test]
    fn renders_command_help_panel_docked_right() {
        let mut terminal = make_terminal();
        let mut state = AppState::from_versions(versions_fixture(), true);
        state.show_command_help = true;
        terminal
            .draw(|f| {
                render(f, &mut state);
                render_overlays(f, &state);
            })
            .unwrap();

        let text = buffer_as_text(terminal.backend().buffer());
        assert!(
            text.contains("Keyboard Help"),
            "help panel title should render"
        );
        assert!(
            text.contains("Quit from any screen"),
            "top binding should render"
        );
        assert!(
            text.contains("Filter versions"),
            "a later binding should render"
        );
    }

    #[test]
    fn command_help_panel_scrolls_to_reveal_lower_commands() {
        let mut terminal = make_terminal();
        let mut state = AppState::from_versions(versions_fixture(), true);

        state.show_command_help = true;
        state.command_help_scroll = usize::MAX; // Jump far past the end.

        terminal
            .draw(|f| {
                render(f, &mut state);
                render_overlays(f, &state);
            })
            .unwrap();

        // 1. The draw should have clamped the offset back to a valid window.
        assert!(
            state.command_help_scroll < usize::MAX,
            "scroll offset should be clamped on render"
        );

        let text = buffer_as_text(terminal.backend().buffer());

        // 2. The bottom binding should be visible after scrolling to the end.
        assert!(
            text.contains("esc close"),
            "the bottom binding should become visible after scrolling"
        );

        // 3. The top binding should have scrolled out of view.
        assert!(
            !text.contains("Quit from any screen"),
            "the top binding should scroll out of view"
        );
    }

    /// Flattens a ratatui test buffer into a plain string for substring assertions.
    fn buffer_as_text(buf: &Buffer) -> String {
        let area = buf.area();
        let mut out = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }
}
