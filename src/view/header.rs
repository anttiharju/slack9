use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const SMALL_LOGO: &str = include_str!("logo_small.txt");

pub const LOGO_HEIGHT: u16 = 5;

pub fn render(frame: &mut Frame, area: Rect) {
    let lines: Vec<Line> = SMALL_LOGO
        .lines()
        .map(|l| Line::from(Span::styled(l, Style::default().fg(Color::Cyan))))
        .collect();

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}
