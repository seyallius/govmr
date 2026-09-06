//! Module theme - Selectable, persistable color schemes for the TUI.
//!
//! Themes are organized into two brightness families ([`ThemeFamily`]) so the
//! picker can group them as the catalogue grows: dark schemes first, light
//! schemes after. `ThemeName::ALL` preserves that dark-then-light ordering.

use ratatui::style::{Color, Modifier, Style};
use std::fmt;

// ------------------------------------------ Types & Impls ------------------------------------- //

/// The selectable color schemes shipped with `GoVMR`.
///
/// Ordered dark-first, light-after so the picker can group them into two
/// contiguous [`ThemeFamily`] sections.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ThemeName {
    /// The default Go-brand cyan look.
    #[default]
    GoCyan,
    /// JetBrains "New Island" — deep blue-slate with a bright azure accent.
    JetBrainsNewIsland,
    /// Cursor editor dark — near-black with a soft indigo glow.
    CursorDark,
    /// Deep indigo on near-black — easy on the eyes for late nights.
    Midnight,
    /// Neon Tokyo skyline — cool blue on ink.
    TokyoNight,
    /// Catppuccin Mocha — cozy pastel espresso.
    CatppuccinMocha,
    /// Snowstorm / Nord blue palette.
    Nord,
    /// Dark Dracula purple palette.
    Dracula,
    /// Gruvbox Dark — warm retro earth tones.
    GruvboxDark,
    /// Rosé Pine — muted dusky rose.
    RosePine,
    /// Retro phosphor-green terminal.
    Matrix,
    /// Warm solarized amber.
    Amber,
    /// Minimal greyscale.
    Mono,
    /// Cursor editor light — clean paper with an indigo accent.
    CursorLight,
    /// Catppuccin Latte — soft pastel milk.
    CatppuccinLatte,
    /// GitHub Light — crisp paper white.
    GitHubLight,
    /// Solarized Light — calm parchment.
    SolarizedLight,
    /// Rosé Pine Dawn — gentle morning rose.
    RosePineDawn,
    /// Bright high-contrast light scheme.
    Light,
}
impl ThemeName {
    /// Every available theme, in display order (dark schemes first).
    pub const ALL: [ThemeName; 19] = [
        ThemeName::GoCyan,
        ThemeName::JetBrainsNewIsland,
        ThemeName::CursorDark,
        ThemeName::Midnight,
        ThemeName::TokyoNight,
        ThemeName::CatppuccinMocha,
        ThemeName::Nord,
        ThemeName::Dracula,
        ThemeName::GruvboxDark,
        ThemeName::RosePine,
        ThemeName::Matrix,
        ThemeName::Amber,
        ThemeName::Mono,
        ThemeName::CursorLight,
        ThemeName::CatppuccinLatte,
        ThemeName::GitHubLight,
        ThemeName::SolarizedLight,
        ThemeName::RosePineDawn,
        ThemeName::Light,
    ];

    /// Short identifier used in the config file and the CLI.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            ThemeName::GoCyan => "gocyan",
            ThemeName::JetBrainsNewIsland => "newisland",
            ThemeName::CursorDark => "cursordark",
            ThemeName::Midnight => "midnight",
            ThemeName::TokyoNight => "tokyonight",
            ThemeName::CatppuccinMocha => "mocha",
            ThemeName::Nord => "nord",
            ThemeName::Dracula => "dracula",
            ThemeName::GruvboxDark => "gruvboxdark",
            ThemeName::RosePine => "rosepine",
            ThemeName::Matrix => "matrix",
            ThemeName::Amber => "amber",
            ThemeName::Mono => "mono",
            ThemeName::CursorLight => "cursorlight",
            ThemeName::CatppuccinLatte => "latte",
            ThemeName::GitHubLight => "githublight",
            ThemeName::SolarizedLight => "solarizedlight",
            ThemeName::RosePineDawn => "rosedawn",
            ThemeName::Light => "light",
        }
    }

    /// Human-friendly display name.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            ThemeName::GoCyan => "Go Cyan",
            ThemeName::JetBrainsNewIsland => "JetBrains New Island",
            ThemeName::CursorDark => "Cursor Dark",
            ThemeName::Midnight => "Midnight",
            ThemeName::TokyoNight => "Tokyo Night",
            ThemeName::CatppuccinMocha => "Catppuccin Mocha",
            ThemeName::Nord => "Nord",
            ThemeName::Dracula => "Dracula",
            ThemeName::GruvboxDark => "Gruvbox Dark",
            ThemeName::RosePine => "Rosé Pine",
            ThemeName::Matrix => "Matrix Green",
            ThemeName::Amber => "Amber Glow",
            ThemeName::Mono => "Monochrome",
            ThemeName::CursorLight => "Cursor Light",
            ThemeName::CatppuccinLatte => "Catppuccin Latte",
            ThemeName::GitHubLight => "GitHub Light",
            ThemeName::SolarizedLight => "Solarized Light",
            ThemeName::RosePineDawn => "Rosé Pine Dawn",
            ThemeName::Light => "Light",
        }
    }

    /// Parses a config/CLI key back into a [`ThemeName`] (case-insensitive).
    #[must_use]
    pub fn from_key(raw: &str) -> Option<ThemeName> {
        let key = raw.trim().to_lowercase();
        Self::ALL
            .iter()
            .copied()
            .find(|t| t.key() == key || t.title().to_lowercase() == key)
    }

    /// Whether this theme paints a bright background.
    ///
    /// This is the authoritative grouping source; [`Theme::is_light`] performs
    /// the same check against the concrete palette as a sanity net.
    #[must_use]
    pub fn is_light(self) -> bool {
        matches!(
            self,
            ThemeName::CursorLight
                | ThemeName::CatppuccinLatte
                | ThemeName::GitHubLight
                | ThemeName::SolarizedLight
                | ThemeName::RosePineDawn
                | ThemeName::Light
        )
    }

    /// Whether this theme paints a dim background.
    #[must_use]
    pub fn is_dark(self) -> bool {
        !self.is_light()
    }

    /// The brightness family this theme belongs to.
    #[must_use]
    pub fn family(self) -> ThemeFamily {
        if self.is_light() {
            ThemeFamily::Light
        } else {
            ThemeFamily::Dark
        }
    }

    /// All themes belonging to `family`, preserving [`ThemeName::ALL`] order.
    #[must_use]
    pub fn in_family(family: ThemeFamily) -> Vec<ThemeName> {
        Self::ALL
            .iter()
            .copied()
            .filter(|t| t.family() == family)
            .collect()
    }

    /// Index of this theme within its own family's ordered list.
    #[must_use]
    pub fn index_in_family(self) -> usize {
        Self::in_family(self.family())
            .iter()
            .position(|t| *t == self)
            .unwrap_or(0)
    }
}
impl fmt::Display for ThemeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.title())
    }
}

/// Coarse brightness family a theme belongs to, used to group the picker.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeFamily {
    /// Dim-background schemes.
    Dark,
    /// Bright-background schemes.
    Light,
}
impl ThemeFamily {
    /// Every family in display order (dark first).
    pub const ALL: [ThemeFamily; 2] = [ThemeFamily::Dark, ThemeFamily::Light];

    /// Human-friendly section label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            ThemeFamily::Dark => "Dark",
            ThemeFamily::Light => "Light",
        }
    }

    /// Small glyph shown next to the section header.
    #[must_use]
    pub fn icon(self) -> &'static str {
        match self {
            ThemeFamily::Dark => "🌙",
            ThemeFamily::Light => "☀️",
        }
    }

    /// Position of this family within [`ThemeFamily::ALL`] (0 = Dark, 1 = Light).
    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|f| *f == self).unwrap_or(0)
    }

    /// The family at position `i`, clamped to a valid index.
    #[must_use]
    pub fn at(i: usize) -> ThemeFamily {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }
}

/// Where the theme-picker navigation currently is.
///
/// The picker is a tiny two-level file browser: choose a [`ThemeFamily`]
/// folder, then choose a theme inside it. Modeling the level as an enum
/// keeps the key handler free of boolean flags and makes illegal states
/// (e.g. "a theme is highlighted but no folder is open") unrepresentable.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ThemePickerView {
    /// Choosing between the Dark / Light folders.
    #[default]
    Categories,
    /// Browsing the themes inside one family.
    Family(ThemeFamily),
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
    /// Quiet chrome color (unfocused panel borders, dividers, tab underline).
    /// Sits between `bg` and `gray` so inactive chrome recedes without vanishing.
    pub dim: Color,
}
impl Theme {
    /// Builds the palette for the named scheme.
    #[must_use]
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
                dim: Color::Rgb(47, 55, 71),
            },
            ThemeName::JetBrainsNewIsland => Theme {
                brand: Color::Rgb(93, 176, 255),
                brand_dark: Color::Rgb(28, 52, 84),
                success: Color::Rgb(127, 191, 123),
                error: Color::Rgb(255, 107, 107),
                warning: Color::Rgb(255, 196, 92),
                accent: Color::Rgb(187, 154, 247),
                grey: Color::Rgb(124, 134, 152),
                fg: Color::Rgb(222, 228, 238),
                bg: Color::Rgb(30, 35, 48),
                dim: Color::Rgb(55, 63, 80),
            },
            ThemeName::CursorDark => Theme {
                brand: Color::Rgb(124, 137, 255),
                brand_dark: Color::Rgb(44, 42, 84),
                success: Color::Rgb(94, 218, 152),
                error: Color::Rgb(255, 105, 97),
                warning: Color::Rgb(255, 193, 94),
                accent: Color::Rgb(232, 121, 249),
                grey: Color::Rgb(112, 117, 138),
                fg: Color::Rgb(226, 228, 240),
                bg: Color::Rgb(17, 18, 24),
                dim: Color::Rgb(46, 49, 64),
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
                dim: Color::Rgb(43, 49, 73),
            },
            ThemeName::TokyoNight => Theme {
                brand: Color::Rgb(122, 162, 247),
                brand_dark: Color::Rgb(36, 40, 59),
                success: Color::Rgb(158, 206, 106),
                error: Color::Rgb(247, 118, 142),
                warning: Color::Rgb(224, 175, 104),
                accent: Color::Rgb(187, 154, 247),
                grey: Color::Rgb(86, 95, 137),
                fg: Color::Rgb(192, 202, 245),
                bg: Color::Rgb(26, 27, 38),
                dim: Color::Rgb(41, 46, 66),
            },
            ThemeName::CatppuccinMocha => Theme {
                brand: Color::Rgb(203, 166, 247),
                brand_dark: Color::Rgb(49, 50, 68),
                success: Color::Rgb(166, 227, 161),
                error: Color::Rgb(243, 139, 168),
                warning: Color::Rgb(249, 226, 175),
                accent: Color::Rgb(245, 194, 231),
                grey: Color::Rgb(108, 112, 134),
                fg: Color::Rgb(205, 214, 244),
                bg: Color::Rgb(30, 30, 46),
                dim: Color::Rgb(69, 71, 90),
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
                dim: Color::Rgb(71, 79, 95),
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
                dim: Color::Rgb(59, 63, 85),
            },
            ThemeName::GruvboxDark => Theme {
                brand: Color::Rgb(254, 128, 25),
                brand_dark: Color::Rgb(60, 56, 54),
                success: Color::Rgb(184, 187, 38),
                error: Color::Rgb(251, 73, 52),
                warning: Color::Rgb(250, 189, 47),
                accent: Color::Rgb(211, 134, 155),
                grey: Color::Rgb(146, 131, 116),
                fg: Color::Rgb(235, 219, 178),
                bg: Color::Rgb(40, 40, 40),
                dim: Color::Rgb(80, 73, 69),
            },
            ThemeName::RosePine => Theme {
                brand: Color::Rgb(196, 167, 231),
                brand_dark: Color::Rgb(38, 35, 58),
                success: Color::Rgb(156, 207, 216),
                error: Color::Rgb(235, 111, 146),
                warning: Color::Rgb(246, 193, 119),
                accent: Color::Rgb(235, 188, 186),
                grey: Color::Rgb(110, 106, 134),
                fg: Color::Rgb(224, 222, 244),
                bg: Color::Rgb(25, 23, 36),
                dim: Color::Rgb(64, 61, 82),
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
                dim: Color::Rgb(35, 56, 44),
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
                dim: Color::Rgb(66, 55, 40),
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
                dim: Color::Rgb(58, 58, 58),
            },
            ThemeName::CursorLight => Theme {
                brand: Color::Rgb(99, 102, 241),
                brand_dark: Color::Rgb(224, 226, 255),
                success: Color::Rgb(22, 163, 74),
                error: Color::Rgb(220, 38, 38),
                warning: Color::Rgb(217, 119, 6),
                accent: Color::Rgb(168, 85, 247),
                grey: Color::Rgb(115, 115, 130),
                fg: Color::Rgb(26, 26, 46),
                bg: Color::Rgb(250, 250, 250),
                dim: Color::Rgb(213, 213, 226),
            },
            ThemeName::CatppuccinLatte => Theme {
                brand: Color::Rgb(136, 57, 239),
                brand_dark: Color::Rgb(204, 208, 218),
                success: Color::Rgb(64, 160, 43),
                error: Color::Rgb(210, 15, 57),
                warning: Color::Rgb(223, 142, 29),
                accent: Color::Rgb(234, 118, 203),
                grey: Color::Rgb(108, 111, 133),
                fg: Color::Rgb(76, 79, 105),
                bg: Color::Rgb(239, 241, 245),
                dim: Color::Rgb(188, 192, 204),
            },
            ThemeName::GitHubLight => Theme {
                brand: Color::Rgb(9, 105, 218),
                brand_dark: Color::Rgb(221, 244, 255),
                success: Color::Rgb(26, 127, 55),
                error: Color::Rgb(207, 34, 46),
                warning: Color::Rgb(154, 103, 0),
                accent: Color::Rgb(130, 80, 223),
                grey: Color::Rgb(89, 99, 110),
                fg: Color::Rgb(31, 35, 40),
                bg: Color::Rgb(255, 255, 255),
                dim: Color::Rgb(209, 217, 224),
            },
            ThemeName::SolarizedLight => Theme {
                brand: Color::Rgb(38, 139, 210),
                brand_dark: Color::Rgb(238, 232, 213),
                success: Color::Rgb(133, 153, 0),
                error: Color::Rgb(220, 50, 47),
                warning: Color::Rgb(181, 137, 0),
                accent: Color::Rgb(108, 113, 196),
                grey: Color::Rgb(147, 161, 161),
                fg: Color::Rgb(101, 123, 131),
                bg: Color::Rgb(253, 246, 227),
                dim: Color::Rgb(222, 216, 199),
            },
            ThemeName::RosePineDawn => Theme {
                brand: Color::Rgb(144, 122, 169),
                brand_dark: Color::Rgb(242, 233, 225),
                success: Color::Rgb(86, 148, 159),
                error: Color::Rgb(180, 99, 122),
                warning: Color::Rgb(234, 157, 52),
                accent: Color::Rgb(215, 130, 126),
                grey: Color::Rgb(152, 147, 165),
                fg: Color::Rgb(87, 82, 121),
                bg: Color::Rgb(250, 244, 237),
                dim: Color::Rgb(223, 218, 217),
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
                dim: Color::Rgb(205, 211, 218),
            },
        }
    }

    /// Whether this is a light (bright-background) scheme.
    #[must_use]
    pub fn is_light(&self) -> bool {
        matches!(self.bg, Color::Rgb(r, _, _) if r > 200)
    }

    // ------------------------------------------- Styles ------------------------------------------- //

    /// Style for focused highlights and secondary brand indicators.
    #[must_use]
    pub fn highlight(&self) -> Style {
        Style::default().fg(self.brand)
    }

    /// Style for success banners, active markers, and positive feedback.
    #[must_use]
    pub fn success(&self) -> Style {
        Style::default().fg(self.success)
    }

    /// Style for error messages, failure alerts, and destructive actions.
    #[must_use]
    pub fn error(&self) -> Style {
        Style::default().fg(self.error)
    }

    /// Style for warning banners and non-fatal notifications.
    #[must_use]
    pub fn warning(&self) -> Style {
        Style::default().fg(self.warning)
    }

    /// Style for primary headers and dialog titles.
    #[must_use]
    pub fn title(&self) -> Style {
        Style::default().fg(self.brand).add_modifier(Modifier::BOLD)
    }

    /// Style for primary container borders.
    #[must_use]
    pub fn border(&self) -> Style {
        Style::default().fg(self.brand)
    }

    /// Style for subtle hints, borders, and footer shortcuts.
    #[must_use]
    pub fn muted(&self) -> Style {
        Style::default().fg(self.grey)
    }

    /// Style for brand-colored bold text.
    #[must_use]
    pub fn brand_bold(&self) -> Style {
        Style::default().fg(self.brand).add_modifier(Modifier::BOLD)
    }

    /// Style for the currently selected list row (inverted brand block).
    #[must_use]
    pub fn selected_row(&self) -> Style {
        Style::default()
            .fg(self.fg)
            .bg(self.brand_dark)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for pre-release / unstable version badges.
    #[must_use]
    pub fn badge_unstable(&self) -> Style {
        Style::default()
            .fg(self.warning)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for the "(installed)" badge.
    #[must_use]
    pub fn badge_installed(&self) -> Style {
        Style::default().fg(self.accent)
    }

    /// Style for the "(active)" badge.
    #[must_use]
    pub fn badge_active(&self) -> Style {
        Style::default()
            .fg(self.success)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for an inactive tab title.
    #[must_use]
    pub fn tab_inactive(&self) -> Style {
        Style::default().fg(self.grey)
    }

    /// Style for an active tab title.
    #[must_use]
    pub fn tab_active(&self) -> Style {
        Style::default().fg(self.brand).add_modifier(Modifier::BOLD)
    }

    /// Style for key hints in the help footer.
    #[must_use]
    pub fn key_hint(&self) -> Style {
        Style::default().fg(self.brand).add_modifier(Modifier::BOLD)
    }

    /// Style for dim descriptive text inside modals.
    #[must_use]
    pub fn modal_body(&self) -> Style {
        Style::default().fg(self.fg)
    }

    /// Style for quiet chrome: unfocused borders, dividers, the tab underline.
    #[must_use]
    pub fn dim_border(&self) -> Style {
        Style::default().fg(self.dim)
    }
}
