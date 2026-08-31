//! Copyright (c) 2026 SeyedAli
//! Licensed under the MIT License. See LICENSE file in the project root for details.
//
//! Module config - Persisted user preferences (currently: the active color theme).

use crate::theme::ThemeName;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Line key under which the chosen theme is stored in `~/.govmr/config`.
const THEME_KEY: &str = "theme";

/// Reads the persisted preferences from disk.
///
/// Unknown or missing values fall back to defaults, so this never fails hard —
/// a corrupt/absent config simply yields the default theme.
pub struct Config {
    /// The theme selected by the user.
    pub theme: ThemeName,
    /// Path to the backing config file.
    path: PathBuf,
}
impl Config {
    /// Loads configuration from `<base_dir>/config`, creating defaults if absent.
    pub fn load(base_dir: &Path) -> Self {
        let path = base_dir.join("config");
        let theme = std::fs::read_to_string(&path)
            .ok()
            .and_then(|contents| Self::parse_theme(&contents))
            .unwrap_or_default();
        Self { theme, path }
    }

    /// Persists a new theme choice to disk, preserving any other keys.
    ///
    /// # Errors
    /// Returns an IO error if the config file cannot be written.
    pub fn set_theme(&mut self, theme: ThemeName) -> std::io::Result<()> {
        self.theme = theme;

        let mut lines: Vec<String> = Vec::new();
        let mut found = false;
        if let Ok(existing) = std::fs::read_to_string(&self.path) {
            for line in existing.lines() {
                if let Some(key) = line.split('=').next() {
                    if key.trim() == THEME_KEY {
                        lines.push(format!("{} = {}", THEME_KEY, theme.key()));
                        found = true;
                        continue;
                    }
                }
                lines.push(line.to_string());
            }
        }
        if !found {
            lines.push(format!("{} = {}", THEME_KEY, theme.key()));
        }

        let mut file = std::fs::File::create(&self.path)?;
        file.write_all(lines.join("\n").as_bytes())?;
        file.write_all(b"\n")?;
        Ok(())
    }

    /// Extracts the theme value from the raw config text.
    fn parse_theme(contents: &str) -> Option<ThemeName> {
        for line in contents.lines() {
            let mut parts = line.splitn(2, '=');
            let key = parts.next()?.trim();
            let value = parts.next()?.trim();
            if key == THEME_KEY {
                return ThemeName::from_key(value);
            }
        }
        None
    }
}
