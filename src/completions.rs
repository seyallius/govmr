//! Module completions - Automatic, idempotent generation of shell completion scripts.

use crate::{cli::Cli, logging};
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use std::fs;
use std::path::PathBuf;

// ----------------------------------------- Public API ----------------------------------------- //

/// Generates shell completion scripts for supported shells if they don't already exist.
///
/// This function is idempotent: it only creates the files on the first run or if they
/// have been manually deleted. It writes to standard user-local directories so that
/// shells can discover them automatically.
pub fn ensure_completions() {
    let paths = get_completion_paths();
    if paths.is_empty() {
        return;
    }

    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();

    for (shell, path) in paths {
        // Idempotent: skip if the completion script already exists
        if path.exists() {
            continue;
        }

        // Ensure parent directories exist
        if let Some(parent) = path.parent()
            && let Err(e) = fs::create_dir_all(parent)
        {
            logging::warn(&format!(
                "failed to create completion directory {}: {}",
                parent.display(),
                e
            ));
            continue;
        }

        match fs::File::create(&path) {
            Ok(mut file) => {
                generate(shell, &mut cmd, &name, &mut file);
                logging::info(&format!(
                    "generated {} completions at {}",
                    shell,
                    path.display()
                ));
            }
            Err(e) => {
                logging::warn(&format!(
                    "failed to write {} completions to {}: {}",
                    shell,
                    path.display(),
                    e
                ));
            }
        }
    }
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Returns a list of (Shell, Path) tuples for standard user-local completion directories.
fn get_completion_paths() -> Vec<(Shell, PathBuf)> {
    let mut paths = Vec::new();

    #[cfg(unix)]
    {
        if let Some(data) = dirs::data_dir() {
            paths.push((
                Shell::Bash,
                data.join("bash-completion")
                    .join("completions")
                    .join("govmr"),
            ));
            paths.push((
                Shell::Zsh,
                data.join("zsh").join("site-functions").join("_govmr"),
            ));
        }
        if let Some(config) = dirs::config_dir() {
            paths.push((
                Shell::Fish,
                config.join("fish").join("completions").join("govmr.fish"),
            ));
            // PowerShell Core on Linux/macOS
            paths.push((
                Shell::PowerShell,
                config.join("powershell").join("govmr.ps1"),
            ));
        }
    }

    #[cfg(windows)]
    {
        if let Some(docs) = dirs::document_dir() {
            // PowerShell 5.1 (Windows PowerShell)
            paths.push((
                Shell::PowerShell,
                docs.join("WindowsPowerShell").join("govmr.ps1"),
            ));
            // PowerShell 6+ (PowerShell Core)
            paths.push((Shell::PowerShell, docs.join("PowerShell").join("govmr.ps1")));
        }
    }

    paths
}
