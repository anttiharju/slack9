use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Flex, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const BIG_LOGO: &str = include_str!("logo_big.txt");

pub fn render(frame: &mut Frame) {
    let area = frame.area();

    let lines: Vec<Line> = BIG_LOGO
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| Line::from(Span::styled(l, Style::default().fg(Color::Cyan))))
        .collect();

    let logo_height = lines.len() as u16;
    let logo_width = BIG_LOGO.lines().map(|l| l.len() as u16).max().unwrap_or(0);

    // Center vertically
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(logo_height), Constraint::Min(0)])
        .flex(Flex::Center)
        .split(area);

    // Center horizontally
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(logo_width), Constraint::Min(0)])
        .flex(Flex::Center)
        .split(vertical[1]);

    let centered = Rect::new(horizontal[1].x, vertical[1].y, horizontal[1].width, vertical[1].height);

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, centered);
}
