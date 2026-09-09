//! `GoVMR` - Go Version Manager in Rust.
//!
//! Provides CLI and interactive TUI tooling to fetch, install, switch,
//! and manage multiple Go toolchain versions seamlessly.

use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use govmr::{app::{self, Action, App}, cli::{self, Cli}, completions, config, logging, manager::GoManager, tui};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{env::consts::{ARCH, OS}, io, sync::atomic::{AtomicBool, Ordering}, sync::Arc, time::Duration};
use tokio::sync::mpsc;

// ------------------------------------------- <Main> ------------------------------------------- //

/// Main runtime entry point initializing terminal rendering or executing CLI commands.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Argument parsing stays first: `--help`/`--version`/parse errors exit here
    // and must not open (or pollute) the audit log. Everything fallible that
    // follows is covered by it, because logging is initialised before `GoManager::new()`.
    let cli_args = Cli::parse();
    logging::init();
    install_panic_hook();

    let mode = if cli_args.command.is_some() {
        "cli"
    } else {
        "tui"
    };
    // argv is recorded for CLI runs only: it is the one input the log can never
    // recover afterwards (which subcommand, with which arguments).
    let args = if cli_args.command.is_some() {
        let joined = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
        format!(" args=\"{joined}\"")
    } else {
        String::new()
    };
    logging::info(&format!(
        "govmr {} started: mode={mode} os={OS} arch={ARCH}{args}",
        config::current_govmr_version()
    ));

    // `GoManager::new()` resolves $HOME, creates ~/.govmr subdirectories, loads the
    // config and builds the HTTP client — the one fallible step that runs before any
    // error handler exists, so it reports its own failure to the log.
    let manager = match GoManager::new() {
        Ok(manager) => Arc::new(manager),
        Err(e) => {
            logging::error(&format!("startup failed: area=manager error=\"{e}\""));
            log_session_end("startup", None);
            return Err(e.into());
        }
    };

    // Content-aware completion generation: rewrites scripts only when the
    // CLI definition actually changed, and ensures symlinks are intact.
    completions::ensure_completions();

    if cli_args.command.is_some() {
        let result = cli::handle_cli(cli_args, manager.clone()).await;
        if let Err(e) = &result {
            logging::error(&format!("cli: failed error=\"{e}\""));
        }
        log_session_end(
            if result.is_ok() { "complete" } else { "error" },
            Some(&manager),
        );
        return result;
    }

    // Terminal initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_tui(&mut terminal, manager.clone()).await;

    // Cleanup terminal state. Best-effort on purpose: a terminal that is already
    // gone (detached pty, closed pipe) must not mask the loop's real result.
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    // Session bookend, so the log alone answers "what did this run end up with?".
    match &result {
        Ok(reason) => log_session_end(reason.as_str(), Some(&manager)),
        Err(e) => {
            logging::error(&format!("tui run failed: error=\"{e}\""));
            log_session_end("error", Some(&manager));
        }
    }

    result.map(|_| ())
}

// -------------------------------------- Types & Impls --------------------------------------- //

/// Why the interactive loop stopped; rendered into the session bookend line.
#[derive(Clone, Copy)]
enum QuitReason {
    /// The user quit with `q` (dashboard or help overlay).
    Quit,
    /// The user quit with `Ctrl-C`.
    Interrupt,
}
impl QuitReason {
    /// Log-safe identifier for this reason.
    fn as_str(self) -> &'static str {
        match self {
            QuitReason::Quit => "quit",
            QuitReason::Interrupt => "interrupt",
        }
    }
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Runs the interactive dashboard event loop until the user quits.
async fn run_tui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    manager: Arc<GoManager>,
) -> anyhow::Result<QuitReason> {
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
    )?;
    if !should_continue {
        // The setup guide owns the "why" (quit vs dismissed) in its own lines.
        return Ok(QuitReason::Quit);
    }

    let mut app = App::new(manager.clone(), shim_path.clone());
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();
    let _ = action_tx.send(Action::Refresh);

    let reason = loop {
        terminal.draw(|f| {
            tui::render(f, &mut app.state);
            tui::render_overlays(f, &app.state);
        })?;

        app::handle_actions(&mut action_rx, &mut app, &manager, &action_tx).await?;

        // Keep the log viewer's contents fresh while it is open (throttled).
        app.refresh_logs_if_open();

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && let app::KeyOutcome::Quit = app::handle_key(key, &mut app, &action_tx)
        {
            let interrupted =
                key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
            break if interrupted {
                QuitReason::Interrupt
            } else {
                QuitReason::Quit
            };
        }
    };

    Ok(reason)
}

/// Writes the closing bookend line: why the session ended and which toolchains it
/// left installed/active.
///
/// `manager` is absent on the two paths that never got a working app — startup
/// failure and panic — where the summary is reported as `unknown` rather than
/// pretending nothing was installed.
fn log_session_end(reason: &str, manager: Option<&GoManager>) {
    let (installed, active) = match manager {
        Some(manager) => {
            let versions = manager.installed_versions();
            let installed = if versions.is_empty() {
                "[]".to_string()
            } else {
                format!("[{}]", versions.join(", "))
            };
            (
                installed,
                manager
                    .get_active_version()
                    .unwrap_or_else(|| "none".to_string()),
            )
        }
        None => ("unknown".to_string(), "unknown".to_string()),
    };
    logging::info(&format!(
        "session ended: reason={reason} installed={installed} active={active}"
    ));
}

/// Restores the terminal *and* records the panic in the audit log before chaining to
/// the default hook, so a crash still leaves a post-mortem behind.
fn install_panic_hook() {
    // Installed before anything can panic (including `GoManager::new()`), so
    // `logging::init()` has already run and the payload has somewhere to go.
    static PANIC_LOGGED: AtomicBool = AtomicBool::new(false);

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);

        // A panic during unwinding can re-enter the hook; the first report is
        // the informative one, and the logger must not be shouted into twice.
        if !PANIC_LOGGED.swap(true, Ordering::SeqCst) {
            logging::error(&format!("panic: {}", panic_report(info)));
            log_session_end("panic", None);
        }

        default_hook(info);
    }));
}

/// Renders the panic payload plus its source location as greppable fields.
fn panic_report(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = info
        .payload()
        .downcast_ref::<&str>()
        .map_or_else(
            || info.payload().downcast_ref::<String>().cloned(),
            |s| Some((*s).to_string()),
        )
        .unwrap_or_else(|| "<non-string payload>".to_string());
    let location = info.location().map_or_else(
        || "unknown".to_string(),
        |loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()),
    );
    format!("message=\"{payload}\" location=\"{location}\"")
}
