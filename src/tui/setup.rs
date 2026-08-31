//! Copyright (c) 2026 SeyedAli
//! Licensed under the MIT License. See LICENSE file in the project root for details.
//!
//! Module setup - Interactive onboarding screen for configuring system PATH settings.

use crate::manager::GoManager;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    Terminal,
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::io;

/// Checks if the shim directory is in `PATH` and presents an interactive setup guide if missing.
///
/// # Arguments
/// * `terminal` - Mutable reference to the terminal backend.
/// * `manager` - Reference to the core GoManager instance.
///
/// # Errors
/// Returns [`io::Error`] if terminal rendering or event polling fails.
pub async fn run_setup_guide_if_needed<B: Backend>(
    terminal: &mut Terminal<B>,
    manager: &GoManager,
) -> io::Result<()> {
    if manager.get_shim_manager().is_in_path() {
        return Ok(());
    }

    let shim_path = manager
        .get_shim_manager()
        .get_shim_dir()
        .to_string_lossy()
        .to_string();

    loop {
        terminal.draw(|f| {
            let size = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(20),
                    Constraint::Percentage(60),
                    Constraint::Percentage(20),
                ])
                .split(size);

            let text = vec![
                Line::from(Span::raw(
                    "To use GoVMR, add the shim directory to your PATH:",
                )),
                Line::from(""),
                #[cfg(unix)]
                Line::from(Span::raw(format!("export PATH=\"{}:$PATH\"", shim_path))),
                #[cfg(windows)]
                Line::from(Span::raw(format!("setx PATH \"%PATH%;{}\"", shim_path))),
                Line::from(""),
                Line::from(Span::raw("Press [Enter] to continue to GoVMR...")),
            ];

            let block = Block::default()
                .title(" GoVMR First-Time Setup ")
                .borders(Borders::ALL)
                .border_style(crate::tui::styles::Theme::border());

            let paragraph = Paragraph::new(text)
                .block(block)
                .alignment(Alignment::Center);
            f.render_widget(paragraph, chunks[1]);
        })?;

        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Enter || key.code == KeyCode::Char('q') {
                break;
            }
        }
    }
    Ok(())
}
