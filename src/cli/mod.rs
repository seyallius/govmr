//! Module cli - Command-line interface definitions and subcommand handlers.

mod output;
mod progress;

use crate::{
    manager::GoManager,
    theme::ThemeName,
    version::{GoVersion, resolve_version},
};
use anyhow::Result;
use clap::{Parser, Subcommand};
use indicatif::ProgressStyle;
use output::{CYAN, GREEN, GREY, RED, RESET, YELLOW, paint};
use progress::CliProgress;
use std::sync::Arc;

// ------------------------------------------ Types & Impls ------------------------------------- //

/// CLI argument parser configuration for `GoVMR`.
#[derive(Parser)]
#[command(name = "govmr", about = "Go Version Manager in Rust", version)]
pub struct Cli {
    /// Optional subcommand to execute. If omitted, launches the interactive TUI.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Available subcommands for command-line operations.
#[derive(Subcommand)]
pub enum Commands {
    /// Download and install a specified Go version.
    Install {
        /// Target version or prefix (e.g. "1.22", "1.21.6").
        version: String,
    },
    /// Switch the active system Go version to an installed release.
    Use {
        /// Installed version to activate.
        version: String,
    },
    /// Remove an installed Go version from disk.
    Delete {
        /// Version to uninstall.
        version: String,
    },
    /// List all locally installed Go versions.
    List,
    /// View or change the TUI color theme.
    Theme {
        /// Apply a theme by name (e.g. `midnight`); omit to list themes.
        #[arg(value_parser = parse_theme)]
        name: Option<ThemeName>,
    },
}

// ----------------------------------------- Public API ----------------------------------------- //

/// Clap value parser mapping a theme key/title to a [`ThemeName`].
fn parse_theme(raw: &str) -> Result<ThemeName, String> {
    ThemeName::from_key(raw).ok_or_else(|| {
        format!(
            "unknown theme '{}'. Available: {}",
            raw,
            ThemeName::ALL
                .iter()
                .map(|t| t.key())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

/// Dispatches execution based on the parsed CLI subcommand.
///
/// # Arguments
/// * `cli` - The parsed CLI command structure.
/// * `manager` - Thread-safe reference to the `GoManager`.
///
/// # Errors
/// Returns [`anyhow::Error`] if requested versions cannot be resolved or operations fail.
pub async fn handle_cli(cli: Cli, manager: Arc<GoManager>) -> Result<()> {
    match cli.command {
        Some(Commands::Install { version }) => cmd_install(&manager, &version).await?,
        Some(Commands::Use { version }) => cmd_use(&manager, &version).await?,
        Some(Commands::Delete { version }) => cmd_delete(&manager, &version).await?,
        Some(Commands::List) => cmd_list(&manager).await?,
        Some(Commands::Theme { name }) => cmd_theme(&manager, name)?,
        None => unreachable!(),
    }
    Ok(())
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Downloads and installs the requested Go version, showing live CLI progress.
///
/// # Errors
/// Returns [`anyhow::Error`] if the version cannot be resolved or the install fails.
async fn cmd_install(manager: &GoManager, version: &str) -> Result<()> {
    let clean_ver = version.trim_start_matches("go");
    println!(
        "{} Resolving Go version {}{}{}…",
        paint(CYAN, "🔍"),
        CYAN,
        clean_ver,
        RESET
    );
    let versions = manager.fetch_versions().await?;
    let target = resolve_version(clean_ver, &versions)
        .ok_or_else(|| anyhow::anyhow!("Version {clean_ver} not found"))?;

    // The bar starts as a download gauge; it is swapped for a spinner
    // once the archive finishes and extraction begins.
    let bar_style = ProgressStyle::with_template(
        "  {spinner:.cyan} {bar:40.cyan/blue} {percent:>3}%\n  {bytes:>10} / {total_bytes:<10} {bytes_per_sec:<12} eta {eta}",
    )?
    .progress_chars("█▓▒░ ");
    let spin_style = ProgressStyle::with_template("  {spinner:.green} {msg}")?;

    let progress = Arc::new(CliProgress::new(bar_style, spin_style));
    let progress_cb = progress.clone();

    manager
        .download_and_install(target, move |event| progress_cb.on_event(event))
        .await?;

    progress.finish();
    println!(
        "{} Successfully installed & activated {}",
        paint(GREEN, "✅"),
        paint(CYAN, &format!("Go {}", target.raw_version))
    );
    println!(
        "  {} Archive cleaned to save space. Shim generated at ~/.govmr/shim/",
        paint(GREY, "ℹ️")
    );
    Ok(())
}

/// Activates an installed Go version.
///
/// # Errors
/// Returns [`anyhow::Error`] if the version cannot be resolved or switching fails.
async fn cmd_use(manager: &GoManager, version: &str) -> Result<()> {
    let clean_ver = version.trim_start_matches("go");
    let versions = manager.fetch_versions().await?;
    let installed: Vec<GoVersion> = versions.into_iter().filter(|v| v.installed).collect();
    let target = resolve_version(clean_ver, &installed)
        .ok_or_else(|| anyhow::anyhow!("Installed version {clean_ver} not found"))?;

    let in_path = manager.switch_version(target)?;
    println!(
        "{} Switched to {}",
        paint(GREEN, "✅"),
        paint(CYAN, &format!("Go {}", target.raw_version))
    );
    if !in_path {
        println!(
            "{} GoVMR shim is not in your PATH. Please configure your shell.",
            paint(YELLOW, "⚠️")
        );
    }
    Ok(())
}

/// Removes an installed Go version from disk.
///
/// # Errors
/// Returns [`anyhow::Error`] if the version cannot be resolved or deletion fails.
async fn cmd_delete(manager: &GoManager, version: &str) -> Result<()> {
    let clean_ver = version.trim_start_matches("go");
    let versions = manager.fetch_versions().await?;
    let installed: Vec<GoVersion> = versions.into_iter().filter(|v| v.installed).collect();
    let target = resolve_version(clean_ver, &installed)
        .ok_or_else(|| anyhow::anyhow!("Installed version {clean_ver} not found"))?;

    manager.delete_version(target)?;
    println!(
        "{} Successfully deleted {}",
        paint(GREEN, "🗑️ "),
        paint(RED, &format!("Go {}", target.raw_version))
    );
    Ok(())
}

/// Lists all locally installed Go versions.
///
/// # Errors
/// Returns [`anyhow::Error`] if the version manifest cannot be fetched.
async fn cmd_list(manager: &GoManager) -> Result<()> {
    let versions = manager.fetch_versions().await?;
    let installed: Vec<_> = versions.into_iter().filter(|v| v.installed).collect();
    if installed.is_empty() {
        println!(
            "{} No Go versions installed yet. Try {} to install one.",
            paint(YELLOW, "ℹ️"),
            paint(CYAN, "govmr install <version>")
        );
        return Ok(());
    }
    println!("{}", paint(CYAN, "Installed Go versions:"));
    for v in installed {
        if v.active {
            println!(
                "  {} {} {}",
                paint(GREEN, "●"),
                paint(GREEN, &v.raw_version),
                paint(GREEN, "(active)")
            );
        } else {
            println!("  {} {}", paint(GREY, "○"), paint(GREY, &v.raw_version));
        }
    }
    Ok(())
}

/// Applies a theme by name, or lists the available themes when no name is given.
///
/// # Errors
/// Returns [`anyhow::Error`] if the supplied theme cannot be persisted.
fn cmd_theme(manager: &GoManager, name: Option<ThemeName>) -> Result<()> {
    if let Some(name) = name {
        manager.set_theme(name)?;
        println!(
            "{} Theme set to {} (saved to ~/.govmr/config.toml)",
            paint(GREEN, "🎨"),
            paint(CYAN, name.title())
        );
    } else {
        let current = manager.theme_name();
        println!("{}", paint(CYAN, "Available themes:"));
        for t in ThemeName::ALL {
            if t == current {
                println!(
                    "  {} {} {}",
                    paint(GREEN, "●"),
                    paint(GREEN, t.key()),
                    paint(GREEN, "(current)")
                );
            } else {
                println!(
                    "  {} {:<10} {}",
                    paint(GREY, "○"),
                    t.key(),
                    paint(GREY, t.title())
                );
            }
        }
        println!(
            "\nSwitch with {} — e.g. {}",
            paint(CYAN, "govmr theme <name>"),
            paint(CYAN, "govmr theme midnight")
        );
    }
    Ok(())
}
