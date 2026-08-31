//! Copyright (c) 2026 SeyedAli
//! Licensed under the MIT License. See LICENSE file in the project root for details.
//
//! Module setup - Interactive onboarding screen and in-app PATH help overlay.

use crate::tui::styles::Theme;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    Frame, Terminal,
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};
use std::io;

/// Checks if the shim directory is in `PATH` and presents an interactive setup guide if missing.
///
/// # Errors
/// Returns [`io::Error`] if terminal rendering or event polling fails.
pub async fn run_setup_guide_if_needed<B: Backend>(
    terminal: &mut Terminal<B>,
    shim_path: &str,
    shim_in_path: bool,
) -> io::Result<()> {
    if shim_in_path {
        return Ok(());
    }

    loop {
        terminal.draw(|f| draw_setup_modal(f, f.area(), shim_path))?;

        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Enter
                || key.code == KeyCode::Char('q')
                || key.code == KeyCode::Esc
            {
                break;
            }
        }
    }
    Ok(())
}

/// Draws the PATH-setup help overlay centered on the current frame.
///
/// Used both for first-time onboarding and the in-app `[h]` help overlay.
pub fn draw_setup_modal(frame: &mut Frame, screen: Rect, shim_path: &str) {
    // Wider on roomy terminals, but clamped so it never overflows small screens.
    let pct_x = if screen.width >= 90 { 78 } else { 92 };
    let pct_y = if screen.height >= 26 { 56 } else { 84 };
    let area = centered_rect(pct_x, pct_y, screen);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" 🔧 ", Style::default().fg(Theme::BRAND)),
            Span::styled(" GoVMR Setup ", Theme::title()),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::border());
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "Add the GoVMR shim directory to your PATH so `go`",
                Theme::modal_body(),
            )),
            Line::from(Span::styled(
                "resolves through GoVMR:",
                Theme::modal_body(),
            )),
        ]),
        chunks[0],
    );

    #[cfg(unix)]
    let cmd = format!("export PATH=\"{}:$PATH\"", shim_path);
    #[cfg(windows)]
    let cmd = format!("setx PATH \"%PATH%;{}\"", shim_path);

    let cmd_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::border());
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" $ {} ", cmd),
            Theme::brand_bold(),
        )))
        .block(cmd_block)
        .alignment(Alignment::Center),
        chunks[2],
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Add the line above to your ~/.bashrc, ~/.zshrc, or shell profile, then reload your shell.",
            Theme::muted(),
        )))
        .wrap(ratatui::widgets::Wrap { trim: true }),
        chunks[3],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" Press ", Theme::muted()),
            Span::styled("any key", Theme::key_hint()),
            Span::styled(" to close ", Theme::muted()),
        ]))
        .alignment(Alignment::Center),
        chunks[4],
    );
}

/// Calculates a centered rectangle for the onboarding modal.
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
