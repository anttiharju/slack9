use crate::config::Config;
use crate::model::{Status, TrackedMessage};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};
use std::time::Duration;

use super::{command_bar, filter_bar, header};

pub fn render(
    frame: &mut Frame,
    area: Rect,
    command_buf: Option<&str>,
    filter_editing: bool,
    filter: &str,
    all_channels: &[(String, String)],
    messages: &[TrackedMessage],
    tracked_channels: &[(String, String)],
    config: &Config,
    list_state: &mut ListState,
    poll_interval: Duration,
    poll_elapsed: Option<Duration>,
) {
    let in_command_mode = command_buf.is_some();
    let in_filter_mode = filter_editing;
    let has_overlay = in_command_mode || in_filter_mode;

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(header::LOGO_HEIGHT + 1), Constraint::Min(1)])
        .split(area);

    header::render(frame, outer[0]);
    let content_area = outer[1];

    let chunks = if has_overlay {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(1)])
            .split(content_area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(content_area)
    };

    let (overlay_area, list_area, status_area) = if has_overlay {
        (Some(chunks[0]), chunks[1], chunks[2])
    } else {
        (None, chunks[0], chunks[1])
    };

    if let Some(overlay_area) = overlay_area {
        if in_command_mode {
            command_bar::render(frame, overlay_area, command_buf.unwrap_or(""), all_channels);
        } else if in_filter_mode {
            filter_bar::render(frame, overlay_area, filter);
        }
    }

    let q = filter.to_lowercase();
    let items: Vec<ListItem> = messages
        .iter()
        .filter(|m| m.status != Status::Completed)
        .filter(|m| {
            if q.is_empty() {
                return true;
            }
            m.text.to_lowercase().contains(&q) || m.display_name.to_lowercase().contains(&q) || m.channel_name.to_lowercase().contains(&q)
        })
        .map(|m| {
            let (label, color) = match m.status {
                Status::Backlog => ("backlog", Color::Yellow),
                Status::TakingALook => ("taking a look", Color::Blue),
                Status::Blocked => ("blocked", Color::Red),
                Status::Completed => unreachable!(),
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("[{:<14}] ", label), Style::default().fg(color).add_modifier(Modifier::BOLD)),
                Span::styled(format!("#{} ", m.channel_name), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("@{}", m.display_name), Style::default().fg(Color::Cyan)),
                Span::raw(format!(": {}", m.text)),
            ]))
        })
        .collect();

    let channel_list: String = tracked_channels
        .iter()
        .map(|(_, name)| format!("#{}", name))
        .collect::<Vec<_>>()
        .join(", ");

    let filter_indicator = if !filter.is_empty() && !filter_editing {
        format!(" [/{}]", filter)
    } else {
        String::new()
    };

    let title = format!(
        " slack9s \u{2014} {} (every {}, {} window){} ",
        channel_list, config.poll_interval, config.time_window, filter_indicator,
    );

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .title_bottom(" :q to quit | /: filter ")
                .borders(Borders::ALL)
                .padding(Padding::new(1, 1, 0, 0)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, list_area, list_state);

    let total_blocks = poll_interval.as_secs().max(1) as usize;
    let spans = match poll_elapsed {
        Some(elapsed) => {
            let elapsed_secs = elapsed.as_secs() as usize;
            if elapsed_secs < 1 {
                // Just polled — flash all green
                vec![Span::styled("\u{2588}".repeat(total_blocks), Style::default().fg(Color::Green))]
            } else {
                let remaining = total_blocks.saturating_sub(elapsed_secs);
                let consumed = total_blocks - remaining;
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
            // Haven't polled yet — show all dim
            vec![Span::styled("\u{2591}".repeat(total_blocks), Style::default().fg(Color::DarkGray))]
        }
    };
    let status_line = Paragraph::new(Line::from(spans));
    frame.render_widget(status_line, status_area);
}
