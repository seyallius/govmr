//! Copyright (c) 2026 SeyedAli
//! Licensed under the MIT License. See LICENSE file in the project root for details.
//!
//! Module styles - Color schemes, styling definitions, and UI themes for Ratatui.

use ratatui::style::{Color, Modifier, Style};

// ------------------------------------------ Types & Impls ------------------------------------- //

/// Central color theme configuration providing consistent styles across TUI views.
pub struct Theme;
impl Theme {
    // ----------------------------------------- Public API ----------------------------------------- //

    /// Style for focused highlights and secondary brand indicators.
    pub fn highlight() -> Style {
        Style::default().fg(Color::Rgb(60, 113, 168))
    }

    /// Style for success banners, active markers, and positive feedback.
    pub fn success() -> Style {
        Style::default().fg(Color::Rgb(140, 219, 47))
    }

    /// Style for error messages, failure alerts, and destructive actions.
    pub fn error() -> Style {
        Style::default().fg(Color::Rgb(242, 93, 148))
    }

    /// Style for warning banners and non-fatal notifications.
    pub fn warning() -> Style {
        Style::default().fg(Color::Rgb(255, 204, 0))
    }

    /// Style for primary headers and dialog titles.
    pub fn title() -> Style {
        Style::default()
            .fg(Color::Rgb(60, 113, 168))
            .add_modifier(Modifier::BOLD)
    }

    /// Style for primary container borders.
    pub fn border() -> Style {
        Style::default().fg(Color::Rgb(60, 113, 168))
    }

    /// Style for subtle hints, borders, and footer shortcuts.
    pub fn muted() -> Style {
        Style::default().fg(Color::Rgb(98, 98, 98))
    }
}
