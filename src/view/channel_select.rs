use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding};

use super::{command_bar, filter_bar, header};

pub fn render(
    frame: &mut Frame,
    area: Rect,
    command_buf: Option<&str>,
    filter_editing: bool,
    filter: &str,
    all_channels: &[(String, String)],
    user_names: &[String],
    list_state: &mut ListState,
    poll_label: &str,
    workspace_label: &str,
) {
    let in_command_mode = command_buf.is_some();
    let in_filter_mode = filter_editing;
    let has_overlay = in_command_mode || in_filter_mode;

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(header::LOGO_HEIGHT), Constraint::Min(1)])
        .split(area);

    header::render(frame, outer[0], None, None, Some(poll_label), Some(workspace_label));
    let content_area = outer[1];

    let chunks = if has_overlay {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(content_area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1)])
            .split(content_area)
    };

    let (overlay_area, list_area) = if has_overlay { (Some(chunks[0]), chunks[1]) } else { (None, chunks[0]) };

    if let Some(overlay_area) = overlay_area {
        if in_command_mode {
            command_bar::render(frame, overlay_area, command_buf.unwrap_or(""), all_channels, user_names);
        } else if in_filter_mode {
            filter_bar::render(frame, overlay_area, filter);
        }
    }

    let filtered_channels: Vec<&(String, String)> = if filter.is_empty() {
        all_channels.iter().collect()
    } else {
        let q = filter.to_lowercase();
        all_channels.iter().filter(|(_, name)| name.to_lowercase().contains(&q)).collect()
    };

    let items: Vec<ListItem> = filtered_channels
        .iter()
        .map(|(_, name)| {
            ListItem::new(Line::from(vec![Span::styled(
                format!("#{}", name),
                Style::default().fg(Color::Rgb(255, 165, 0)),
            )]))
        })
        .collect();

    let filter_indicator = if !filter.is_empty() && !filter_editing {
        format!(" [/{}]", filter)
    } else {
        String::new()
    };

    let list_border_color = if has_overlay { Color::Blue } else { Color::Cyan };
    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(" slack9s \u{2014} select channel{} ", filter_indicator))
                .title_bottom(" enter: select | /: filter | :q to quit ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(list_border_color))
                .padding(Padding::new(1, 1, 0, 0)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, list_area, list_state);
}
