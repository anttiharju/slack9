use crate::config::Config;
use crate::model::TrackedMessage;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding};
use std::collections::HashSet;
use std::time::Duration;

use super::{command_bar, header};
use crate::cli;

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    command_buf: Option<&str>,
    command_error: bool,
    all_channels: &[(String, String)],
    user_names: &[String],
    messages: &[&TrackedMessage],
    tracked_channels: &[(String, String)],
    config: &Config,
    list_state: &mut ListState,
    poll: Duration,
    poll_elapsed: Option<Duration>,
    poll_in_flight: bool,
    team_name: &str,
    active_reactions: &HashSet<String>,
) {
    let has_overlay = command_buf.is_some();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(header::LOGO_HEIGHT), Constraint::Min(1)])
        .split(area);

    header::render(
        frame,
        outer[0],
        Some(poll),
        poll_elapsed,
        poll_in_flight,
        &config.header.config_labels(),
        Some(team_name),
    );

    // Commands hint on the same row as the poll indicator (last row of header)
    let mut spans = vec![Span::styled(
        "commands",
        Style::default().fg(Color::Rgb(255, 165, 0)).add_modifier(Modifier::BOLD),
    )];
    for (prefix, rest) in cli::tui_command_prefixes() {
        spans.push(Span::styled(format!(" :{}", prefix), Style::default().fg(Color::White)));
        spans.push(Span::styled(rest, Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)));
    }
    let commands_line = Line::from(spans);
    let cmd_area = Rect::new(outer[0].x, outer[0].bottom().saturating_sub(1), outer[0].width, 1);
    frame.render_widget(ratatui::widgets::Paragraph::new(commands_line), cmd_area);

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
        command_bar::render(frame, overlay_area, command_buf.unwrap_or(""), command_error, all_channels, user_names);
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

    // Build bottom title for reaction toggles
    let reaction_names: Vec<&String> = config.reactions.keys().collect();
    let bottom_title = if reaction_names.is_empty() {
        String::new()
    } else {
        let toggles: Vec<String> = reaction_names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let check = if active_reactions.contains(*name) { "x" } else { " " };
                format!("{}) {} [{}]", i + 1, name, check)
            })
            .collect();
        format!(" show messages with reactions for: {} ", toggles.join(" "))
    };

    let list_border_color = if has_overlay { Color::Blue } else { Color::Cyan };
    let mut block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(list_border_color))
        .padding(Padding::new(1, 1, 0, 0));
    if !bottom_title.is_empty() {
        block = block.title_bottom(bottom_title);
    }
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, list_area, list_state);
}
