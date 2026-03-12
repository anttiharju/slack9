use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

pub fn render(frame: &mut Frame, area: Rect, buf: &str, error: bool, error_msg: Option<&str>) {
    let border_color = if error { Color::Red } else { Color::Cyan };
    let mut lines: Vec<Line> = Vec::new();
    if let Some(msg) = error_msg {
        lines.push(Line::from(Span::styled(
            msg,
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(vec![Span::raw(":"), Span::raw(buf.to_string())]));
    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(" command ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .padding(Padding::new(1, 1, 0, 0)),
    );
    frame.render_widget(paragraph, area);
}
