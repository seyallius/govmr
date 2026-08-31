use crate::{
    app::{ActiveTab, AppState},
    tui::styles::Theme,
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Row, Table, Tabs},
};

pub fn render(frame: &mut Frame, state: &mut AppState) {
    let size = frame.area();

    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Theme::border())
        .title(Span::styled(" GoVM - Go Version Manager ", Theme::title()));
    frame.render_widget(main_block, size);

    let inner = size.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Warning (if any)
            Constraint::Length(3), // Tabs
            Constraint::Min(5),    // Main Content
            Constraint::Length(2), // Status/Message
            Constraint::Length(1), // Help Footer
        ])
        .split(inner);

    // Warning Banner if Shim is not in PATH
    if !state.is_shim_in_path {
        let warning =
            Paragraph::new("⚠️ GoVM shim is not in your PATH. Please configure your shell.")
                .style(Theme::warning());
        frame.render_widget(warning, chunks[0]);
    }

    // Tabs
    let tab_titles = vec!["Available Versions", "Installed Versions"];
    let tabs = Tabs::new(tab_titles)
        .select(match state.active_tab {
            ActiveTab::Available => 0,
            ActiveTab::Installed => 1,
        })
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Theme::muted()),
        )
        .highlight_style(Theme::highlight().add_modifier(Modifier::BOLD));
    frame.render_widget(tabs, chunks[1]);

    // Tab Body
    match state.active_tab {
        ActiveTab::Available => {
            let items: Vec<ListItem> = state
                .versions
                .iter()
                .map(|v| {
                    let mut spans = vec![Span::raw(format!("{:<15}", v.display_name))];
                    if v.active {
                        spans.push(Span::styled("(active) ", Theme::success()));
                    }
                    if v.installed {
                        spans.push(Span::styled("(installed)", Theme::highlight()));
                    }
                    ListItem::new(Line::from(spans))
                })
                .collect();

            let list = List::new(items)
                .highlight_style(
                    Style::default()
                        .bg(Color::Rgb(60, 113, 168))
                        .fg(Color::White),
                )
                .highlight_symbol(">> ");
            frame.render_stateful_widget(list, chunks[2], &mut state.list_state);
        }
        ActiveTab::Installed => {
            let installed: Vec<&crate::models::GoVersion> =
                state.versions.iter().filter(|v| v.installed).collect();
            let rows: Vec<Row> = installed
                .iter()
                .map(|v| {
                    let status = if v.active { "active" } else { "" };
                    let path_str = v
                        .path
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    Row::new(vec![v.raw_version.clone(), path_str, status.to_string()])
                })
                .collect();

            let table = Table::new(
                rows,
                [
                    Constraint::Percentage(20),
                    Constraint::Percentage(60),
                    Constraint::Percentage(20),
                ],
            )
            .header(Row::new(vec!["Version", "Path", "Status"]).style(Theme::title()))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Theme::muted()),
            );
            frame.render_widget(table, chunks[2]);
        }
    }

    // Status Message / Loading Indicator
    if state.loading {
        let loading_msg = if let Some(ver) = &state.action_target {
            format!("Processing Go {}...", ver)
        } else {
            "Loading...".into()
        };
        frame.render_widget(
            Paragraph::new(loading_msg).style(Theme::highlight()),
            chunks[3],
        );
    } else if let Some((msg, is_err)) = &state.status_message {
        let style = if *is_err {
            Theme::error()
        } else {
            Theme::success()
        };
        frame.render_widget(Paragraph::new(msg.as_str()).style(style), chunks[3]);
    }

    // Help Footer
    let help = match state.active_tab {
        ActiveTab::Available => {
            "[i] Install  [u] Use  [d] Delete  [r] Refresh  [Tab] Switch Tab  [q] Quit"
        }
        ActiveTab::Installed => "[u] Use  [d] Delete  [Tab] Switch Tab  [q] Quit",
    };
    frame.render_widget(Paragraph::new(help).style(Theme::muted()), chunks[4]);

    // Confirmation Modal
    if let Some(target) = &state.confirming_delete {
        let modal_area = centered_rect(60, 20, size);
        frame.render_widget(Clear, modal_area);
        let block = Block::default()
            .title(" Confirm Deletion ")
            .borders(Borders::ALL)
            .border_style(Theme::error());
        let text = Paragraph::new(format!(
            "\nAre you sure you want to delete Go {}?\n\n[y] Yes    [n] No",
            target
        ))
        .block(block)
        .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(text, modal_area);
    }
}

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
