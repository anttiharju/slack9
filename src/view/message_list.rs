use crate::config::Config;
use crate::model::TrackedMessage;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding};
use std::time::Duration;

use super::{command_bar, header};

const STATUS_COLORS: &[Color] = &[Color::Yellow, Color::Blue, Color::Red, Color::Green, Color::Magenta, Color::Cyan];

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
    poll_interval: Duration,
    poll_elapsed: Option<Duration>,
    team_name: &str,
) {
    let has_overlay = command_buf.is_some();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(header::LOGO_HEIGHT), Constraint::Min(1)])
        .split(area);

    header::render(
        frame,
        outer[0],
        Some(poll_interval),
        poll_elapsed,
        Some(&config.poll_interval.to_string()),
        Some(team_name),
        Some(&config.time_window),
    );
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

    let last_status = config.last_status_index();
    let has_reactions = !config.reactions.is_empty();
    let items: Vec<ListItem> = messages
        .iter()
        .filter(|m| last_status != Some(m.status))
        .map(|m| {
            let mut spans = Vec::new();
            if has_reactions
                && let Some((name, _)) = config.reactions.get_index(m.status) {
                    let label = name.replace('_', " ");
                    let color = STATUS_COLORS[m.status % STATUS_COLORS.len()];
                    spans.push(Span::styled(
                        format!("[{:<14}] ", label),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ));
                }
            spans.push(Span::styled(format!("#{} ", m.channel_name), Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(format!("@{}", m.display_name), Style::default().fg(Color::Rgb(255, 165, 0))));
            spans.push(Span::raw(format!(": {}", m.text)));
            ListItem::new(Line::from(spans))
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

    let list_border_color = if has_overlay { Color::Blue } else { Color::Cyan };
    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .title_bottom(" :q to quit ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(list_border_color))
                .padding(Padding::new(1, 1, 0, 0)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, list_area, list_state);
}
