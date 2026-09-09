//! Module keys - Translation of terminal key events into app mutations and actions.
//!
//! The TUI event loop forwards every pressed key here. The handler walks the
//! same precedence the user sees on screen: global quit shortcuts, active modal
//! capture (help, theme picker, delete confirmation, filter mode), then the
//! main shortcut set.

use crate::{
    app::{Action, App, BusyState, MsgKind, VisualAddition, state::SystemPrompt},
    logging,
    theme::{Theme, ThemeName, ThemePickerView},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
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
        return handle_help_overlay_key(key, app, action_tx);
    }
    if app.state.show_theme_picker {
        return handle_theme_picker_key(key, app);
    }
    if let Some(target) = app.state.confirming_delete.take() {
        return handle_confirm_delete_key(key, app, action_tx, &target);
    }
    if let Some(outcome) = handle_command_help_key(key, app) {
        return outcome;
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
    if let Some(prompt) = app.state.system_prompt {
        return handle_system_prompt_key(key, app, action_tx, prompt);
    }

    handle_main_shortcut_key(key, app, action_tx)
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Handles keys while the PATH-setup help overlay is open.
///
/// The overlay captures every OTHER key until dismissed, EXCEPT 'q' which
/// quits the app and 'f' which applies the permanent PATH fix. The overlay
/// stays open on 'f' so the result notice is shown inside it.
///
/// 'f' is dispatched as [`Action::FixPath`] rather than calling the manager
/// directly, so both fix-path entry points share one handler — and therefore one
/// log line, one notice, and one `is_shim_in_path` refresh.
fn handle_help_overlay_key(
    key: KeyEvent,
    app: &mut App,
    action_tx: &UnboundedSender<Action>,
) -> KeyOutcome {
    match key.code {
        KeyCode::Char('q') => KeyOutcome::Quit,
        KeyCode::Char('f') => {
            let _ = action_tx.send(Action::FixPath);
            KeyOutcome::Continue
        }
        _ => {
            app.state.show_help = false;
            app.state.path_fix_notice = None;
            KeyOutcome::Continue
        }
    }
}

/// Handles keys while the theme picker is open.
///
/// The picker is a two-level browser: at the folder level Enter/Right opens
/// the highlighted Dark/Light family; inside a family Enter saves the
/// highlighted theme, while Left/Esc step back out (Esc at the folder level
/// cancels the whole picker and restores the saved theme).
fn handle_theme_picker_key(key: KeyEvent, app: &mut App) -> KeyOutcome {
    let in_family = matches!(app.state.theme_picker.view, ThemePickerView::Family(_));
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            if in_family {
                app.picker_back();
            } else {
                app.picker_cancel();
            }
        }
        KeyCode::Enter | KeyCode::Right => app.picker_enter(),
        KeyCode::Left if in_family => app.picker_back(),
        KeyCode::Down | KeyCode::Char('j') => app.picker_move(1),
        KeyCode::Up | KeyCode::Char('k') => app.picker_move(-1),
        KeyCode::Char(c) if in_family && c.is_ascii_digit() && c != '0' => {
            if let ThemePickerView::Family(family) = app.state.theme_picker.view {
                let i = (c as u8 - b'1') as usize;
                if i < ThemeName::in_family(family).len() {
                    app.state.theme_picker.theme_cursor = i;
                    app.state.theme = Theme::for_name(app.picker_theme());
                    app.picker_apply();
                }
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

/// Handles keys while the right-docked keyboard help panel is open.
///
/// While the panel is shown it swallows navigation keys (so they scroll the
/// command catalogue instead of the version list) plus `?`/Esc to close it and
/// `q`/Ctrl-C to quit. `u` and `x` launch the self-update / self-uninstall
/// confirmations (they mean those maintenance actions only while this panel is
/// open — on the main dashboard `u` switches the active version). Any other key
/// is ignored so reading the list can never trigger an unrelated action.
fn handle_command_help_key(key: KeyEvent, app: &mut App) -> Option<KeyOutcome> {
    if !app.state.show_command_help {
        return None;
    }
    Some(match key.code {
        KeyCode::Char('?') | KeyCode::Esc => {
            app.close_command_help();
            KeyOutcome::Continue
        }
        KeyCode::Char('U') => {
            app.state.system_prompt = Some(SystemPrompt::Update);
            app.close_command_help();
            KeyOutcome::Continue
        }
        KeyCode::Char('X') => {
            app.state.system_prompt = Some(SystemPrompt::UninstallKeep);
            app.close_command_help();
            KeyOutcome::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.scroll_command_help(1);
            KeyOutcome::Continue
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.scroll_command_help(-1);
            KeyOutcome::Continue
        }
        KeyCode::PageDown => {
            app.scroll_command_help(8);
            KeyOutcome::Continue
        }
        KeyCode::PageUp => {
            app.scroll_command_help(-8);
            KeyOutcome::Continue
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.scroll_command_help(i64::MIN);
            KeyOutcome::Continue
        }
        KeyCode::End | KeyCode::Char('G') => {
            app.scroll_command_help(i64::MAX);
            KeyOutcome::Continue
        }
        KeyCode::Char('q') => KeyOutcome::Quit,
        _ => KeyOutcome::Continue,
    })
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
///
/// Only the accepted cancel is logged. A DEBUG line per keypress during a
/// multi-minute install drowned the audit trail in noise (one line for every
/// stray keystroke) without ever saying anything the outcome didn't.
fn handle_install_cancel_key(key: KeyEvent, app: &mut App) -> Option<KeyOutcome> {
    let cancel_pressed = key.code == KeyCode::Esc || key.code == KeyCode::Char('c');
    if app.state.cancel_install.is_none() || !cancel_pressed {
        return None;
    }
    let version = app
        .state
        .busy
        .as_ref()
        .and_then(BusyState::target)
        .unwrap_or("unknown")
        .to_string();
    if let Some(tx) = app.state.cancel_install.take() {
        let _ = tx.send(true);
        app.set_status("Cancelling installation...", MsgKind::Info);
        logging::debug(&format!(
            "install: cancel requested version={version} key={:?}",
            key.code
        ));
    }
    Some(KeyOutcome::Continue)
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
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.clear_log_display();
            KeyOutcome::Continue
        }
        KeyCode::Char('-') => {
            let anchor = logging::read_lines().len();
            app.state.log_visual_additions.push(VisualAddition {
                text: "-".repeat(50),
                anchor,
            });
            app.refresh_logs();
            KeyOutcome::Continue
        }
        KeyCode::Enter => {
            let anchor = logging::read_lines().len();
            app.state.log_visual_additions.push(VisualAddition {
                text: String::new(),
                anchor,
            });
            app.refresh_logs();
            KeyOutcome::Continue
        }
        KeyCode::Char('q') => KeyOutcome::Quit,
        _ => KeyOutcome::Continue,
    })
}

fn handle_system_prompt_key(
    key: KeyEvent,
    app: &mut App,
    action_tx: &UnboundedSender<Action>,
    prompt: SystemPrompt,
) -> KeyOutcome {
    match key.code {
        KeyCode::Char('y' | 'Y') => {
            match prompt {
                SystemPrompt::Update => {
                    let _ = action_tx.send(Action::Update);
                    app.state.system_prompt = None;
                }
                SystemPrompt::UninstallKeep => {
                    // First yes = proceed to purge question
                    app.state.system_prompt = Some(SystemPrompt::UninstallPurge);
                }
                SystemPrompt::UninstallPurge => {
                    // Second yes = actually uninstall with purge
                    let _ = action_tx.send(Action::Uninstall(true));
                    app.state.system_prompt = None;
                }
            }
        }
        // 'p' only works on the first uninstall prompt to escalate to Purge
        KeyCode::Char('p' | 'P') if prompt == SystemPrompt::UninstallKeep => {
            app.state.system_prompt = Some(SystemPrompt::UninstallPurge);
        }
        KeyCode::Char('n' | 'N') => {
            match prompt {
                // On second prompt, n means "No purge, just remove binary"
                SystemPrompt::UninstallPurge => {
                    let _ = action_tx.send(Action::UninstallBinaryOnly);
                    app.state.system_prompt = None;
                }
                // Any other command (e.g., SystemPrompt::UninstallKeep) n cancels entirely
                _ => {
                    app.state.system_prompt = None;
                }
            }
        }
        // Esc and c ALWAYS cancel completely
        KeyCode::Esc | KeyCode::Char('c') => {
            app.state.system_prompt = None;
        }
        _ => {}
    }
    KeyOutcome::Continue
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
        KeyCode::Char('/') if !app.is_busy() => {
            app.state.filter_mode = true;
        }
        KeyCode::Char('T') => {
            app.open_theme_picker();
        }
        KeyCode::Char('L') => {
            app.open_logs();
        }
        KeyCode::Char('?') => {
            app.toggle_command_help();
        }
        KeyCode::Char('h') => {
            // The PATH-setup overlay is only relevant while the shim is missing;
            if !app.state.is_shim_in_path {
                app.state.show_help = true;
            }
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
        KeyCode::Char('U') => {
            app.state.system_prompt = Some(SystemPrompt::Update);
        }
        KeyCode::Char('X') => {
            app.state.system_prompt = Some(SystemPrompt::UninstallKeep);
        }
        _ => {}
    }

    KeyOutcome::Continue
}
