use crate::config::Config;
use crate::model::{TrackedMessage, effective_category};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding};
use std::collections::{HashMap, HashSet};

use super::{command_bar, filter_bar, header};
use crate::cli;

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    command_buf: Option<&str>,
    command_error: bool,
    command_error_msg: Option<&str>,
    filter_buf: Option<&str>,
    channel_filter: Option<&str>,
    all_messages: &[&TrackedMessage],
    messages: &[&TrackedMessage],
    config: &Config,
    list_state: &mut ListState,
    poll: &header::PollState,
    team_name: &str,
    user_name: &str,
    active_categories: &HashSet<String>,
    show_uncategorised: bool,
    rollup_reactions: bool,
) {
    let has_filter_visible = filter_buf.is_some() || channel_filter.is_some_and(|f| !f.is_empty());
    let has_command = command_buf.is_some();
    let command_bar_height: u16 = if has_command && command_error_msg.is_some() { 4 } else { 3 };

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(header::LOGO_HEIGHT), Constraint::Min(1)])
        .split(area);

    header::render(
        frame,
        outer[0],
        Some(poll),
        &config.header.config_labels(),
        Some(team_name),
        Some(user_name),
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
    spans.push(Span::styled(" /", Style::default().fg(Color::White)));
    spans.push(Span::styled(
        "#channel",
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
    ));
    let commands_line = Line::from(spans);
    let cmd_area = Rect::new(outer[0].x, outer[0].bottom().saturating_sub(1), outer[0].width, 1);
    frame.render_widget(ratatui::widgets::Paragraph::new(commands_line), cmd_area);

    let content_area = outer[1];

    let mut overlay_constraints: Vec<Constraint> = Vec::new();
    if has_command {
        overlay_constraints.push(Constraint::Length(command_bar_height));
    }
    if has_filter_visible {
        overlay_constraints.push(Constraint::Length(3));
    }
    overlay_constraints.push(Constraint::Min(1));

    let overlay_count = has_command as usize + has_filter_visible as usize;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(overlay_constraints)
        .split(content_area);

    let list_area = chunks[overlay_count];

    // Render overlay bars
    let mut overlay_idx: usize = 0;
    if let Some(cbuf) = command_buf {
        command_bar::render(frame, chunks[overlay_idx], cbuf, command_error, command_error_msg);
        overlay_idx += 1;
    }
    if has_filter_visible {
        let area = chunks[overlay_idx];
        if let Some(fbuf) = filter_buf {
            filter_bar::render(frame, area, fbuf, true);
        } else if let Some(cf) = channel_filter
            && !cf.is_empty()
        {
            filter_bar::render(frame, area, cf, false);
        }
    }

    let items: Vec<ListItem> = messages
        .iter()
        .map(|m| {
            let mut spans = vec![
                Span::styled(format!("#{} ", m.channel_name), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("@{}", m.display_name), Style::default().fg(Color::Rgb(255, 165, 0))),
                Span::raw(": "),
            ];
            spans.extend(highlight_spans(&m.text));
            ListItem::new(Line::from(spans))
        })
        .collect();

    // Build top title with per-category message counts
    let root_by_ts: HashMap<&str, &TrackedMessage> = if rollup_reactions {
        all_messages
            .iter()
            .filter(|m| m.thread_ts.as_deref().is_none_or(|tts| tts == m.ts.as_str()))
            .map(|m| (m.ts.as_str(), *m))
            .collect()
    } else {
        HashMap::new()
    };
    let category_names: Vec<&String> = config.categories.keys().collect();
    let title = if category_names.is_empty() {
        String::from(" messages ")
    } else {
        let mut counts: Vec<(String, usize)> = Vec::new();
        let mut uncategorised_count: usize = 0;
        for msg in all_messages.iter() {
            let effective: &TrackedMessage = if rollup_reactions {
                msg.thread_ts
                    .as_ref()
                    .filter(|tts| tts.as_str() != msg.ts.as_str())
                    .and_then(|tts| root_by_ts.get(tts.as_str()).copied())
                    .unwrap_or(*msg)
            } else {
                msg
            };
            match effective_category(effective, &config.categories) {
                Some(cat) => {
                    if let Some(entry) = counts.iter_mut().find(|(n, _)| *n == cat) {
                        entry.1 += 1;
                    } else {
                        counts.push((cat, 1));
                    }
                }
                None => uncategorised_count += 1,
            }
        }
        let mut parts: Vec<String> = category_names
            .iter()
            .map(|name| {
                let count = counts.iter().find(|(n, _)| n == *name).map_or(0, |(_, c)| *c);
                format!("{} {}", count, name)
            })
            .collect();
        parts.push(format!("{} uncategorised", uncategorised_count));
        let stats = parts.join(", ");
        format!(" {} ", stats)
    };

    // Build bottom title for category toggles
    let category_names: Vec<&String> = config.categories.keys().collect();
    let bottom_title = if category_names.is_empty() {
        String::new()
    } else {
        let mut toggles: Vec<String> = category_names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let check = if active_categories.contains(*name) { "x" } else { " " };
                format!("{}) {} [{}]", i + 1, name, check)
            })
            .collect();
        let uncategorised_check = if show_uncategorised { "x" } else { " " };
        toggles.push(format!("0) uncategorised [{}]", uncategorised_check));
        let rollup_check = if rollup_reactions { "x" } else { " " };
        format!(" R) rollup reactions [{}], show categories: {} ", rollup_check, toggles.join(" "))
    };

    let list_border_color = if overlay_count > 0 { Color::Blue } else { Color::Cyan };
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

/// Parse \u{E000}...\u{E001} highlight markers in text and return spans
/// with matched portions styled in orange.
fn highlight_spans(text: &str) -> Vec<Span<'_>> {
    let mut spans = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('\u{E000}') {
        if start > 0 {
            spans.push(Span::raw(&rest[..start]));
        }
        rest = &rest[start + '\u{E000}'.len_utf8()..];
        if let Some(end) = rest.find('\u{E001}') {
            spans.push(Span::styled(
                &rest[..end],
                Style::default().fg(Color::Rgb(255, 165, 0)).add_modifier(Modifier::BOLD),
            ));
            rest = &rest[end + '\u{E001}'.len_utf8()..];
        }
    }
    if !rest.is_empty() {
        spans.push(Span::raw(rest));
    }
    spans
}
