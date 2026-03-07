use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const SMALL_LOGO: &str = include_str!("logo_small.txt");

pub const LOGO_HEIGHT: u16 = 5;

pub fn render(frame: &mut Frame, area: Rect) {
    let logo_width = SMALL_LOGO.lines().map(|l| l.len()).max().unwrap_or(0) as u16;
    let lines: Vec<Line> = SMALL_LOGO
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| Line::from(Span::styled(l, Style::default().fg(Color::Rgb(255, 165, 0)))))
        .collect();

    let x = area.right().saturating_sub(logo_width);
    let logo_area = Rect::new(x, area.y, logo_width, area.height);

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, logo_area);
}
