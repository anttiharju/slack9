use crate::config::Config;
use crate::model::TrackedMessage;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding};
use std::time::Duration;

use super::{command_bar, header};
use crate::cli;

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    command_buf: Option<&str>,
    all_channels: &[(String, String)],
    user_names: &[String],
    messages: &[TrackedMessage],
    tracked_channels: &[(String, String)],
    config: &Config,
    list_state: &mut ListState,
    poll: Duration,
    poll_elapsed: Option<Duration>,
    team_name: &str,
) {
    let has_overlay = command_buf.is_some();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(header::LOGO_HEIGHT), Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    header::render(
        frame,
        outer[0],
        Some(poll),
        poll_elapsed,
        Some(&config.header.poll_label()),
        Some(team_name),
        Some(&config.header.past_label()),
    );

    // Commands hint line
    let cmd_names: String = cli::tui_command_names().iter().map(|n| format!(":{}", n)).collect::<Vec<_>>().join(" ");
    let commands_line = Line::from(vec![
        Span::styled("commands", Style::default().fg(Color::Rgb(255, 165, 0)).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {}", cmd_names), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]);
    frame.render_widget(ratatui::widgets::Paragraph::new(commands_line), outer[1]);

    let content_area = outer[2];

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

    let items: Vec<ListItem> = messages
        .iter()
        .map(|m| {
            let spans = vec![
                Span::styled(format!("#{} ", m.channel_name), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("@{}", m.display_name), Style::default().fg(Color::Rgb(255, 165, 0))),
                Span::raw(format!(": {}", m.text)),
            ];
            ListItem::new(Line::from(spans))
        })
        .collect();

    let view_label = if tracked_channels.is_empty() {
        "search".to_string()
    } else {
        tracked_channels
            .iter()
            .map(|(_, name)| format!("#{}", name))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let title = format!(" {} ", view_label);

    let list_border_color = if has_overlay { Color::Blue } else { Color::Cyan };
    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(list_border_color))
                .padding(Padding::new(1, 1, 0, 0)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, list_area, list_state);
}
