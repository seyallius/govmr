//! Copyright (c) 2026 SeyedAli
//! Licensed under the MIT License. See LICENSE file in the project root for details.
//
//! Module styles - Color schemes, styling definitions, and UI themes for Ratatui.

use ratatui::style::{Color, Modifier, Style};

// ------------------------------------------ Types & Impls ------------------------------------- //

/// Central color theme configuration providing consistent styles across TUI views.
///
/// The palette is built around the official Go brand cyan, complemented by soft
/// accents and high-contrast feedback colors.
pub struct Theme;
impl Theme {
    // ------------------------------------------- Colors ------------------------------------------- //

    /// The Go brand cyan used for primary chrome, borders and accents.
    pub const BRAND: Color = Color::Rgb(0, 173, 216);
    /// Deep teal used for selection backgrounds.
    pub const BRAND_DARK: Color = Color::Rgb(6, 84, 106);
    /// Fresh green used for success states.
    pub const GREEN: Color = Color::Rgb(63, 208, 127);
    /// Coral red used for errors and destructive actions.
    pub const RED: Color = Color::Rgb(255, 89, 110);
    /// Warm amber used for warnings and cautions.
    pub const AMBER: Color = Color::Rgb(255, 184, 77);
    /// Violet used for secondary highlights and hints.
    pub const VIOLET: Color = Color::Rgb(167, 139, 250);
    /// Faint grey used for dimmed text and inactive elements.
    pub const GREY: Color = Color::Rgb(105, 115, 134);
    /// Near-white used for primary text.
    pub const FOREGROUND: Color = Color::Rgb(224, 232, 240);
    /// Dark slate used for subtle backgrounds.
    pub const BG: Color = Color::Rgb(16, 21, 30);

    // ------------------------------------------- Styles ------------------------------------------- //

    /// Style for focused highlights and secondary brand indicators.
    pub fn highlight() -> Style {
        Style::default().fg(Self::BRAND)
    }

    /// Style for success banners, active markers, and positive feedback.
    pub fn success() -> Style {
        Style::default().fg(Self::GREEN)
    }

    /// Style for error messages, failure alerts, and destructive actions.
    pub fn error() -> Style {
        Style::default().fg(Self::RED)
    }

    /// Style for warning banners and non-fatal notifications.
    pub fn warning() -> Style {
        Style::default().fg(Self::AMBER)
    }

    /// Style for primary headers and dialog titles.
    pub fn title() -> Style {
        Style::default()
            .fg(Self::BRAND)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for primary container borders.
    pub fn border() -> Style {
        Style::default().fg(Self::BRAND)
    }

    /// Style for subtle hints, borders, and footer shortcuts.
    pub fn muted() -> Style {
        Style::default().fg(Self::GREY)
    }

    // ----------------------------------------- Extra styles ---------------------------------------- //

    /// Style for brand-colored bold text.
    pub fn brand_bold() -> Style {
        Style::default()
            .fg(Self::BRAND)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for the currently selected list row (inverted cyan block).
    pub fn selected_row() -> Style {
        Style::default()
            .fg(Self::FOREGROUND)
            .bg(Self::BRAND_DARK)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for pre-release / unstable version badges.
    pub fn badge_unstable() -> Style {
        Style::default().fg(Self::AMBER).add_modifier(Modifier::BOLD)
    }

    /// Style for the "(installed)" badge.
    pub fn badge_installed() -> Style {
        Style::default().fg(Self::VIOLET)
    }

    /// Style for the "(active)" badge.
    pub fn badge_active() -> Style {
        Style::default().fg(Self::GREEN).add_modifier(Modifier::BOLD)
    }

    /// Style for an inactive tab title.
    pub fn tab_inactive() -> Style {
        Style::default().fg(Self::GREY)
    }

    /// Style for an active tab title.
    pub fn tab_active() -> Style {
        Style::default()
            .fg(Self::BRAND)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for key hints in the help footer.
    pub fn key_hint() -> Style {
        Style::default()
            .fg(Self::BRAND)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for dim descriptive text inside modals.
    pub fn modal_body() -> Style {
        Style::default().fg(Self::FOREGROUND)
    }
}
