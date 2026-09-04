//! Module setup - Interactive onboarding screen and in-app PATH help overlay.

use super::widgets::centered_rect;
use crate::theme::Theme;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    backend::Backend, layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
    Terminal,
};
use std::io;

// ----------------------------------------- Public API ----------------------------------------- //

/// Checks if the shim directory is in `PATH` and presents an interactive setup guide if missing.
///
/// Returns `true` if the user wants to continue to the main TUI, or `false` if they pressed a quit key.
///
/// # Errors
/// Returns [`io::Error`] if terminal rendering or event polling fails.
pub async fn run_setup_guide_if_needed<B: Backend>(
    terminal: &mut Terminal<B>,
    shim_path: &str,
    shim_in_path: bool,
    theme: &Theme,
) -> io::Result<bool> {
    if shim_in_path {
        return Ok(true);
    }

    loop {
        terminal.draw(|f| draw_setup_modal(f, f.area(), shim_path, theme))?;
        if let Event::Key(key) = event::read()? {
            // Only react to physical key presses, ignoring release/repeat artifacts
            if key.kind == KeyEventKind::Press {
                // Universal quit shortcuts exit the setup guide entirely
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    return Ok(false);
                }
                if key.code == KeyCode::Char('q') {
                    return Ok(false);
                }

                // ANY other key press closes the modal and proceeds to the app
                return Ok(true);
            }
        }
    }
}

/// Draws the PATH-setup help overlay centered on the current frame.
///
/// Used both for first-time onboarding and the in-app `[h]` help overlay.
/// Shows only short one-liners; the permanent fix is applied by pressing `f`,
/// which runs the platform script in a hidden child process (see
/// [`crate::manager::GoManager::fix_path_permanently`]).
pub fn draw_setup_modal(frame: &mut Frame, screen: Rect, shim_path: &str, theme: &Theme) {
    // Wider on roomy terminals, but clamped so it never overflows small screens.
    let pct_x = if screen.width >= 90 { 78 } else { 92 };
    let pct_y = if screen.height >= 32 { 68 } else { 88 };
    let area = centered_rect(pct_x, pct_y, screen);

    frame.render_widget(Block::default().style(Style::default().bg(theme.bg)), area);

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" 🔧 ", Style::default().fg(theme.brand)),
            Span::styled(" GoVMR Setup ", theme.title()),
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

    draw_setup_content(frame, inner, shim_path, theme);
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Renders the setup content for the current platform.
///
/// On Unix, this shows a short `export` one-liner + `f` to persist via govmr itself.
/// On Windows, this shows a short session one-liner + `f` to fix permanently.
///
/// The verbose (but safe, idempotent) PowerShell script on Windows is intentionally NOT
/// rendered here — pressing `f` runs it in a hidden child process instead,
/// so nothing long ever overflows the modal.
fn draw_setup_content(frame: &mut Frame, inner: Rect, shim_path: &str, theme: &Theme) {
    let chunks = create_layout_chunks(inner);

    // Description
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Add the GoVMR shim directory to your PATH so `go` resolves through GoVMR.",
            theme.modal_body(),
        ))),
        chunks[0],
    );

    // Current-session one-liner
    #[cfg(unix)]
    let session_cmd = format!("export PATH=\"{}:$PATH\"", shim_path);
    #[cfg(windows)]
    let session_cmd = format!("$env:PATH+=\";{}\"", shim_path);

    let session_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border())
        .title(Span::styled(" This session ", theme.muted()));

    #[cfg(unix)]
    let session_display = format!(" $ {} ", session_cmd);
    #[cfg(windows)]
    let session_display = format!(" PS> {} ", session_cmd);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            session_display,
            theme.brand_bold(),
        )))
            .block(session_block),
        chunks[1],
    );

    // Press f to fix permanently
    #[cfg(unix)]
    let fix_text = " to fix permanently (govmr appends to your shell profile for you).";
    #[cfg(windows)]
    let fix_text = " to fix permanently (govmr runs the PowerShell snippet for you).";

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" Press ", theme.muted()),
            Span::styled("f", theme.key_hint()),
            Span::styled(fix_text, theme.muted()),
        ])),
        chunks[3],
    );

    // Press any key
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" Press ", theme.muted()),
            Span::styled("any key", theme.key_hint()),
            Span::styled(" to close ", theme.muted()),
        ]))
            .alignment(Alignment::Center),
        chunks[5],
    );
}

/// Creates the layout chunks for the application interface.
///
/// This layout organizes the terminal space into a vertical arrangement
/// with specific sections for displaying application information and controls.
fn create_layout_chunks(inner: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Description
            Constraint::Length(3), // Current-session one-liner
            Constraint::Length(1), // Spacer
            Constraint::Length(2), // Press f to fix permanently
            Constraint::Min(1),    // Filler
            Constraint::Length(2), // Press any key
        ])
        .split(inner)
        .to_vec()
}
