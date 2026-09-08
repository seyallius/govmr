//! Module completions - Automatic, idempotent generation of shell completion scripts.

use crate::{cli::Cli, logging};
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use std::fs;
use std::path::PathBuf;

// ----------------------------------------- Public API ----------------------------------------- //

/// Generates shell completion scripts for the active shell if they don't already exist.
///
/// This function is idempotent: it only creates the files on the first run or if they
/// have been manually deleted. It detects the user's active shell via the `$SHELL`
/// environment variable to avoid cluttering the system with scripts for unused shells.
pub fn ensure_completions() {
    logging::debug("checking shell completions...");
    let paths = get_completion_paths();

    if paths.is_empty() {
        logging::debug("no completion paths resolved, skipping generation");
        return;
    }

    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();

    for (shell, path) in paths {
        // Idempotent: skip if the completion script already exists
        if path.exists() {
            logging::debug(&format!(
                "{} completions already exist at {}, skipping",
                shell,
                path.display()
            ));
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

/// Returns a list of (Shell, Path) tuples for the user's active shell.
///
/// Inspects the `$SHELL` environment variable to determine which shell the user
/// is currently running. If it cannot be determined, it falls back to generating
/// completions for all supported shells (and logs a warning).
fn get_completion_paths() -> Vec<(Shell, PathBuf)> {
    let mut paths = Vec::new();

    let shell_env = std::env::var("SHELL").unwrap_or_default();
    let shell_name = shell_env.rsplit('/').next().unwrap_or("").to_lowercase();

    let target_shell = match shell_name.as_str() {
        s if s.contains("bash") => Some(Shell::Bash),
        s if s.contains("zsh") => Some(Shell::Zsh),
        s if s.contains("fish") => Some(Shell::Fish),
        s if s.contains("pwsh") || s.contains("powershell") => Some(Shell::PowerShell),
        _ => None,
    };

    if let Some(sh) = target_shell {
        logging::debug(&format!("detected active shell: {sh} ($SHELL={shell_env})"));
    } else if !shell_env.is_empty() {
        logging::warn(&format!(
            "unrecognized shell '{shell_env}' in $SHELL, falling back to generating all supported completions",
        ));
    } else {
        logging::debug("$SHELL is empty, falling back to generating all supported completions");
    }

    #[cfg(unix)]
    {
        if let Some(data) = dirs::data_dir() {
            if target_shell == Some(Shell::Bash) || target_shell.is_none() {
                paths.push((
                    Shell::Bash,
                    data.join("bash-completion")
                        .join("completions")
                        .join("govmr"),
                ));
            }
            if target_shell == Some(Shell::Zsh) || target_shell.is_none() {
                paths.push((
                    Shell::Zsh,
                    data.join("zsh").join("site-functions").join("_govmr"),
                ));
            }
        }
        if let Some(config) = dirs::config_dir() {
            if target_shell == Some(Shell::Fish) || target_shell.is_none() {
                paths.push((
                    Shell::Fish,
                    config.join("fish").join("completions").join("govmr.fish"),
                ));
            }
            if target_shell == Some(Shell::PowerShell) || target_shell.is_none() {
                paths.push((
                    Shell::PowerShell,
                    config.join("powershell").join("govmr.ps1"),
                ));
            }
        }
    }

    #[cfg(windows)]
    {
        // On Windows, default to PowerShell unless Git Bash is explicitly detected.
        let is_bash = target_shell == Some(Shell::Bash);

        if let Some(docs) = dirs::document_dir() {
            if !is_bash {
                // PowerShell 5.1 (Windows PowerShell)
                paths.push((
                    Shell::PowerShell,
                    docs.join("WindowsPowerShell").join("govmr.ps1"),
                ));
                // PowerShell 6+ (PowerShell Core)
                paths.push((Shell::PowerShell, docs.join("PowerShell").join("govmr.ps1")));
            }
        }
    }

    paths
}
