use crate::input;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

pub fn render(frame: &mut Frame, area: Rect, buf: &str, all_channels: &[(String, String)]) {
    let ghost = input::ghost_completion(buf, all_channels);
    let mut spans = vec![Span::raw(":"), Span::raw(buf.to_string())];
    if !ghost.is_empty() {
        spans.push(Span::styled(ghost, Style::default().fg(Color::DarkGray)));
    }
    let paragraph = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .title(" command ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .padding(Padding::new(1, 1, 0, 0)),
    );
    frame.render_widget(paragraph, area);
}
