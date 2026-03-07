use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding};

use super::command_bar;

pub fn render(frame: &mut Frame, area: Rect, command_buf: Option<&str>, all_channels: &[(String, String)], list_state: &mut ListState) {
    let in_command_mode = command_buf.is_some();

    let chunks = if in_command_mode {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1)])
            .split(area)
    };

    let (cmd_area, list_area) = if in_command_mode {
        (Some(chunks[0]), chunks[1])
    } else {
        (None, chunks[0])
    };

    if let Some(cmd_area) = cmd_area {
        command_bar::render(frame, cmd_area, command_buf.unwrap_or(""), all_channels);
    }

    let items: Vec<ListItem> = all_channels
        .iter()
        .map(|(_, name)| ListItem::new(Line::from(vec![Span::styled(format!("#{}", name), Style::default().fg(Color::Cyan))])))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" slack9s \u{2014} select channel ")
                .title_bottom(" enter: select | :q to quit ")
                .borders(Borders::ALL)
                .padding(Padding::new(1, 1, 0, 0)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, list_area, list_state);
}
