//! GoVMR - Go Version Manager in Rust.
//!
//! Provides CLI and interactive TUI tooling to fetch, install, switch,
//! and manage multiple Go toolchain versions seamlessly.

use clap::Parser;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use govmr::{
    app::{self, Action, App},
    cli::{self, Cli},
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
        let result = cli::handle_cli(cli_args, manager).await;
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

    let should_continue = tui::setup::run_setup_guide_if_needed(
        terminal,
        &shim_path,
        shim_in_path,
        &initial_theme,
        &manager,
    )
    .await?;

    if !should_continue {
        return Ok(());
    }

    let mut app = App::new(manager.clone(), shim_path.clone());
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();

    let _ = action_tx.send(Action::Refresh);

    loop {
        terminal.draw(|f| {
            tui::render(f, &mut app.state);
            tui::render_overlays(f, &app.state);
        })?;
        app::handle_actions(&mut action_rx, &mut app, &manager, &action_tx).await?;

        // Keep the log viewer's contents fresh while it is open (throttled).
        app.refresh_logs_if_open();

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if let app::KeyOutcome::Quit = app::handle_key(key, &mut app, &action_tx) {
                    break;
                }
            }
        }
    }

    Ok(())
}
