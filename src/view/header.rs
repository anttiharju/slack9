use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use std::time::Duration;

use super::Palette;

const SMALL_LOGO: &str = include_str!("header_logo.txt");

/// Logo height + 1 row for the poll indicator
pub const LOGO_HEIGHT: u16 = 6;

const BLOCKS: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

const TAIL_LEN: f64 = 7.0;

/// Fraction of the poll interval used by the wave phase (the rest is drain).
/// Computed from the logo width which determines num_cells.
pub fn wave_fraction() -> f64 {
    let logo_width = SMALL_LOGO.lines().map(|l| l.len()).max().unwrap_or(0);
    let num_cells = logo_width.div_ceil(2);
    let wave_steps = (num_cells - 1) as f64 + TAIL_LEN;
    let total_steps = wave_steps + TAIL_LEN;
    wave_steps / total_steps
}

/// Wave tail: for a cell at integer distance `d` behind the peak returns block level 0–7.
///
/// Each step drops by one level: █ ▇ ▆ ▅ ▄ ▃ ▂ ▁
fn wave_level_at(d: usize) -> usize {
    7_usize.saturating_sub(d)
}

pub struct PollState {
    pub interval: Duration,
    pub elapsed: Option<Duration>,
    pub in_flight: bool,
    pub drain_elapsed: Option<Duration>,
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    poll: Option<&PollState>,
    config_labels: &[(&str, String)],
    workspace_label: Option<&str>,
    user_label: Option<&str>,
    palette: &Palette,
) {
    let logo_width = SMALL_LOGO.lines().map(|l| l.len()).max().unwrap_or(0) as u16;
    let lines: Vec<Line> = SMALL_LOGO
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| Line::from(Span::styled(l, Style::default().fg(palette.accent))))
        .collect();

    let x = area.right().saturating_sub(logo_width).saturating_sub(1);
    let bar_x = area.right().saturating_sub(logo_width);
    let logo_area = Rect::new(x, area.y, logo_width, area.height.min(5));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, logo_area);

    // Workspace label in top-left
    if let Some(name) = workspace_label {
        let label_line = Line::from(vec![
            Span::styled("Workspace ", Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
            Span::styled(name, Style::default().fg(palette.text).add_modifier(Modifier::BOLD)),
        ]);
        let label_area = Rect::new(area.x, area.y, area.width.saturating_sub(logo_width + 1), 1);
        frame.render_widget(Paragraph::new(label_line), label_area);
    }

    // User label below workspace
    if let Some(name) = user_label {
        let label_line = Line::from(vec![
            Span::styled("User ", Style::default().fg(palette.accent).add_modifier(Modifier::BOLD)),
            Span::styled(name, Style::default().fg(palette.text).add_modifier(Modifier::BOLD)),
        ]);
        let label_area = Rect::new(area.x, area.y + 1, area.width.saturating_sub(logo_width + 1), 1);
        frame.render_widget(Paragraph::new(label_line), label_area);
    }

    // Config labels on rows below user
    for (i, (name, value)) in config_labels.iter().enumerate() {
        let title = format!("{}{} ", name.chars().next().unwrap_or_default().to_uppercase(), &name[1..]);
        let mut spans = vec![Span::styled(title, Style::default().fg(palette.accent).add_modifier(Modifier::BOLD))];
        if let Some(val) = value.strip_suffix(" (default)") {
            spans.push(Span::styled(val, Style::default().fg(palette.text).add_modifier(Modifier::BOLD)));
            spans.push(Span::styled(" (default)", Style::default().fg(palette.text_muted)));
        } else {
            spans.push(Span::styled(
                value.as_str(),
                Style::default().fg(palette.text).add_modifier(Modifier::BOLD),
            ));
        }
        let label_line = Line::from(spans);
        // Place on rows below user label
        let row = if i == 0 { area.y + 3 } else { area.y + 2 };
        let label_area = Rect::new(area.x, row, area.width.saturating_sub(logo_width + 1), 1);
        frame.render_widget(Paragraph::new(label_line), label_area);
    }

    // Poll progress bar under the logo, same width
    if let Some(ps) = poll {
        let interval = ps.interval;
        let poll_in_flight = ps.in_flight;
        let drain_elapsed = ps.drain_elapsed;
        let bar_width = logo_width as usize;
        let bar_y = area.y + 5;
        if bar_y >= area.bottom() {
            return;
        }
        let bar_area = Rect::new(bar_x, bar_y, logo_width, 1);
        let num_cells = bar_width.div_ceil(2); // each cell = block + space

        // Total animation = wave + drain, both at the same step rate, fitting in `poll`.
        // Wave: peak travels (num_cells - 1 + TAIL_LEN) steps.
        // Drain: descends TAIL_LEN (7) steps.
        let cycle_secs = interval.as_secs_f64().max(1.0);
        let wave_steps = (num_cells - 1) as f64 + TAIL_LEN;
        let total_steps = wave_steps + TAIL_LEN;
        let wave_fraction = wave_steps / total_steps;
        let wave_duration_secs = cycle_secs * wave_fraction;
        let drain_duration_secs = cycle_secs * (1.0 - wave_fraction);

        let elapsed_secs = ps.elapsed.map_or(0.0, |e| e.as_secs_f64());
        let wave_progress = (elapsed_secs / wave_duration_secs).clamp(0.0, 1.0);

        let mut bar = String::with_capacity(bar_width * 4);

        if poll_in_flight || (drain_elapsed.is_none() && wave_progress >= 1.0) {
            // Explosion: all cells full while fetch is in-flight
            for i in 0..num_cells {
                bar.push(BLOCKS[7]);
                if i + 1 < num_cells {
                    bar.push(' ');
                }
            }
        } else if let Some(drain_el) = drain_elapsed {
            // Drain: descent from █ to ▁ at the same step rate as the wave
            let drain_progress = (drain_el.as_secs_f64() / drain_duration_secs).clamp(0.0, 1.0);
            let level = ((1.0 - drain_progress) * 7.0).round() as usize;
            for i in 0..num_cells {
                bar.push(BLOCKS[level]);
                if i + 1 < num_cells {
                    bar.push(' ');
                }
            }
        } else if wave_progress < 1.0 {
            // Wave phase: peak sweeps left → right with a trailing gradient
            let peak = -TAIL_LEN + wave_progress * ((num_cells - 1) as f64 + TAIL_LEN);

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
        } else {
            // Idle: all ▁
            for i in 0..num_cells {
                bar.push(BLOCKS[0]);
                if i + 1 < num_cells {
                    bar.push(' ');
                }
            }
        }

        let spans = vec![Span::styled(bar, Style::default().fg(palette.poll_bar))];
        let status_line = Paragraph::new(Line::from(spans));
        frame.render_widget(status_line, bar_area);
    }
}
