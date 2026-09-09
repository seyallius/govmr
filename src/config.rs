//! Module config - Persisted user preferences stored as TOML under `~/.govmr`.
//!
//! Loading and saving are both logged: "my theme won't save" is otherwise
//! indistinguishable from "the theme was saved and something else reset it",
//! so the file path, the resolved value, and where it came from all go to the log.

use crate::{logging, theme::ThemeName};
use serde::{Deserialize, Serialize};
use std::env;

/// Allow overriding the "current" version for testing purposes.
const GOVMR_TEST_VERSION: &str = "GOVMR_TEST_VERSION";

// ------------------------------------------ Types & Impls ------------------------------------- //

/// On-disk layout of `~/.govmr/config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigFile {
    /// Selected color-theme key (e.g. `"midnight"`).
    #[serde(default)]
    theme: String,
}
impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            theme: ThemeName::GoCyan.key().to_string(),
        }
    }
}

/// Reads and writes persisted preferences from / to `~/.govmr/config.toml`.
///
/// Unknown fields and a missing/absent file are tolerated (defaults are used),
/// and a legacy plain-text `~/.govmr/config` is migrated on first load.
pub struct Config {
    /// The theme selected by the user.
    pub theme: ThemeName,
    /// Path to the backing TOML file.
    path: std::path::PathBuf,
}
impl Config {
    /// Loads configuration from `<base_dir>/config.toml`, falling back to (and
    /// migrating) the legacy `<base_dir>/config` key/value file if present.
    ///
    /// Which of the three sources answered (`config.toml`, the legacy file, or
    /// built-in defaults) is recorded, since "no config file" and "unparseable
    /// config file" look identical to the user but need different fixes.
    #[must_use]
    pub fn load(base_dir: &std::path::Path) -> Self {
        let path = base_dir.join("config.toml");
        let legacy = base_dir.join("config");
        let mut source = "defaults";

        // 1) The file we write ourselves.
        let mut cfg = match std::fs::read_to_string(&path) {
            Ok(raw) => match toml::from_str::<ConfigFile>(&raw) {
                Ok(cfg) => {
                    source = "config.toml";
                    Some(cfg)
                }
                Err(e) => {
                    logging::warn(&format!(
                        "config: parse failed path=\"{}\" error={e} action=try_legacy",
                        path.display()
                    ));
                    None
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                logging::warn(&format!(
                    "config: unreadable path=\"{}\" error={e} action=try_legacy",
                    path.display()
                ));
                None
            }
        };

        // 2) One-time migration from the old plain-text `theme = x` file.
        if cfg.is_none() {
            let migrated = std::fs::read_to_string(&legacy)
                .ok()
                .and_then(|raw| parse_legacy_theme(&raw));
            if let Some(theme) = migrated {
                // The legacy file is only *read*; config.toml appears on the
                // next save, which the log states so the gap is not mistaken
                // for a failed write.
                logging::info(&format!(
                    "config: migrated legacy=\"{}\" to=\"{}\" theme={theme} note=pending_first_save",
                    legacy.display(),
                    path.display()
                ));
                cfg = Some(ConfigFile { theme });
                source = "legacy config";
            }
        }

        // 3) Defaults.
        let cfg = cfg.unwrap_or_default();
        let theme = ThemeName::from_key(&cfg.theme).unwrap_or_else(|| {
            logging::warn(&format!(
                "config: unknown theme value=\"{}\" path=\"{}\" fallback={}",
                cfg.theme,
                path.display(),
                ThemeName::default().key()
            ));
            ThemeName::default()
        });

        logging::info(&format!(
            "config: loaded path=\"{}\" theme={} source={source}",
            path.display(),
            theme.key()
        ));
        Self { theme, path }
    }

    /// Persists a new theme choice to `config.toml`.
    ///
    /// # Errors
    /// Returns an IO error if the config cannot be serialized or the file
    /// cannot be written.
    pub fn set_theme(&mut self, theme: ThemeName) -> std::io::Result<()> {
        self.theme = theme;
        let cfg = ConfigFile {
            theme: theme.key().to_string(),
        };
        // The config is a plain string key, so serialization cannot fail in
        // practice; the mapping keeps the signature panic-free either way.
        let body = format!(
            "# GoVMR user preferences\n# Re-generate with `govmr theme <name>` or press T in the TUI.\n\n{}\n",
            toml::to_string(&cfg).map_err(std::io::Error::other)?
        );
        std::fs::write(&self.path, &body).map_err(|e| {
            logging::error(&format!(
                "config: write failed path=\"{}\" theme={} error={e}",
                self.path.display(),
                theme.key()
            ));
            e
        })?;
        logging::debug(&format!(
            "config: written path=\"{}\" theme={}",
            self.path.display(),
            theme.key()
        ));
        Ok(())
    }
}

// ----------------------------------------- Public API ----------------------------------------- //

/// Returns the current cargo package version. If [`GOVMR_TEST_VERSION`]
/// is configured in the path, it'll return that version.
pub fn current_govmr_version() -> String {
    env::var(GOVMR_TEST_VERSION).unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string())
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Extracts `theme = <key>` from the legacy plain-text config format.
fn parse_legacy_theme(contents: &str) -> Option<String> {
    for line in contents.lines() {
        let mut parts = line.splitn(2, '=');
        let key = parts.next()?.trim();
        let value = parts.next()?.trim();
        if key == "theme" {
            return Some(value.to_string());
        }
    }
    None
}
