use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use std::time::Duration;

const SMALL_LOGO: &str = include_str!("logo_small.txt");

/// Logo height + 1 row for the poll indicator
pub const LOGO_HEIGHT: u16 = 6;

pub fn render(frame: &mut Frame, area: Rect, poll_interval: Option<Duration>, poll_elapsed: Option<Duration>) {
    let logo_width = SMALL_LOGO.lines().map(|l| l.len()).max().unwrap_or(0) as u16;
    let lines: Vec<Line> = SMALL_LOGO
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| Line::from(Span::styled(l, Style::default().fg(Color::Rgb(255, 165, 0)))))
        .collect();

    let x = area.right().saturating_sub(logo_width);
    let logo_area = Rect::new(x, area.y, logo_width, area.height.min(5));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, logo_area);

    // Poll progress bar under the logo, same width
    if let Some(interval) = poll_interval {
        let bar_width = logo_width as usize;
        let bar_y = area.y + 5;
        if bar_y < area.bottom() {
            let bar_area = Rect::new(x, bar_y, logo_width, 1);
            let spans = match poll_elapsed {
                Some(elapsed) => {
                    let elapsed_secs = elapsed.as_secs_f64();
                    let total_secs = interval.as_secs_f64().max(1.0);
                    if elapsed_secs < 1.0 {
                        // Just polled — flash all green
                        vec![Span::styled("\u{2588}".repeat(bar_width), Style::default().fg(Color::Green))]
                    } else {
                        let ratio = (elapsed_secs / total_secs).min(1.0);
                        let consumed = (ratio * bar_width as f64).round() as usize;
                        let remaining = bar_width.saturating_sub(consumed);
                        let mut s = Vec::new();
                        if remaining > 0 {
                            s.push(Span::styled("\u{2588}".repeat(remaining), Style::default().fg(Color::DarkGray)));
                        }
                        if consumed > 0 {
                            s.push(Span::styled("\u{2591}".repeat(consumed), Style::default().fg(Color::DarkGray)));
                        }
                        s
                    }
                }
                None => {
                    vec![Span::styled("\u{2591}".repeat(bar_width), Style::default().fg(Color::DarkGray))]
                }
            };
            let status_line = Paragraph::new(Line::from(spans));
            frame.render_widget(status_line, bar_area);
        }
    }
}
