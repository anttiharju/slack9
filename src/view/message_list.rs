use crate::config::Config;
use crate::model::{Status, TrackedMessage};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};

use super::{command_bar, filter_bar};

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
) {
    let in_command_mode = command_buf.is_some();
    let in_filter_mode = filter_editing;
    let has_overlay = in_command_mode || in_filter_mode;

    let chunks = if has_overlay {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(1)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area)
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

    let status_line = Paragraph::new("");
    frame.render_widget(status_line, status_area);
}
