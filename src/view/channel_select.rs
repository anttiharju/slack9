use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding};

use super::{command_bar, header};

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    command_buf: Option<&str>,
    all_channels: &[(String, String)],
    user_names: &[String],
    list_state: &mut ListState,
    poll_label: &str,
    workspace_label: &str,
    past_label: &str,
) {
    let has_overlay = command_buf.is_some();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(header::LOGO_HEIGHT), Constraint::Min(1)])
        .split(area);

    header::render(frame, outer[0], None, None, Some(poll_label), Some(workspace_label), Some(past_label));
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
        command_bar::render(frame, overlay_area, command_buf.unwrap_or(""), all_channels, user_names);
    }

    let items: Vec<ListItem> = all_channels
        .iter()
        .map(|(_, name)| {
            ListItem::new(Line::from(vec![Span::styled(
                format!("#{}", name),
                Style::default().fg(Color::Rgb(255, 165, 0)),
            )]))
        })
        .collect();

    let list_border_color = if has_overlay { Color::Blue } else { Color::Cyan };
    let list = List::new(items)
        .block(
            Block::default()
                .title(" slack9 \u{2014} select channel ")
                .title_bottom(" enter: select | :q to quit ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(list_border_color))
                .padding(Padding::new(1, 1, 0, 0)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, list_area, list_state);
}
