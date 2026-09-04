//! Module keys - Translation of terminal key events into app mutations and actions.
//!
//! The TUI event loop forwards every pressed key here. The handler walks the
//! same precedence the user sees on screen: global quit shortcuts, active modal
//! capture (help, theme picker, delete confirmation, filter mode), then the
//! main shortcut set.

use crate::{
    app::{Action, App, MsgKind},
    theme::{Theme, ThemeName},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::text::{Line, Span};
use tokio::sync::mpsc::UnboundedSender;

// ------------------------------------------ Types & Impls ------------------------------------- //

/// How the event loop should proceed after a key has been handled.
pub enum KeyOutcome {
    /// Keep the event loop running.
    Continue,
    /// Quit the application.
    Quit,
}

// ----------------------------------------- Public API ----------------------------------------- //

/// Applies one terminal key event to the application, returning how the event
/// loop should proceed.
///
/// Release/repeat artifacts are ignored, universal `Ctrl-C` quits bypass every
/// modal, and any other key is routed through the active modal (help, theme
/// picker, delete confirmation, filter) before reaching the main shortcuts.
pub fn handle_key(key: KeyEvent, app: &mut App, action_tx: &UnboundedSender<Action>) -> KeyOutcome {
    // Only react on key press (not release artifacts).
    if key.kind == KeyEventKind::Release {
        return KeyOutcome::Continue;
    }

    // Universal quit shortcuts MUST bypass modal capture.
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return KeyOutcome::Quit;
    }

    // Help overlay captures every OTHER key until dismissed, EXCEPT 'q' which
    // quits the app and 'f' which applies the permanent PATH fix. The overlay
    // stays open on 'f' so the result notice is shown inside it.
    if app.state.show_help {
        return match key.code {
            KeyCode::Char('q') => KeyOutcome::Quit,
            KeyCode::Char('f') => {
                match app.manager.fix_path_permanently() {
                    Ok(lines) => {
                        // lines is Vec<String> - we need to iterate over it
                        let styled_lines: Vec<Line<'static>> = lines
                            .iter() // Use iter() instead of into_iter()
                            .enumerate()
                            .map(|(i, line)| {
                                if i == 0 {
                                    // First line: success message
                                    Line::from(Span::styled(
                                        line.clone(),
                                        app.state.theme.success(),
                                    ))
                                } else if i == 1 && line.starts_with("    ") {
                                    // Command line: indent preserved, brand bold
                                    Line::from(Span::styled(
                                        line.clone(),
                                        app.state.theme.brand_bold(),
                                    ))
                                } else {
                                    // Other lines: muted but visible
                                    Line::from(Span::styled(line.clone(), app.state.theme.muted()))
                                }
                            })
                            .collect();
                        app.state.path_fix_notice = Some(styled_lines);
                    }
                    Err(e) => {
                        app.state.path_fix_notice = Some(vec![
                            Line::from(Span::styled(
                                "Failed to fix PATH:".to_string(),
                                app.state.theme.error(),
                            )),
                            Line::from(Span::styled(format!("  {}", e), app.state.theme.muted())),
                        ]);
                    }
                }
                return KeyOutcome::Continue;
            }
            _ => {
                app.state.show_help = false;
                app.state.path_fix_notice = None;
                KeyOutcome::Continue
            }
        };
    }

    // Log viewer: scroll, toggle follow, or close. Captures keys while open.
    if app.state.show_logs {
        return match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('L') => {
                app.close_logs();
                KeyOutcome::Continue
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.scroll_logs(1); // up = older history
                KeyOutcome::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.scroll_logs(-1); // down = newer entries
                KeyOutcome::Continue
            }
            KeyCode::PageUp => {
                app.scroll_logs(15);
                KeyOutcome::Continue
            }
            KeyCode::PageDown => {
                app.scroll_logs(-15);
                KeyOutcome::Continue
            }
            KeyCode::Home | KeyCode::Char('g') => {
                app.scroll_logs(i64::MAX); // jump to oldest
                KeyOutcome::Continue
            }
            KeyCode::End | KeyCode::Char('G') => {
                app.scroll_logs(i64::MIN); // jump to newest
                KeyOutcome::Continue
            }
            KeyCode::Char('f') => {
                app.toggle_log_follow();
                KeyOutcome::Continue
            }
            _ => KeyOutcome::Continue,
        };
    }

    // Theme picker: navigate with arrows/vim keys, Enter saves,
    // Esc/q cancels and restores the persisted theme.
    if app.state.show_theme_picker {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => app.picker_cancel(),
            KeyCode::Enter => app.picker_apply(),
            KeyCode::Down | KeyCode::Char('j') => app.picker_move(1),
            KeyCode::Up | KeyCode::Char('k') => app.picker_move(-1),
            KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                let i = (c as u8 - b'1') as usize;
                if i < ThemeName::ALL.len() {
                    app.state.theme_picker_index = i;
                    app.state.theme = Theme::for_name(app.picker_theme());
                    app.picker_apply();
                }
            }
            _ => {}
        }
        return KeyOutcome::Continue;
    }

    // Destructive-action confirmation takes precedence.
    if let Some(target) = app.state.confirming_delete.take() {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(v) = app
                    .state
                    .versions
                    .iter()
                    .find(|x| x.raw_version == target)
                    .cloned()
                {
                    let _ = action_tx.send(Action::Delete(v));
                }
            }
            _ => {
                app.set_status("Delete cancelled", MsgKind::Info);
            }
        }
        return KeyOutcome::Continue;
    }

    // While typing a filter query, capture text input.
    if app.state.filter_mode {
        match key.code {
            KeyCode::Esc => {
                app.state.filter.clear();
                app.state.filter_mode = false;
                app.state.list_state.select(Some(0));
            }
            KeyCode::Enter => {
                app.state.filter_mode = false;
                app.clamp_selection();
            }
            KeyCode::Backspace => {
                app.state.filter.pop();
                app.state.list_state.select(Some(0));
            }
            KeyCode::Char(c) => {
                app.state.filter.push(c);
                app.state.list_state.select(Some(0));
            }
            _ => {}
        }
        return KeyOutcome::Continue;
    }

    match key.code {
        KeyCode::Char('q') => return KeyOutcome::Quit,
        KeyCode::Tab => app.switch_tab(),
        KeyCode::Down | KeyCode::Char('j') => app.next_item(),
        KeyCode::Up | KeyCode::Char('k') => app.previous_item(),
        KeyCode::Char('/') => {
            app.state.filter_mode = true;
        }
        KeyCode::Char('T') => {
            app.open_theme_picker();
        }
        KeyCode::Char('L') => {
            app.open_logs();
        }
        KeyCode::Char('h') | KeyCode::Char('?') => {
            app.state.show_help = true;
        }
        KeyCode::Char('r') if !app.is_busy() => {
            let _ = action_tx.send(Action::Refresh);
        }
        KeyCode::Char('i') if !app.is_busy() => {
            if let Some(v) = app.selected_version().cloned() {
                if v.installed {
                    app.set_status(
                        format!("Go {} is already installed", v.raw_version),
                        MsgKind::Info,
                    );
                } else {
                    let _ = action_tx.send(Action::Install(v));
                }
            }
        }
        KeyCode::Char('u') if !app.is_busy() => {
            if let Some(v) = app.selected_version().cloned() {
                if v.installed {
                    let _ = action_tx.send(Action::Use(v));
                } else {
                    app.set_status("Install this version first — press i", MsgKind::Error);
                }
            }
        }
        KeyCode::Char('d') if !app.is_busy() => {
            if let Some(v) = app.selected_version().cloned() {
                if v.active {
                    app.set_status(
                        "Cannot delete the active version — switch first",
                        MsgKind::Error,
                    );
                } else if v.installed {
                    app.state.confirming_delete = Some(v.raw_version.clone());
                }
            }
        }
        _ => {}
    }

    KeyOutcome::Continue
}
