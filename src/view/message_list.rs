use crate::config::Config;
use crate::model::{Status, TrackedMessage};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};

use super::command_bar;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    command_buf: Option<&str>,
    all_channels: &[(String, String)],
    messages: &[TrackedMessage],
    tracked_channels: &[(String, String)],
    config: &Config,
    list_state: &mut ListState,
) {
    let in_command_mode = command_buf.is_some();

    let chunks = if in_command_mode {
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

    let (cmd_area, list_area, status_area) = if in_command_mode {
        (Some(chunks[0]), chunks[1], chunks[2])
    } else {
        (None, chunks[0], chunks[1])
    };

    if let Some(cmd_area) = cmd_area {
        command_bar::render(frame, cmd_area, command_buf.unwrap_or(""), all_channels);
    }

    let items: Vec<ListItem> = messages
        .iter()
        .filter(|m| m.status != Status::Completed)
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

    let title = format!(
        " slack9s \u{2014} {} (every {}, {} window) ",
        channel_list, config.poll_interval, config.time_window,
    );

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .title_bottom(" :q to quit ")
                .borders(Borders::ALL)
                .padding(Padding::new(1, 1, 0, 0)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, list_area, list_state);

    let status_line = Paragraph::new("");
    frame.render_widget(status_line, status_area);
}
