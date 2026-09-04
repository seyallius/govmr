//! Module theme - Selectable, persistable color schemes for the TUI.

use ratatui::style::{Color, Modifier, Style};
use std::fmt;

// ------------------------------------------ Types & Impls ------------------------------------- //

/// The selectable color schemes shipped with GoVMR.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeName {
    /// The default Go-brand cyan look.
    GoCyan,
    /// Deep indigo on near-black — easy on the eyes for late nights.
    Midnight,
    /// Retro phosphor-green terminal.
    Matrix,
    /// Warm solarized amber.
    Amber,
    /// Snowstorm / Nord blue palette.
    Nord,
    /// Dark Dracula purple palette.
    Dracula,
    /// Bright high-contrast light scheme.
    Light,
    /// Minimal greyscale.
    Mono,
}

impl ThemeName {
    /// Every available theme, in display order.
    pub const ALL: [ThemeName; 8] = [
        ThemeName::GoCyan,
        ThemeName::Midnight,
        ThemeName::Matrix,
        ThemeName::Amber,
        ThemeName::Nord,
        ThemeName::Dracula,
        ThemeName::Light,
        ThemeName::Mono,
    ];

    /// Short identifier used in the config file and the CLI.
    pub fn key(self) -> &'static str {
        match self {
            ThemeName::GoCyan => "gocyan",
            ThemeName::Midnight => "midnight",
            ThemeName::Matrix => "matrix",
            ThemeName::Amber => "amber",
            ThemeName::Nord => "nord",
            ThemeName::Dracula => "dracula",
            ThemeName::Light => "light",
            ThemeName::Mono => "mono",
        }
    }

    /// Human-friendly display name.
    pub fn title(self) -> &'static str {
        match self {
            ThemeName::GoCyan => "Go Cyan",
            ThemeName::Midnight => "Midnight",
            ThemeName::Matrix => "Matrix Green",
            ThemeName::Amber => "Amber Glow",
            ThemeName::Nord => "Nord",
            ThemeName::Dracula => "Dracula",
            ThemeName::Light => "Light",
            ThemeName::Mono => "Monochrome",
        }
    }

    /// Parses a config/CLI key back into a [`ThemeName`] (case-insensitive).
    pub fn from_key(raw: &str) -> Option<ThemeName> {
        let key = raw.trim().to_lowercase();
        Self::ALL
            .iter()
            .copied()
            .find(|t| t.key() == key || t.title().to_lowercase() == key)
    }
}

impl fmt::Display for ThemeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.title())
    }
}

impl Default for ThemeName {
    fn default() -> Self {
        ThemeName::GoCyan
    }
}

/// A concrete palette plus derived widget styles.
#[derive(Clone, Copy)]
pub struct Theme {
    /// Primary brand / accent color (borders, highlights, key hints).
    pub brand: Color,
    /// Darker shade of the brand, used for selection/gauge backgrounds.
    pub brand_dark: Color,
    /// Success / active color.
    pub success: Color,
    /// Error / destructive color.
    pub error: Color,
    /// Warning / caution color.
    pub warning: Color,
    /// Secondary accent for badges.
    pub accent: Color,
    /// Dimmed text and inactive elements.
    pub grey: Color,
    /// Primary foreground text color.
    pub fg: Color,
    /// Screen background fill.
    pub bg: Color,
}

impl Theme {
    /// Builds the palette for the named scheme.
    pub fn for_name(name: ThemeName) -> Theme {
        match name {
            ThemeName::GoCyan => Theme {
                brand: Color::Rgb(0, 173, 216),
                brand_dark: Color::Rgb(6, 84, 106),
                success: Color::Rgb(63, 208, 127),
                error: Color::Rgb(255, 89, 110),
                warning: Color::Rgb(255, 184, 77),
                accent: Color::Rgb(167, 139, 250),
                grey: Color::Rgb(105, 115, 134),
                fg: Color::Rgb(224, 232, 240),
                bg: Color::Rgb(18, 22, 30),
            },
            ThemeName::Midnight => Theme {
                brand: Color::Rgb(129, 140, 248),
                brand_dark: Color::Rgb(49, 46, 129),
                success: Color::Rgb(52, 211, 153),
                error: Color::Rgb(248, 113, 113),
                warning: Color::Rgb(250, 204, 21),
                accent: Color::Rgb(192, 132, 252),
                grey: Color::Rgb(92, 102, 128),
                fg: Color::Rgb(203, 213, 225),
                bg: Color::Rgb(13, 16, 28),
            },
            ThemeName::Matrix => Theme {
                brand: Color::Rgb(0, 220, 120),
                brand_dark: Color::Rgb(0, 77, 42),
                success: Color::Rgb(0, 230, 118),
                error: Color::Rgb(255, 90, 90),
                warning: Color::Rgb(220, 255, 0),
                accent: Color::Rgb(0, 200, 190),
                grey: Color::Rgb(88, 120, 96),
                fg: Color::Rgb(190, 255, 205),
                bg: Color::Rgb(4, 12, 8),
            },
            ThemeName::Amber => Theme {
                brand: Color::Rgb(255, 183, 77),
                brand_dark: Color::Rgb(110, 66, 18),
                success: Color::Rgb(153, 220, 110),
                error: Color::Rgb(235, 100, 80),
                warning: Color::Rgb(255, 214, 90),
                accent: Color::Rgb(214, 160, 255),
                grey: Color::Rgb(140, 122, 96),
                fg: Color::Rgb(236, 226, 206),
                bg: Color::Rgb(24, 19, 12),
            },
            ThemeName::Nord => Theme {
                brand: Color::Rgb(136, 192, 208),
                brand_dark: Color::Rgb(59, 66, 82),
                success: Color::Rgb(163, 190, 140),
                error: Color::Rgb(191, 97, 106),
                warning: Color::Rgb(235, 203, 139),
                accent: Color::Rgb(180, 142, 173),
                grey: Color::Rgb(118, 128, 146),
                fg: Color::Rgb(216, 222, 233),
                bg: Color::Rgb(46, 52, 64),
            },
            ThemeName::Dracula => Theme {
                brand: Color::Rgb(189, 147, 249),
                brand_dark: Color::Rgb(68, 42, 110),
                success: Color::Rgb(80, 250, 123),
                error: Color::Rgb(255, 85, 85),
                warning: Color::Rgb(241, 250, 140),
                accent: Color::Rgb(255, 121, 198),
                grey: Color::Rgb(98, 114, 164),
                fg: Color::Rgb(248, 248, 242),
                bg: Color::Rgb(40, 42, 54),
            },
            ThemeName::Light => Theme {
                brand: Color::Rgb(0, 121, 107),
                brand_dark: Color::Rgb(178, 223, 219),
                success: Color::Rgb(21, 116, 63),
                error: Color::Rgb(197, 16, 32),
                warning: Color::Rgb(176, 116, 0),
                accent: Color::Rgb(119, 62, 160),
                grey: Color::Rgb(120, 128, 138),
                fg: Color::Rgb(24, 28, 34),
                bg: Color::Rgb(250, 250, 248),
            },
            ThemeName::Mono => Theme {
                brand: Color::Rgb(228, 228, 228),
                brand_dark: Color::Rgb(70, 70, 70),
                success: Color::Rgb(200, 200, 200),
                error: Color::Rgb(245, 245, 245),
                warning: Color::Rgb(165, 165, 165),
                accent: Color::Rgb(150, 150, 150),
                grey: Color::Rgb(112, 112, 112),
                fg: Color::Rgb(226, 226, 226),
                bg: Color::Rgb(20, 20, 20),
            },
        }
    }

    /// Whether this is a light (bright-background) scheme.
    pub fn is_light(&self) -> bool {
        matches!(self.bg, Color::Rgb(r, _, _) if r > 200)
    }

    // ------------------------------------------- Styles ------------------------------------------- //

    /// Style for focused highlights and secondary brand indicators.
    pub fn highlight(&self) -> Style {
        Style::default().fg(self.brand)
    }

    /// Style for success banners, active markers, and positive feedback.
    pub fn success(&self) -> Style {
        Style::default().fg(self.success)
    }

    /// Style for error messages, failure alerts, and destructive actions.
    pub fn error(&self) -> Style {
        Style::default().fg(self.error)
    }

    /// Style for warning banners and non-fatal notifications.
    pub fn warning(&self) -> Style {
        Style::default().fg(self.warning)
    }

    /// Style for primary headers and dialog titles.
    pub fn title(&self) -> Style {
        Style::default().fg(self.brand).add_modifier(Modifier::BOLD)
    }

    /// Style for primary container borders.
    pub fn border(&self) -> Style {
        Style::default().fg(self.brand)
    }

    /// Style for subtle hints, borders, and footer shortcuts.
    pub fn muted(&self) -> Style {
        Style::default().fg(self.grey)
    }

    /// Style for brand-colored bold text.
    pub fn brand_bold(&self) -> Style {
        Style::default().fg(self.brand).add_modifier(Modifier::BOLD)
    }

    /// Style for the currently selected list row (inverted brand block).
    pub fn selected_row(&self) -> Style {
        Style::default()
            .fg(self.fg)
            .bg(self.brand_dark)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for pre-release / unstable version badges.
    pub fn badge_unstable(&self) -> Style {
        Style::default().fg(self.warning).add_modifier(Modifier::BOLD)
    }

    /// Style for the "(installed)" badge.
    pub fn badge_installed(&self) -> Style {
        Style::default().fg(self.accent)
    }

    /// Style for the "(active)" badge.
    pub fn badge_active(&self) -> Style {
        Style::default().fg(self.success).add_modifier(Modifier::BOLD)
    }

    /// Style for an inactive tab title.
    pub fn tab_inactive(&self) -> Style {
        Style::default().fg(self.grey)
    }

    /// Style for an active tab title.
    pub fn tab_active(&self) -> Style {
        Style::default().fg(self.brand).add_modifier(Modifier::BOLD)
    }

    /// Style for key hints in the help footer.
    pub fn key_hint(&self) -> Style {
        Style::default().fg(self.brand).add_modifier(Modifier::BOLD)
    }

    /// Style for dim descriptive text inside modals.
    pub fn modal_body(&self) -> Style {
        Style::default().fg(self.fg)
    }
}
