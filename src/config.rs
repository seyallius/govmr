//! Module config - Persisted user preferences stored as TOML under `~/.govmr`.

use crate::theme::ThemeName;
use serde::{Deserialize, Serialize};

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
    pub fn load(base_dir: &std::path::Path) -> Self {
        let path = base_dir.join("config.toml");
        let legacy = base_dir.join("config");

        let cfg = std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| toml::from_str::<ConfigFile>(&raw).ok())
            .or_else(|| {
                // One-time migration from the old plain-text `theme = x` file.
                std::fs::read_to_string(&legacy)
                    .ok()
                    .and_then(|raw| parse_legacy_theme(&raw))
                    .map(|theme| ConfigFile { theme })
            })
            .unwrap_or_default();

        let theme = ThemeName::from_key(&cfg.theme).unwrap_or_default();
        Self { theme, path }
    }

    /// Persists a new theme choice to `config.toml`.
    ///
    /// # Errors
    /// Returns an IO error if the file cannot be written.
    pub fn set_theme(&mut self, theme: ThemeName) -> std::io::Result<()> {
        self.theme = theme;
        let cfg = ConfigFile {
            theme: theme.key().to_string(),
        };
        let body = format!(
            "# GoVMR user preferences\n# Re-generate with `govmr theme <name>` or press T in the TUI.\n\n{}\n",
            toml::to_string(&cfg).expect("config serializes")
        );
        std::fs::write(&self.path, body)
    }
}

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
