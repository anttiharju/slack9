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
            text_muted: Color::Gray,
            border_focused: Color::Cyan,
            border_unfocused: Color::DarkGray,
            border_overlay: Color::Gray,
            error: Color::Red,
            selection_bg: Color::DarkGray,
            poll_bar: Color::DarkGray,
            splash_version_label: Color::Cyan,
            splash_version_number: Color::Red,
        }
    }

    fn light() -> Self {
        Self {
            accent: Color::Rgb(232, 127, 36), // Slighly more dark orange for better contrast on light background
            text: Color::Rgb(100, 100, 100),  // Medium gray
            text_muted: Color::Gray,
            border_focused: Color::Cyan,
            border_unfocused: Color::White,
            border_overlay: Color::Gray,
            error: Color::Red,
            selection_bg: Color::White,
            poll_bar: Color::White,
            splash_version_label: Color::Gray,
            splash_version_number: Color::LightRed,
        }
    }
}
