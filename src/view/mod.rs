pub mod command_bar;
pub mod filter_bar;
pub mod header;
pub mod message_list;
pub mod palette;
pub mod splash;

pub use palette::Palette;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

impl Theme {
    /// Detect the terminal background theme. Falls back to `Dark` on failure.
    pub fn detect() -> Self {
        let timeout = std::time::Duration::from_millis(100);
        match termbg::theme(timeout) {
            Ok(termbg::Theme::Light) => Self::Light,
            _ => Self::Dark,
        }
    }
}
