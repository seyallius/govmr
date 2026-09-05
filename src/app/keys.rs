//! Module keys - Translation of terminal key events into app mutations and actions.
//!
//! The TUI event loop forwards every pressed key here. The handler walks the
//! same precedence the user sees on screen: global quit shortcuts, active modal
//! capture (help, theme picker, delete confirmation, filter mode), then the
//! main shortcut set.

use crate::{
    app::{Action, App, MsgKind},
    logging,
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

    // Each modal that captures keys handles its own key presses in turn.
    if app.state.show_help {
        return handle_help_overlay_key(key, app);
    }
    if app.state.show_theme_picker {
        return handle_theme_picker_key(key, app);
    }
    if let Some(target) = app.state.confirming_delete.take() {
        return handle_confirm_delete_key(key, app, action_tx, &target);
    }
    if app.state.filter_mode {
        return handle_filter_key(key, app);
    }
    if let Some(outcome) = handle_install_cancel_key(key, app) {
        return outcome;
    }
    if let Some(outcome) = handle_log_panel_key(key, app) {
        return outcome;
    }

    handle_main_shortcut_key(key, app, action_tx)
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Handles keys while the PATH-setup help overlay is open.
///
/// The overlay captures every OTHER key until dismissed, EXCEPT 'q' which
/// quits the app and 'f' which applies the permanent PATH fix. The overlay
/// stays open on 'f' so the result notice is shown inside it.
fn handle_help_overlay_key(key: KeyEvent, app: &mut App) -> KeyOutcome {
    match key.code {
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
                                Line::from(Span::styled(line.clone(), app.state.theme.success()))
                            } else if i == 1 && line.starts_with("    ") {
                                // Command line: indent preserved, brand bold
                                Line::from(Span::styled(line.clone(), app.state.theme.brand_bold()))
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
                        Line::from(Span::styled(format!("  {e}"), app.state.theme.muted())),
                    ]);
                }
            }
            KeyOutcome::Continue
        }
        _ => {
            app.state.show_help = false;
            app.state.path_fix_notice = None;
            KeyOutcome::Continue
        }
    }
}

/// Handles keys while the theme picker is open: navigate with arrows/vim keys,
/// Enter saves, Esc/q cancels and restores the persisted theme.
fn handle_theme_picker_key(key: KeyEvent, app: &mut App) -> KeyOutcome {
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
    KeyOutcome::Continue
}

/// Handles keys while a destructive-action confirmation is pending.
fn handle_confirm_delete_key(
    key: KeyEvent,
    app: &mut App,
    action_tx: &UnboundedSender<Action>,
    target: &str,
) -> KeyOutcome {
    match key.code {
        KeyCode::Char('y' | 'Y') => {
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
    KeyOutcome::Continue
}

/// Handles text input while a filter query is being typed.
fn handle_filter_key(key: KeyEvent, app: &mut App) -> KeyOutcome {
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
    KeyOutcome::Continue
}

/// Intercepts Esc/c to cancel an ongoing installation; other keys fall through
/// to the rest of the dashboard.
fn handle_install_cancel_key(key: KeyEvent, app: &mut App) -> Option<KeyOutcome> {
    if app.state.cancel_install.is_some() {
        logging::debug(&format!(
            "Key pressed during install: {:?}, cancel_install is Some",
            key.code
        ));
        if key.code == KeyCode::Esc || key.code == KeyCode::Char('c') {
            if let Some(tx) = app.state.cancel_install.take() {
                let _ = tx.send(true);
                app.set_status("Cancelling installation...", MsgKind::Info);
            }
            return Some(KeyOutcome::Continue);
        }
    }
    None
}

/// Handles keys for the docked log panel: `L` closes, `` ` `` toggles focus.
/// While focused, the panel swallows navigation keys so they scroll logs
/// instead of the list; unfocused, other keys fall through.
fn handle_log_panel_key(key: KeyEvent, app: &mut App) -> Option<KeyOutcome> {
    if !app.state.show_logs {
        return None;
    }
    match key.code {
        KeyCode::Char('L') => {
            app.close_logs();
            return Some(KeyOutcome::Continue);
        }
        KeyCode::Char('`') => {
            app.state.log_focus = !app.state.log_focus;
            return Some(KeyOutcome::Continue);
        }
        _ => {}
    }
    if !app.state.log_focus {
        return None;
    }
    Some(match key.code {
        KeyCode::Esc => {
            app.state.log_focus = false;
            KeyOutcome::Continue
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.scroll_logs(1);
            KeyOutcome::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.scroll_logs(-1);
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
            app.scroll_logs(i64::MAX);
            KeyOutcome::Continue
        }
        KeyCode::End | KeyCode::Char('G') => {
            app.scroll_logs(i64::MIN);
            KeyOutcome::Continue
        }
        KeyCode::Char('f') => {
            app.toggle_log_follow();
            KeyOutcome::Continue
        }
        KeyCode::Char('w') => {
            app.toggle_log_wrap();
            KeyOutcome::Continue
        }
        KeyCode::Char('q') => KeyOutcome::Quit,
        _ => KeyOutcome::Continue,
    })
}

/// Handles the main dashboard shortcuts once no modal captured the key.
fn handle_main_shortcut_key(
    key: KeyEvent,
    app: &mut App,
    action_tx: &UnboundedSender<Action>,
) -> KeyOutcome {
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
        KeyCode::Char('h' | '?') => {
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
