use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use std::time::Duration;

const SMALL_LOGO: &str = include_str!("logo_small.txt");

/// Logo height + 1 row for the poll indicator
pub const LOGO_HEIGHT: u16 = 6;

const BLOCKS: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

// Phase boundaries (fractions of the cycle)
const WAVE_END: f64 = 0.50; // 50% wave
const EXPLOSION_END: f64 = 0.54; // 4% all-filled
const DRAIN_END: f64 = 0.85; // 31% drain
// hold: 15% all-▁

/// Wave tail: for a cell at integer distance `d` behind the peak returns block level 0–7.
///
/// Each step drops by one level: █ ▇ ▆ ▅ ▄ ▃ ▂ ▁
fn wave_level_at(d: usize) -> usize {
    7_usize.saturating_sub(d)
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    poll_interval: Option<Duration>,
    poll_elapsed: Option<Duration>,
    poll_label: Option<&str>,
    workspace_label: Option<&str>,
) {
    let logo_width = SMALL_LOGO.lines().map(|l| l.len()).max().unwrap_or(0) as u16;
    let lines: Vec<Line> = SMALL_LOGO
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| Line::from(Span::styled(l, Style::default().fg(Color::Rgb(255, 165, 0)))))
        .collect();

    let x = area.right().saturating_sub(logo_width).saturating_sub(1);
    let bar_x = area.right().saturating_sub(logo_width);
    let logo_area = Rect::new(x, area.y, logo_width, area.height.min(5));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, logo_area);

    // Workspace label in top-left
    if let Some(ws) = workspace_label {
        let short = ws
            .trim_end_matches('/')
            .split("//")
            .last()
            .and_then(|h| h.split('.').next())
            .unwrap_or(ws);
        let label_line = Line::from(vec![
            Span::styled("WORKSPACE ", Style::default().fg(Color::Rgb(255, 165, 0)).add_modifier(Modifier::BOLD)),
            Span::styled(short, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]);
        let label_area = Rect::new(area.x, area.y, area.width.saturating_sub(logo_width + 1), 1);
        frame.render_widget(Paragraph::new(label_line), label_area);
    }

    // Poll interval label on second line
    if let Some(label) = poll_label {
        let label_line = Line::from(vec![
            Span::styled("POLL ", Style::default().fg(Color::Rgb(255, 165, 0)).add_modifier(Modifier::BOLD)),
            Span::styled(label, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]);
        let label_area = Rect::new(area.x, area.y + 1, area.width.saturating_sub(logo_width + 1), 1);
        frame.render_widget(Paragraph::new(label_line), label_area);
    }

    // Poll progress bar under the logo, same width
    if let Some(interval) = poll_interval {
        let bar_width = logo_width as usize;
        let bar_y = area.y + 5;
        if bar_y >= area.bottom() {
            return;
        }
        let bar_area = Rect::new(bar_x, bar_y, logo_width, 1);
        let num_cells = bar_width.div_ceil(2); // each cell = block + space

        let cycle_secs = interval.as_secs_f64().max(1.0);
        let elapsed_secs = poll_elapsed.map_or(0.0, |e| e.as_secs_f64());
        let progress = (elapsed_secs / cycle_secs).min(1.0);

        let mut bar = String::with_capacity(bar_width * 4);

        if progress < WAVE_END {
            // Wave phase: peak sweeps left → right with a trailing gradient
            let wave_progress = progress / WAVE_END;
            let tail_len = 7.0_f64;
            // Peak travels from off-screen left to the last cell
            let peak = -tail_len + wave_progress * ((num_cells - 1) as f64 + tail_len);

            for i in 0..num_cells {
                let d = peak - i as f64;
                let level = if d < 0.0 {
                    0 // ahead of the peak
                } else {
                    wave_level_at(d.round() as usize)
                };
                bar.push(BLOCKS[level]);
                if i + 1 < num_cells {
                    bar.push(' ');
                }
            }
        } else if progress < EXPLOSION_END {
            // Explosion: all cells snap to full
            for i in 0..num_cells {
                bar.push(BLOCKS[7]);
                if i + 1 < num_cells {
                    bar.push(' ');
                }
            }
        } else if progress < DRAIN_END {
            // Drain: uniform descent from █ to ▁
            let drain_progress = (progress - EXPLOSION_END) / (DRAIN_END - EXPLOSION_END);
            let level = ((1.0 - drain_progress) * 7.0).round() as usize;
            for i in 0..num_cells {
                bar.push(BLOCKS[level]);
                if i + 1 < num_cells {
                    bar.push(' ');
                }
            }
        } else {
            // Hold: all ▁ until next poll
            for i in 0..num_cells {
                bar.push(BLOCKS[0]);
                if i + 1 < num_cells {
                    bar.push(' ');
                }
            }
        }

        let spans = vec![Span::styled(bar, Style::default().fg(Color::DarkGray))];
        let status_line = Paragraph::new(Line::from(spans));
        frame.render_widget(status_line, bar_area);
    }
}
