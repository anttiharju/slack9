use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

use super::Palette;

pub fn render(frame: &mut Frame, area: Rect, buf: &str, focused: bool, palette: &Palette) {
    let border_color = if focused { palette.border_focused } else { palette.border_unfocused };
    let spans = vec![Span::raw("/"), Span::raw(buf.to_string())];
    let paragraph = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .title(" filter channel ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .padding(Padding::new(1, 1, 0, 0)),
    );
    frame.render_widget(paragraph, area);
}
