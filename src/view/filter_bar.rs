use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

pub fn render(frame: &mut Frame, area: Rect, query: &str) {
    let spans = vec![Span::raw("/"), Span::raw(query.to_string())];
    let paragraph =
        Paragraph::new(Line::from(spans)).block(Block::default().title(" filter ").borders(Borders::ALL).padding(Padding::new(1, 1, 0, 0)));
    frame.render_widget(paragraph, area);
}

/// Renders a small inline indicator when a filter is active but not being edited.
pub fn render_indicator(filter: &str) -> Span<'_> {
    Span::styled(format!(" [/{}]", filter), Style::default().fg(Color::Yellow))
}
