//! Copyright (c) 2026 SeyedAli
//! Licensed under the MIT License. See LICENSE file in the project root for details.
//
//! Module setup - Interactive onboarding screen and in-app PATH help overlay.

use crate::theme::Theme;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame, Terminal,
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
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
/// Platform-specific: shows one command on Unix, two commands (PowerShell + CMD)
/// plus concrete GUI steps on Windows.
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

    #[cfg(unix)]
    draw_setup_content_unix(frame, inner, shim_path, theme);
    #[cfg(windows)]
    draw_setup_content_windows(frame, inner, shim_path, theme);
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Unix layout: one `export` command + copy-pasteable shell profile one-liner.
#[cfg(unix)]
fn draw_setup_content_unix(frame: &mut Frame, inner: Rect, shim_path: &str, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Description
            Constraint::Length(3), // Current-session export command
            Constraint::Length(1), // Spacer
            Constraint::Length(3), // Permanent profile one-liner
            Constraint::Min(1),    // Hint
            Constraint::Length(2), // Press any key
        ])
        .split(inner);

    // Description
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Add the GoVMR shim directory to your PATH so `go` resolves through GoVMR.",
            theme.modal_body(),
        ))),
        chunks[0],
    );

    // Current-session command
    let session_cmd = format!("export PATH=\"{}:$PATH\"", shim_path);
    let session_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border())
        .title(Span::styled(" This session ", theme.muted()));

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" $ {} ", session_cmd),
            theme.brand_bold(),
        )))
        .block(session_block),
        chunks[1],
    );

    // Permanent profile one-liner (auto-detects shell)
    let shell_profile = detect_shell_profile();
    let persist_cmd = format!(
        "echo 'export PATH=\"{}:$PATH\"' >> {} && source {}",
        shim_path, shell_profile, shell_profile
    );
    let persist_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border())
        .title(Span::styled(" Make it permanent ", theme.muted()));

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" $ {} ", persist_cmd),
            theme.brand_bold(),
        )))
        .block(persist_block),
        chunks[3],
    );

    // Hint
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(
                " Run the second command to persist across reboots (appends to {}).",
                shell_profile
            ),
            theme.muted(),
        )))
        .wrap(ratatui::widgets::Wrap { trim: true }),
        chunks[4],
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

/// Best-effort detection of the user's active shell profile file.
///
/// Falls back to `~/.bashrc` if `$SHELL` is unset or unrecognized.
#[cfg(unix)]
fn detect_shell_profile() -> String {
    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|| "~".to_string());

    match std::env::var("SHELL").unwrap_or_default().as_str() {
        s if s.ends_with("/zsh") => format!("{}/.zshrc", home),
        s if s.ends_with("/fish") => format!("{}/.config/fish/config.fish", home),
        s if s.ends_with("/bash") => format!("{}/.bashrc", home),
        _ => format!("{}/.bashrc", home),
    }
}

/// Windows layout: PowerShell + CMD for current session, safe permanent
/// PowerShell command, and GUI fallback steps.
#[cfg(windows)]
fn draw_setup_content_windows(frame: &mut Frame, inner: Rect, shim_path: &str, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Description
            Constraint::Length(3), // PowerShell current session
            Constraint::Length(3), // CMD current session
            Constraint::Length(3), // Permanent PowerShell command
            Constraint::Min(1),    // GUI fallback + warning
            Constraint::Length(2), // Press any key
        ])
        .split(inner);

    // Description
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Add the GoVMR shim directory to your PATH so `go` resolves through GoVMR.",
            theme.modal_body(),
        ))),
        chunks[0],
    );

    // PowerShell — current session
    let ps_cmd = format!("$env:PATH = \"$env:PATH;{}\"", shim_path);
    let ps_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border())
        .title(Span::styled(" PowerShell — this session ", theme.muted()));

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {} ", ps_cmd),
            theme.brand_bold(),
        )))
        .block(ps_block),
        chunks[1],
    );

    // CMD — current session
    let cmd_command = format!("set PATH=%PATH%;{}", shim_path);
    let cmd_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border())
        .title(Span::styled(" CMD — this session ", theme.muted()));

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {} ", cmd_command),
            theme.brand_bold(),
        )))
        .block(cmd_block),
        chunks[2],
    );

    // Permanent — safe PowerShell (no 1024-char truncation)
    let perm_cmd = format!(
        "[Environment]::SetEnvironmentVariable(\"PATH\", [Environment]::GetEnvironmentVariable(\"PATH\", \"User\") + \";{}\", \"User\")",
        shim_path
    );
    let perm_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border())
        .title(Span::styled(
            " Make it permanent (PowerShell) ",
            theme.muted(),
        ));

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {} ", perm_cmd),
            theme.brand_bold(),
        )))
        .block(perm_block),
        chunks[3],
    );

    // Warning + GUI fallback
    let notes = vec![
        Line::from(Span::styled(
            " ⚠ Avoid `setx` — it can truncate PATH to 1024 chars.",
            theme.warning(),
        )),
        Line::from(Span::styled(
            " If the command above fails, open System Properties → Advanced →",
            theme.muted(),
        )),
        Line::from(Span::styled(
            " Environment Variables → User Path → Edit → New → paste the path.",
            theme.muted(),
        )),
        Line::from(Span::styled(
            " Restart your terminal after making PATH permanent.",
            theme.muted(),
        )),
    ];

    frame.render_widget(
        Paragraph::new(notes).wrap(ratatui::widgets::Wrap { trim: true }),
        chunks[4],
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
