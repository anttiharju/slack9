mod cli;
mod config;
mod exitcode;
mod slack;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};
use std::collections::HashMap;
use std::env;
use std::io;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Backlog,
    TakingALook,
    Blocked,
    Completed,
}

struct TrackedMessage {
    channel_name: String,
    display_name: String,
    text: String,
    status: Status,
}

fn determine_status(msg: &slack::Message, reactions: &config::ReactionsConfig) -> Status {
    if msg.has_any_reaction(&reactions.completed) {
        Status::Completed
    } else if msg.has_any_reaction(&reactions.blocked) {
        Status::Blocked
    } else if msg.has_any_reaction(&reactions.taking_a_look) {
        Status::TakingALook
    } else {
        Status::Backlog
    }
}

fn main() {
    cli::parse_args();

    let config = config::load().unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(exitcode::config_load_error());
    });

    let time_window = config.time_window_duration().unwrap_or_else(|e| {
        eprintln!("Error: invalid time_window: {}", e);
        std::process::exit(exitcode::invalid_time_window());
    });

    let poll_interval = config.poll_interval_duration().unwrap_or_else(|e| {
        eprintln!("Error: invalid poll_interval: {}", e);
        std::process::exit(exitcode::invalid_poll_interval());
    });

    let xoxd = env::var("SLACK9S_XOXD").unwrap_or_else(|_| {
        eprintln!("Error: SLACK9S_XOXD environment variable not set");
        std::process::exit(exitcode::missing_xoxd());
    });

    let xoxc = env::var("SLACK9S_XOXC").unwrap_or_else(|_| {
        eprintln!("Error: SLACK9S_XOXC environment variable not set");
        std::process::exit(exitcode::missing_xoxc());
    });

    let workspace_url = env::var("SLACK9S_WORKSPACE_URL").unwrap_or_else(|_| {
        eprintln!("Error: SLACK9S_WORKSPACE_URL environment variable not set");
        std::process::exit(exitcode::missing_workspace_url());
    });

    let mut client = slack::SlackClient::new(workspace_url, xoxd, xoxc);

    match client.auth_test() {
        Ok(response) if response.ok => {}
        Ok(response) => {
            eprintln!("Auth failed: {}", response.error.unwrap_or_else(|| "unknown error".to_string()));
            std::process::exit(exitcode::auth_rejected());
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(exitcode::request_failed());
        }
    }

    println!(
        "\nPolling every {} for messages within {} window...\n",
        config.poll_interval, config.time_window
    );

    client.load_users().unwrap_or_else(|e| {
        eprintln!("Error loading users: {}", e);
        std::process::exit(exitcode::user_load_error());
    });

    let channels = client.resolve_channels(&config.channels).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(exitcode::channel_resolve_error());
    });

    // Setup terminal
    enable_raw_mode().expect("failed to enable raw mode");
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen).expect("failed to enter alternate screen");
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).expect("failed to create terminal");

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic);
    }));

    let mut messages: Vec<TrackedMessage> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut last_poll: Option<Instant> = None;
    let mut command_buf: Option<String> = None;
    let mut list_state = ListState::default();
    let mut pending_g: Option<char> = None;

    loop {
        if event::poll(Duration::from_millis(100)).unwrap_or(false)
            && let Ok(Event::Key(key)) = event::read()
            && key.kind == KeyEventKind::Press
        {
            if let Some(ref mut buf) = command_buf {
                match key.code {
                    KeyCode::Enter => {
                        let cmd = buf.trim().to_string();
                        command_buf = None;
                        if cmd == "q" || cmd == "q!" {
                            break;
                        }
                    }
                    KeyCode::Esc => {
                        command_buf = None;
                    }
                    KeyCode::Backspace => {
                        buf.pop();
                        if buf.is_empty() {
                            command_buf = None;
                        }
                    }
                    KeyCode::Char(c) => {
                        buf.push(c);
                    }
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Char(':') => {
                        pending_g = None;
                        command_buf = Some(String::new());
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        pending_g = None;
                        let visible_count = messages.iter().filter(|m| m.status != Status::Completed).count();
                        if visible_count > 0 {
                            let i = match list_state.selected() {
                                Some(i) => {
                                    if i == 0 {
                                        visible_count - 1
                                    } else {
                                        i - 1
                                    }
                                }
                                None => visible_count - 1,
                            };
                            list_state.select(Some(i));
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        pending_g = None;
                        let visible_count = messages.iter().filter(|m| m.status != Status::Completed).count();
                        if visible_count > 0 {
                            let i = match list_state.selected() {
                                Some(i) => {
                                    if i >= visible_count - 1 {
                                        0
                                    } else {
                                        i + 1
                                    }
                                }
                                None => 0,
                            };
                            list_state.select(Some(i));
                        }
                    }
                    KeyCode::Char('g') => {
                        if pending_g == Some('g') {
                            let visible_count = messages.iter().filter(|m| m.status != Status::Completed).count();
                            if visible_count > 0 {
                                list_state.select(Some(0));
                            }
                            pending_g = None;
                        } else {
                            pending_g = Some('g');
                        }
                    }
                    KeyCode::Char('G') => {
                        if pending_g == Some('G') {
                            let visible_count = messages.iter().filter(|m| m.status != Status::Completed).count();
                            if visible_count > 0 {
                                list_state.select(Some(visible_count - 1));
                            }
                            pending_g = None;
                        } else {
                            pending_g = Some('G');
                        }
                    }
                    _ => {
                        pending_g = None;
                    }
                }
            }
        }

        if last_poll.is_none_or(|t| t.elapsed() >= poll_interval) {
            last_poll = Some(Instant::now());

            for (channel_id, channel_name) in &channels {
                if let Ok(resp) = client.conversations_history(channel_id, time_window)
                    && let Some(msgs) = resp.messages
                {
                    for msg in msgs.iter().rev() {
                        let status = determine_status(msg, &config.reactions);

                        if let Some(&idx) = seen.get(&msg.ts) {
                            messages[idx].status = status;
                        } else {
                            for reaction in &config.reactions.backlog {
                                let _ = client.reactions_add(channel_id, &msg.ts, reaction);
                            }

                            let user_id = msg.user.as_deref().unwrap_or("unknown");
                            let display_name = client.resolve_user(user_id);
                            let text = msg.text.as_deref().unwrap_or("").to_string();

                            seen.insert(msg.ts.clone(), messages.len());
                            messages.push(TrackedMessage {
                                channel_name: channel_name.clone(),
                                display_name,
                                text,
                                status,
                            });
                        }
                    }
                }
            }
        }

        let command_buf_snapshot = command_buf.clone();
        terminal
            .draw(|frame| {
                let area = frame.area();
                let in_command_mode = command_buf_snapshot.is_some();

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

                let (cmd_block_area, list_area, status_area) = if in_command_mode {
                    (Some(chunks[0]), chunks[1], chunks[2])
                } else {
                    (None, chunks[0], chunks[1])
                };

                if let Some(cmd_area) = cmd_block_area {
                    let buf = command_buf_snapshot.as_deref().unwrap_or("");
                    let cmd_paragraph = Paragraph::new(Line::from(vec![Span::raw(":"), Span::raw(buf)])).block(
                        Block::default()
                            .title(" command ")
                            .borders(Borders::ALL)
                            .padding(Padding::new(1, 1, 0, 0)),
                    );
                    frame.render_widget(cmd_paragraph, cmd_area);
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

                let channel_list: String = channels.iter().map(|(_, name)| format!("#{}", name)).collect::<Vec<_>>().join(", ");

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

                frame.render_stateful_widget(list, list_area, &mut list_state);

                let status_line = if in_command_mode { Paragraph::new("") } else { Paragraph::new("") };
                frame.render_widget(status_line, status_area);
            })
            .expect("failed to draw");
    }

    disable_raw_mode().expect("failed to disable raw mode");
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen).expect("failed to leave alternate screen");
}
