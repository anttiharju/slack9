use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

pub fn render(frame: &mut Frame, area: Rect, buf: &str, error: bool) {
    let spans = vec![Span::raw(":"), Span::raw(buf.to_string())];
    let border_color = if error { Color::Red } else { Color::Cyan };
    let paragraph = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .title(" command ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .padding(Padding::new(1, 1, 0, 0)),
    );
    frame.render_widget(paragraph, area);
}
