use ratatui::style::Color;

use super::Theme;

/// Centralized color palette for the TUI. All view code should pull colors
/// from a `Palette` instance instead of hard-coding `Color::*` values.
pub struct Palette {
    /// Primary accent (logos, labels, author names, highlights).
    pub accent: Color,
    /// Primary text content.
    pub text: Color,
    /// Secondary / muted text (defaults, channel names, hints).
    pub text_muted: Color,
    /// Focused border color.
    pub border_focused: Color,
    /// Unfocused / dimmed border color.
    pub border_unfocused: Color,
    /// Border when an overlay is covering the list.
    pub border_overlay: Color,
    /// Error indicators (borders, messages).
    pub error: Color,
    /// Highlighted selection background.
    pub selection_bg: Color,
    /// Poll progress bar.
    pub poll_bar: Color,
    /// Splash "Version" label.
    pub splash_version_label: Color,
    /// Splash version number.
    pub splash_version_number: Color,
}

impl Palette {
    pub fn from_theme(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self::dark(),
            Theme::Light => Self::light(),
        }
    }

    fn dark() -> Self {
        Self {
            accent: Color::Rgb(255, 165, 0), // Orange
            text: Color::White,
            text_muted: Color::DarkGray,
            border_focused: Color::Cyan,
            border_unfocused: Color::DarkGray,
            border_overlay: Color::Blue,
            error: Color::Red,
            selection_bg: Color::DarkGray,
            poll_bar: Color::DarkGray,
            splash_version_label: Color::Rgb(0, 206, 209), // Turquoise
            splash_version_number: Color::Red,
        }
    }

    fn light() -> Self {
        Self {
            accent: Color::Black,
            text: Color::Black,
            text_muted: Color::Black,
            border_focused: Color::Black,
            border_unfocused: Color::Black,
            border_overlay: Color::Black,
            error: Color::Black,
            selection_bg: Color::Black,
            poll_bar: Color::Black,
            splash_version_label: Color::Black,
            splash_version_number: Color::Black,
        }
    }
}
