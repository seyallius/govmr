//! Module completions - Content-aware shell completion lifecycle management.
//!
//! Completions are generated into `~/.govmr/completions/` and exposed to shells
//! via symlinks in their standard discovery directories. This keeps every govmr
//! artifact under one roof while still being auto-discovered by Bash, Zsh, Fish,
//! and PowerShell.
//!
//! Staleness is detected by comparing generated content against the on-disk file,
//! so any CLI change (new commands, flags, renames) triggers a refresh regardless
//! of whether the version string changed.

use crate::{cli::Cli, logging};
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------- Types ----------------------------------------- //

/// A single shell completion target: which shell, what the canonical file is
/// called inside `~/.govmr/completions/`, and where the shell expects to find it.
struct CompletionTarget {
    /// The `clap_complete` shell identifier.
    shell: Shell,
    /// Filename inside `~/.govmr/completions/` (e.g. `"govmr.bash"`).
    canonical_name: &'static str,
    /// Path where the shell auto-discovers completions (symlink target on Unix).
    link_path: PathBuf,
}

// ----------------------------------------- Public API ----------------------------------------- //

/// Ensures shell completions are generated, current, and linked into the
/// shell's discovery directory.
///
/// Rather than checking file existence, this generates the completion script
/// in memory and compares it byte-for-byte with the canonical file on disk.
/// Any CLI change — new subcommands, renamed flags, version bumps — produces
/// different content and triggers a rewrite. The symlink is then verified
/// (or recreated) so the shell always sees the latest script.
///
/// Called once at startup from `main()`. Safe to call repeatedly: unchanged
/// completions produce only debug-level log noise.
pub fn ensure_completions() {
    logging::debug("completions: check started");

    let targets = detect_shell_targets();
    if targets.is_empty() {
        logging::debug("completions: skipped reason=no_targets_detected");
        return;
    }

    let Some(completions_dir) = get_completions_dir() else {
        logging::warn("completions: skipped reason=no_home_dir");
        return;
    };

    if let Err(e) = fs::create_dir_all(&completions_dir) {
        logging::warn(&format!(
            "completions: create dir failed path=\"{}\" error={e}",
            completions_dir.display()
        ));
        return;
    }

    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();

    for target in &targets {
        //  Generate in memory
        let mut buf = Vec::new();
        generate(target.shell, &mut cmd, &name, &mut buf);

        // Content-aware staleness check
        let canonical = completions_dir.join(target.canonical_name);
        let stale = match fs::read(&canonical) {
            Ok(existing) => existing != buf,
            Err(_) => true, // file missing or unreadable → treat as stale
        };

        if stale {
            if let Err(e) = fs::write(&canonical, &buf) {
                logging::warn(&format!(
                    "completions: write failed path=\"{}\" error={e}",
                    canonical.display()
                ));
                continue;
            }
            logging::info(&format!(
                "completions: updated shell={} path=\"{}\" note=reload_shell_to_apply",
                target.shell,
                canonical.display()
            ));
        } else {
            logging::debug(&format!("completions: up to date shell={}", target.shell));
        }

        // Ensure the shell-facing link is correct
        if !is_link_valid(&target.link_path, &canonical) {
            match create_link(&canonical, &target.link_path) {
                Ok(()) => logging::debug(&format!(
                    "completions: linked link=\"{}\" target=\"{}\"",
                    target.link_path.display(),
                    canonical.display()
                )),
                Err(e) => logging::warn(&format!(
                    "completions: link failed link=\"{}\" error={e}",
                    target.link_path.display()
                )),
            }
        }
    }
}

/// Removes every shell completion symlink and the canonical scripts directory.
///
/// Called during uninstall so no orphaned completion files survive, regardless
/// of whether the user chose to purge `~/.govmr`.
pub fn remove_completions() {
    logging::debug("completions: removal started");

    for target in &all_possible_targets() {
        // Remove the shell-facing link (symlink on Unix, copied file on Windows).
        if target.link_path.symlink_metadata().is_ok() {
            match fs::remove_file(&target.link_path) {
                Ok(()) => logging::info(&format!(
                    "completions: removed shell={} link=\"{}\"",
                    target.shell,
                    target.link_path.display()
                )),
                Err(e) => logging::warn(&format!(
                    "completions: remove link failed path=\"{}\" error={e}",
                    target.link_path.display()
                )),
            }
        }
    }

    // Remove the canonical scripts directory.
    if let Some(dir) = get_completions_dir()
        && dir.exists()
    {
        match fs::remove_dir_all(&dir) {
            Ok(()) => logging::info(&format!("completions: removed dir=\"{}\"", dir.display())),
            Err(e) => logging::warn(&format!(
                "completions: remove dir failed path=\"{}\" error={e}",
                dir.display()
            )),
        }
    }
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Returns `~/.govmr/completions/`, the single home for all generated scripts.
fn get_completions_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".govmr").join("completions"))
}

/// Builds completion targets for the user's active shell (detected via `$SHELL`).
///
/// Falls back to all supported shells when `$SHELL` is unset or unrecognised,
/// so completions are never silently skipped.
fn detect_shell_targets() -> Vec<CompletionTarget> {
    let shell_env = std::env::var("SHELL").unwrap_or_default();
    let shell_name = shell_env.rsplit('/').next().unwrap_or("").to_lowercase();

    let detected: Option<&str> = match shell_name.as_str() {
        s if s.contains("bash") => Some("bash"),
        s if s.contains("zsh") => Some("zsh"),
        s if s.contains("fish") => Some("fish"),
        s if s.contains("pwsh") || s.contains("powershell") => Some("powershell"),
        _ => None,
    };

    if let Some(name) = detected {
        logging::debug(&format!(
            "completions: detected shell={name} shell_env=\"{shell_env}\""
        ));
    } else if !shell_env.is_empty() {
        logging::warn(&format!(
            "completions: unrecognised shell_env=\"{shell_env}\" action=generate_for_all_shells"
        ));
    } else {
        logging::debug("completions: shell_env empty; action=generate_for_all_known_shells");
    }

    all_possible_targets()
        .into_iter()
        .filter(|t| {
            detected.is_none() || {
                let key = match t.shell {
                    Shell::Bash => "bash",
                    Shell::Zsh => "zsh",
                    Shell::Fish => "fish",
                    Shell::PowerShell => "powershell",
                    _ => "",
                };
                detected == Some(key)
            }
        })
        .collect()
}

/// Returns completion targets for every supported shell, regardless of `$SHELL`.
///
/// Used by [`remove_completions`] to clean up everything, and by
/// [`detect_shell_targets`] as the base set to filter.
fn all_possible_targets() -> Vec<CompletionTarget> {
    let mut targets = Vec::new();

    #[cfg(unix)]
    {
        if let Some(data) = dirs::data_dir() {
            targets.push(CompletionTarget {
                shell: Shell::Bash,
                canonical_name: "govmr.bash",
                link_path: data
                    .join("bash-completion")
                    .join("completions")
                    .join("govmr"),
            });
            targets.push(CompletionTarget {
                shell: Shell::Zsh,
                canonical_name: "_govmr",
                link_path: data.join("zsh").join("site-functions").join("_govmr"),
            });
        }
        if let Some(config) = dirs::config_dir() {
            targets.push(CompletionTarget {
                shell: Shell::Fish,
                canonical_name: "govmr.fish",
                link_path: config.join("fish").join("completions").join("govmr.fish"),
            });
            targets.push(CompletionTarget {
                shell: Shell::PowerShell,
                canonical_name: "govmr.ps1",
                link_path: config.join("powershell").join("govmr.ps1"),
            });
        }
    }

    #[cfg(windows)]
    {
        if let Some(docs) = dirs::document_dir() {
            // PowerShell 5.1 (Windows PowerShell)
            targets.push(CompletionTarget {
                shell: Shell::PowerShell,
                canonical_name: "govmr.ps1",
                link_path: docs.join("WindowsPowerShell").join("govmr.ps1"),
            });
            // PowerShell 6+ (PowerShell Core)
            targets.push(CompletionTarget {
                shell: Shell::PowerShell,
                canonical_name: "govmr.ps1",
                link_path: docs.join("PowerShell").join("govmr.ps1"),
            });
        }
    }

    targets
}

/// Checks whether the shell-facing link exists and points to the canonical file.
///
/// On Unix this verifies the entry is a symlink *and* resolves to the expected
/// target. A stale symlink from a previous install layout (or a regular file
/// dropped by an older version) fails this check and gets replaced.
#[cfg(unix)]
fn is_link_valid(link: &Path, target: &Path) -> bool {
    match fs::symlink_metadata(link) {
        Ok(m) if m.file_type().is_symlink() => {
            fs::read_link(link).is_ok_and(|resolved| resolved == target)
        }
        _ => false,
    }
}

/// On Windows the "link" is a plain copy, so "valid" just means "exists".
#[cfg(windows)]
fn is_link_valid(link: &Path, _target: &Path) -> bool {
    link.exists()
}

/// Creates (or replaces) the shell-facing link pointing at the canonical file.
///
/// On Unix this is a symlink; on Windows a file copy, because creating symlinks
/// on Windows requires elevated privileges or Developer Mode.
#[cfg(unix)]
fn create_link(target: &Path, link: &Path) -> std::io::Result<()> {
    // Remove whatever currently occupies the link path
    // (regular file, symlink, or broken symlink from a previous layout).
    if link.symlink_metadata().is_ok() {
        fs::remove_file(link)?;
    }
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)?;
    }
    std::os::unix::fs::symlink(target, link)
}

/// Windows variant: copies the canonical script into the shell's directory.
#[cfg(windows)]
fn create_link(target: &Path, link: &Path) -> std::io::Result<()> {
    if link.exists() {
        fs::remove_file(link)?;
    }
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(target, link).map(|_| ())
}
