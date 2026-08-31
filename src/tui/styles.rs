use ratatui::style::{Color, Modifier, Style};

pub struct Theme;
impl Theme {
    pub fn highlight() -> Style {
        Style::default().fg(Color::Rgb(60, 113, 168))
    }

    pub fn success() -> Style {
        Style::default().fg(Color::Rgb(140, 219, 47))
    }

    pub fn error() -> Style {
        Style::default().fg(Color::Rgb(242, 93, 148))
    }

    pub fn warning() -> Style {
        Style::default().fg(Color::Rgb(255, 204, 0))
    }

    pub fn title() -> Style {
        Style::default()
            .fg(Color::Rgb(60, 113, 168))
            .add_modifier(Modifier::BOLD)
    }

    pub fn border() -> Style {
        Style::default().fg(Color::Rgb(60, 113, 168))
    }

    pub fn muted() -> Style {
        Style::default().fg(Color::Rgb(98, 98, 98))
    }
}
