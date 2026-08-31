//! Copyright (c) 2026 SeyedAli
//! Licensed under the MIT License. See LICENSE file in the project root for details.
//!
//! GoVMR - Go Version Manager in Rust.
//!
//! Provides CLI and interactive TUI tooling to fetch, install, switch,
//! and manage multiple Go toolchain versions seamlessly.

mod app;
mod cli;
mod errors;
mod manager;
mod models;
mod shim;
mod tui;

use app::{Action, ActiveTab, App};
use clap::Parser;
use cli::{Cli, handle_cli};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use manager::GoManager;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Main runtime entry point initializing terminal rendering or executing CLI commands.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli_args = Cli::parse();
    let manager = Arc::new(GoManager::new()?);

    if cli_args.command.is_some() {
        return handle_cli(cli_args, manager).await;
    }

    // Terminal Initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run setup walkthrough if shim is missing
    tui::setup::run_setup_guide_if_needed(&mut terminal, &manager).await?;

    let mut app = App::new(manager.clone()).await;
    let (action_tx, mut action_rx) = mpsc::channel::<Action>(10);

    loop {
        terminal.draw(|f| tui::views::render(f, &mut app.state))?;

        // Background action event handler
        tokio::select! {
            Some(action) = action_rx.recv() => {
                match action {
                    Action::Refresh => {
                        app.refresh_versions().await;
                    }
                    Action::Install(v) => {
                        app.state.loading = true;
                        app.state.action_target = Some(v.raw_version.clone());
                        let mgr = manager.clone();
                        let tx = action_tx.clone();
                        tokio::spawn(async move {
                            let _ = mgr.download_and_install(&v, |_| {}).await;
                            let _ = tx.send(Action::Refresh).await;
                        });
                    }
                    Action::Use(v) => {
                        app.state.loading = true;
                        match manager.switch_version(&v) {
                            Ok(_) => {
                                app.state.status_message = Some((format!("Switched to Go {}", v.raw_version), false));
                            }
                            Err(e) => {
                                app.state.status_message = Some((e.to_string(), true));
                            }
                        }
                        app.refresh_versions().await;
                    }
                    Action::Delete(v) => {
                        app.state.loading = true;
                        match manager.delete_version(&v) {
                            Ok(_) => {
                                app.state.status_message = Some((format!("Deleted Go {}", v.raw_version), false));
                            }
                            Err(e) => {
                                app.state.status_message = Some((e.to_string(), true));
                            }
                        }
                        app.refresh_versions().await;
                    }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(50)) => {
                if event::poll(std::time::Duration::from_millis(10))? {
                    if let Event::Key(key) = event::read()? {
                        // Confirm Delete Dialog interception
                        if let Some(target) = app.state.confirming_delete.take() {
                            match key.code {
                                KeyCode::Char('y') | KeyCode::Char('Y') => {
                                    if let Some(v) = app.state.versions.iter().find(|x| x.raw_version == target).cloned() {
                                        action_tx.send(Action::Delete(v)).await?;
                                    }
                                }
                                _ => {
                                    app.state.status_message = Some(("Delete cancelled".into(), false));
                                }
                            }
                            continue;
                        }

                        match key.code {
                            KeyCode::Char('q') => break,
                            KeyCode::Tab => {
                                app.state.active_tab = match app.state.active_tab {
                                    ActiveTab::Available => ActiveTab::Installed,
                                    ActiveTab::Installed => ActiveTab::Available,
                                };
                            }
                            KeyCode::Down | KeyCode::Char('j') => app.next_item(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous_item(),
                            KeyCode::Char('r') => {
                                action_tx.send(Action::Refresh).await?;
                            }
                            KeyCode::Char('i') => {
                                if let Some(v) = app.selected_version().cloned() {
                                    if !v.installed {
                                        action_tx.send(Action::Install(v)).await?;
                                    }
                                }
                            }
                            KeyCode::Char('u') => {
                                if let Some(v) = app.selected_version().cloned() {
                                    if v.installed {
                                        action_tx.send(Action::Use(v)).await?;
                                    } else {
                                        app.state.status_message = Some(("Install version first [i]".into(), true));
                                    }
                                }
                            }
                            KeyCode::Char('d') => {
                                if let Some(v) = app.selected_version() {
                                    if v.active {
                                        app.state.status_message = Some(("Cannot delete active version".into(), true));
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
        }
    }

    // Cleanup Terminal state
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
