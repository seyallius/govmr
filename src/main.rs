//! Copyright (c) 2026 SeyedAli
//! Licensed under the MIT License. See LICENSE file in the project root for details.
//
//! GoVMR - Go Version Manager in Rust.
//!
//! Provides CLI and interactive TUI tooling to fetch, install, switch,
//! and manage multiple Go toolchain versions seamlessly.

use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use govmr::{
    app::{Action, App, BusyState, MsgKind, Phase},
    cli::{handle_cli, Cli},
    logging,
    manager::GoManager,
    tui,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, sync::Arc, time::Duration};
use tokio::sync::mpsc;

// ------------------------------------------- <Main> ------------------------------------------- //

/// Main runtime entry point initializing terminal rendering or executing CLI commands.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli_args = Cli::parse();
    let manager = Arc::new(GoManager::new()?);
    logging::init();

    if cli_args.command.is_some() {
        logging::info(&format!(
            "govmr {} started (cli mode)",
            env!("CARGO_PKG_VERSION")
        ));
        let result = handle_cli(cli_args, manager).await;
        if let Err(e) = &result {
            logging::error(&format!("cli command failed: {e}"));
        }
        return result;
    }
    logging::info(&format!(
        "govmr {} started (tui mode)",
        env!("CARGO_PKG_VERSION")
    ));

    // Terminal initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Ensure the terminal is restored even if a panic occurs mid-run.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        default_hook(info);
    }));

    let result = run_tui(&mut terminal, manager).await;

    // Cleanup terminal state
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Runs the interactive dashboard event loop until the user quits.
async fn run_tui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    manager: Arc<GoManager>,
) -> anyhow::Result<()> {
    let shim_path = manager
        .get_shim_manager()
        .get_shim_dir()
        .to_string_lossy()
        .to_string();
    let shim_in_path = manager.get_shim_manager().is_in_path();
    let initial_theme = manager.theme();
    tui::setup::run_setup_guide_if_needed(terminal, &shim_path, shim_in_path, &initial_theme)
        .await?;

    let mut app = App::new(manager.clone(), shim_path).await;
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();

    loop {
        terminal.draw(|f| {
            tui::views::render(f, &mut app.state);
            tui::views::render_overlays(f, &app.state);
        })?;
        handle_actions(&mut action_rx, &mut app, &manager, &action_tx).await?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Only react on key press (not release/repeat artifacts).
                if key.kind == KeyEventKind::Release {
                    continue;
                }

                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }

                // Help overlay captures every key until dismissed.
                if app.state.show_help {
                    app.state.show_help = false;
                    continue;
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
                            if i < govmr::theme::ThemeName::ALL.len() {
                                app.state.theme_picker_index = i;
                                app.state.theme = govmr::theme::Theme::for_name(app.picker_theme());
                                app.picker_apply();
                            }
                        }
                        _ => {}
                    }
                    continue;
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
                    continue;
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
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Tab => app.switch_tab(),
                    KeyCode::Down | KeyCode::Char('j') => app.next_item(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous_item(),
                    KeyCode::Char('/') => {
                        app.state.filter_mode = true;
                    }
                    KeyCode::Char('T') => {
                        app.open_theme_picker();
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
                                app.set_status(
                                    "Install this version first — press i",
                                    MsgKind::Error,
                                );
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
            }
        }
    }

    Ok(())
}

/// Drains and processes all queued background actions for one event-loop tick.
///
/// Each arm only manages the busy state of its *own* operation (Switching,
/// Deleting, Installing); the Refreshing flag lives inside `refresh_versions`.
async fn handle_actions(
    action_rx: &mut mpsc::UnboundedReceiver<Action>,
    app: &mut App,
    manager: &Arc<GoManager>,
    action_tx: &mpsc::UnboundedSender<Action>,
) -> anyhow::Result<()> {
    while let Ok(action) = action_rx.try_recv() {
        match action {
            Action::Refresh => {
                app.refresh_versions().await;
            }
            Action::Install(v) => {
                app.state.busy = Some(BusyState::Installing {
                    version: v.raw_version.clone(),
                    phase: Phase::Downloading,
                    downloaded: 0,
                    total: v.size,
                    speed: 0.0,
                    started_at: std::time::Instant::now(),
                });
                app.state.status_message = None;

                let mgr = manager.clone();
                let progress_tx = action_tx.clone();
                let done_tx = action_tx.clone();
                tokio::spawn(async move {
                    let progress_tx2 = progress_tx.clone();
                    let result = mgr
                        .download_and_install(&v, move |p| {
                            let _ = progress_tx2.send(Action::InstallProgress(p));
                        })
                        .await;
                    match result {
                        Ok(_) => {
                            let _ = done_tx.send(Action::InstallDone(v));
                        }
                        Err(e) => {
                            let _ = done_tx.send(Action::InstallFailed(e.to_string()));
                        }
                    }
                });
            }
            Action::InstallProgress(p) => {
                app.update_install_progress(p);
            }
            Action::InstallDone(v) => {
                app.state.busy = None;
                app.set_status(
                    format!("Go {} installed successfully", v.raw_version),
                    MsgKind::Success,
                );
                app.refresh_versions().await;
            }
            Action::InstallFailed(err) => {
                logging::error(&format!("install failed: {err}"));
                app.state.busy = None;
                app.set_status(format!("Installation failed: {}", err), MsgKind::Error);
            }
            Action::Use(v) => {
                app.state.busy = Some(BusyState::Switching(v.raw_version.clone()));
                match manager.switch_version(&v) {
                    Ok(_) => app.set_status(
                        format!("Switched to Go {}", v.raw_version),
                        MsgKind::Success,
                    ),
                    Err(e) => {
                        logging::error(&format!("use failed: {e}"));
                        app.set_status(e.to_string(), MsgKind::Error)
                    }
                }
                app.refresh_versions().await; // exits with busy == None
            }
            Action::Delete(v) => {
                app.state.busy = Some(BusyState::Deleting(v.raw_version.clone()));
                match manager.delete_version(&v) {
                    Ok(_) => {
                        app.set_status(format!("Deleted Go {}", v.raw_version), MsgKind::Success)
                    }
                    Err(e) => {
                        logging::error(&format!("delete failed: {e}"));
                        app.set_status(e.to_string(), MsgKind::Error)
                    }
                }
                app.refresh_versions().await;
            }
        }
    }
    Ok(())
}
