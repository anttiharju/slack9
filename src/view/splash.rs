use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const BIG_LOGO: &str = include_str!("logo_big.txt");

pub fn render(frame: &mut Frame) {
    let area = frame.area();

    let lines: Vec<Line> = BIG_LOGO
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| Line::from(Span::styled(l, Style::default().fg(Color::Rgb(255, 165, 0)))))
        .collect();

    let logo_height = lines.len() as u16;
    let logo_width = BIG_LOGO.lines().map(|l| l.len() as u16).max().unwrap_or(0);
    let total_height = logo_height + 2; // logo + padding + version line

    // Center vertically
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(total_height), Constraint::Min(0)])
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

    // Version below logo with 1 row padding, centered
    let version = env!("CARGO_PKG_VERSION");
    let version_line = Line::from(vec![
        Span::styled("Version ", Style::default().fg(Color::Rgb(0, 206, 209)).add_modifier(Modifier::BOLD)),
        Span::styled(version, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
    ]);
    let version_area = Rect::new(horizontal[1].x, vertical[1].y + logo_height + 1, horizontal[1].width, 1);
    let version_paragraph = Paragraph::new(version_line).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(version_paragraph, version_area);
}
