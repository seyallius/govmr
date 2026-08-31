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

pub async fn run_setup_guide_if_needed<B: Backend>(
    terminal: &mut Terminal<B>,
    manager: &GoManager,
) -> anyhow::Result<()> {
    if manager.get_shim_manager().is_in_path() {
        return Ok(());
    }

    let shim_path = manager
        .get_shim_manager()
        .get_shim_dir()
        .to_string_lossy()
        .to_string();

    loop {
        terminal
            .draw(|f| {
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
                        "To use GoVM, add the shim directory to your PATH:",
                    )),
                    Line::from(""),
                    #[cfg(unix)]
                    Line::from(Span::raw(format!("export PATH=\"{}:$PATH\"", shim_path))),
                    #[cfg(windows)]
                    Line::from(Span::raw(format!("setx PATH \"%PATH%;{}\"", shim_path))),
                    Line::from(""),
                    Line::from(Span::raw("Press [Enter] to continue to GoVM...")),
                ];

                let block = Block::default()
                    .title(" GoVM First-Time Setup ")
                    .borders(Borders::ALL)
                    .border_style(crate::tui::styles::Theme::border());

                let paragraph = Paragraph::new(text)
                    .block(block)
                    .alignment(Alignment::Center);
                f.render_widget(paragraph, chunks[1]);
            })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Enter || key.code == KeyCode::Char('q') {
                break;
            }
        }
    }
    Ok(())
}
