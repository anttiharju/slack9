use crate::config::Config;
use crate::input;
use crate::model::{self, TrackedMessage};
use crate::slack::SlackClient;
use crate::view;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::ListState;
use std::collections::HashMap;
use std::io;
use std::time::{Duration, Instant};

enum SelectResult {
    Channel(String, String),
    Quit,
}

enum TrackResult {
    BackToSelect,
    Quit,
}

pub struct App {
    client: SlackClient,
    config: Config,
    all_channels: Vec<(String, String)>,
    workspace_url_for_links: String,
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    command_buf: Option<String>,
    time_window: Duration,
    poll_interval: Duration,
}

impl App {
    pub fn new(
        client: SlackClient,
        config: Config,
        all_channels: Vec<(String, String)>,
        workspace_url_for_links: String,
        time_window: Duration,
        poll_interval: Duration,
    ) -> Self {
        enable_raw_mode().expect("failed to enable raw mode");
        let mut stdout = io::stdout();
        crossterm::execute!(stdout, EnterAlternateScreen).expect("failed to enter alternate screen");
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend).expect("failed to create terminal");

        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic| {
            let _ = disable_raw_mode();
            let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
            original_hook(panic);
        }));

        Self {
            client,
            config,
            all_channels,
            workspace_url_for_links,
            terminal,
            command_buf: None,
            time_window,
            poll_interval,
        }
    }

    pub fn run(mut self) {
        // Show splash screen for 1 second
        let splash_start = Instant::now();
        while splash_start.elapsed() < Duration::from_secs(1) {
            self.terminal
                .draw(|frame| {
                    view::splash::render(frame);
                })
                .expect("failed to draw splash");
            if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                let _ = event::read();
            }
        }

        loop {
            let selected = match self.select_channel() {
                SelectResult::Channel(id, name) => (id, name),
                SelectResult::Quit => break,
            };

            match self.track_messages(selected) {
                TrackResult::Quit => break,
                TrackResult::BackToSelect => continue,
            }
        }

        disable_raw_mode().expect("failed to disable raw mode");
        crossterm::execute!(self.terminal.backend_mut(), LeaveAlternateScreen).expect("failed to leave alternate screen");
    }

    fn resolve_mentions(&self, text: &str) -> String {
        let mut result = text.to_string();
        while let Some(start) = result.find("<@") {
            if let Some(end) = result[start..].find('>') {
                let user_id = &result[start + 2..start + end];
                let name = self.client.resolve_user(user_id);
                result.replace_range(start..start + end + 1, &format!("@{}", name));
            } else {
                break;
            }
        }
        result
    }

    fn find_channel(&self, name: &str) -> Option<(String, String)> {
        let name = name.trim().trim_start_matches('#');
        self.all_channels
            .iter()
            .find(|(_, n)| n == name)
            .or_else(|| {
                let matches: Vec<_> = self.all_channels.iter().filter(|(_, n)| n.starts_with(name)).collect();
                if matches.len() == 1 { Some(matches[0]) } else { None }
            })
            .cloned()
    }

    fn select_channel(&mut self) -> SelectResult {
        let mut list_state = ListState::default();
        if !self.all_channels.is_empty() {
            list_state.select(Some(0));
        }
        let mut filter = String::new();
        let mut filter_editing = false;

        loop {
            // Compute filtered channels for input handling
            let filtered_channels: Vec<(usize, &(String, String))> = if filter.is_empty() {
                self.all_channels.iter().enumerate().collect()
            } else {
                let q = filter.to_lowercase();
                self.all_channels
                    .iter()
                    .enumerate()
                    .filter(|(_, (_, name))| name.to_lowercase().contains(&q))
                    .collect()
            };
            let filtered_count = filtered_channels.len();

            if event::poll(Duration::from_millis(100)).unwrap_or(false)
                && let Ok(Event::Key(key)) = event::read()
                && key.kind == KeyEventKind::Press
            {
                if let Some(ref mut buf) = self.command_buf {
                    match key.code {
                        KeyCode::Enter => {
                            let cmd = buf.trim().to_string();
                            self.command_buf = None;
                            if cmd == "q" || cmd == "q!" {
                                return SelectResult::Quit;
                            }
                            let channel_arg = cmd.strip_prefix("c ").or_else(|| cmd.strip_prefix("channel "));
                            if let Some(name) = channel_arg
                                && let Some(ch) = self.find_channel(name)
                            {
                                return SelectResult::Channel(ch.0, ch.1);
                            }
                        }
                        KeyCode::Esc | KeyCode::Char('\x03') => {
                            self.command_buf = None;
                        }
                        KeyCode::Backspace => {
                            buf.pop();
                            if buf.is_empty() {
                                self.command_buf = None;
                            }
                        }
                        KeyCode::Char(c) => {
                            buf.push(c);
                        }
                        KeyCode::Tab => {
                            input::tab_complete_channel(buf, &self.all_channels);
                        }
                        _ => {}
                    }
                } else if filter_editing {
                    match key.code {
                        KeyCode::Enter => {
                            filter_editing = false;
                            // Reset selection to first item in filtered list
                            if filtered_count > 0 {
                                list_state.select(Some(0));
                            } else {
                                list_state.select(None);
                            }
                        }
                        KeyCode::Esc | KeyCode::Char('\x03') => {
                            filter.clear();
                            filter_editing = false;
                            // Reset selection
                            if !self.all_channels.is_empty() {
                                list_state.select(Some(0));
                            }
                        }
                        KeyCode::Backspace => {
                            filter.pop();
                            if filter.is_empty() {
                                filter_editing = false;
                                if !self.all_channels.is_empty() {
                                    list_state.select(Some(0));
                                }
                            } else {
                                // Recompute filtered count and clamp selection
                                let q = filter.to_lowercase();
                                let new_count = self.all_channels.iter().filter(|(_, name)| name.to_lowercase().contains(&q)).count();
                                if new_count > 0 {
                                    let sel = list_state.selected().unwrap_or(0).min(new_count - 1);
                                    list_state.select(Some(sel));
                                } else {
                                    list_state.select(None);
                                }
                            }
                        }
                        KeyCode::Char(c) => {
                            filter.push(c);
                            // Recompute filtered count and clamp selection
                            let q = filter.to_lowercase();
                            let new_count = self.all_channels.iter().filter(|(_, name)| name.to_lowercase().contains(&q)).count();
                            if new_count > 0 {
                                let sel = list_state.selected().unwrap_or(0).min(new_count - 1);
                                list_state.select(Some(sel));
                            } else {
                                list_state.select(None);
                            }
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            if let Some(i) = list_state.selected()
                                && i > 0
                            {
                                list_state.select(Some(i - 1));
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if let Some(i) = list_state.selected()
                                && i + 1 < filtered_count
                            {
                                list_state.select(Some(i + 1));
                            }
                        }
                        KeyCode::Char('g') => {
                            if filtered_count > 0 {
                                list_state.select(Some(0));
                            }
                        }
                        KeyCode::Char('G') => {
                            if filtered_count > 0 {
                                list_state.select(Some(filtered_count - 1));
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(sel) = list_state.selected()
                                && sel < filtered_count
                            {
                                let (_, ch) = filtered_channels[sel];
                                return SelectResult::Channel(ch.0.clone(), ch.1.clone());
                            }
                        }
                        KeyCode::Char('q') => {
                            return SelectResult::Quit;
                        }
                        KeyCode::Char(':') => {
                            self.command_buf = Some(String::new());
                        }
                        KeyCode::Char('/') => {
                            filter_editing = true;
                        }
                        KeyCode::Esc => {
                            if !filter.is_empty() {
                                filter.clear();
                                if !self.all_channels.is_empty() {
                                    list_state.select(Some(0));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            let command_buf_snapshot = self.command_buf.clone();
            let all_channels = &self.all_channels;
            let filter_snap = filter.clone();
            let fe = filter_editing;
            self.terminal
                .draw(|frame| {
                    let area = frame.area();
                    view::channel_select::render(
                        frame,
                        area,
                        command_buf_snapshot.as_deref(),
                        fe,
                        &filter_snap,
                        all_channels,
                        &mut list_state,
                    );
                })
                .expect("failed to draw");
        }
    }

    fn track_messages(&mut self, initial_channel: (String, String)) -> TrackResult {
        let mut channels: Vec<(String, String)> = vec![initial_channel];
        let mut messages: Vec<TrackedMessage> = Vec::new();
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut last_poll: Option<Instant> = None;
        let mut list_state = ListState::default();
        let mut pending_g: Option<char> = None;
        let mut count_buf: u32 = 0;
        let mut filter = String::new();
        let mut filter_editing = false;

        loop {
            // Compute visible (filtered, non-completed) count for navigation
            let q = filter.to_lowercase();
            let visible_count = messages
                .iter()
                .filter(|m| m.status != model::Status::Completed)
                .filter(|m| {
                    if q.is_empty() {
                        return true;
                    }
                    m.text.to_lowercase().contains(&q) || m.display_name.to_lowercase().contains(&q) || m.channel_name.to_lowercase().contains(&q)
                })
                .count();

            if event::poll(Duration::from_millis(100)).unwrap_or(false)
                && let Ok(Event::Key(key)) = event::read()
                && key.kind == KeyEventKind::Press
            {
                if let Some(ref mut buf) = self.command_buf {
                    match key.code {
                        KeyCode::Enter => {
                            let cmd = buf.trim().to_string();
                            self.command_buf = None;
                            if cmd == "q" || cmd == "q!" {
                                return TrackResult::Quit;
                            }
                            if cmd == "c" || cmd == "channel" {
                                return TrackResult::BackToSelect;
                            }
                            let channel_arg = cmd.strip_prefix("c ").or_else(|| cmd.strip_prefix("channel "));
                            if let Some(name) = channel_arg
                                && let Some(ch) = self.find_channel(name)
                            {
                                channels = vec![ch];
                                messages.clear();
                                seen.clear();
                                last_poll = None;
                                list_state = ListState::default();
                                filter.clear();
                                filter_editing = false;
                            }
                        }
                        KeyCode::Esc | KeyCode::Char('\x03') => {
                            self.command_buf = None;
                        }
                        KeyCode::Backspace => {
                            buf.pop();
                            if buf.is_empty() {
                                self.command_buf = None;
                            }
                        }
                        KeyCode::Char(c) => {
                            buf.push(c);
                        }
                        KeyCode::Tab => {
                            input::tab_complete_channel(buf, &self.all_channels);
                        }
                        _ => {}
                    }
                } else if filter_editing {
                    match key.code {
                        KeyCode::Enter => {
                            filter_editing = false;
                            if visible_count > 0 {
                                let sel = list_state.selected().unwrap_or(0).min(visible_count - 1);
                                list_state.select(Some(sel));
                            } else {
                                list_state.select(None);
                            }
                        }
                        KeyCode::Esc | KeyCode::Char('\x03') => {
                            filter.clear();
                            filter_editing = false;
                            let total_visible = messages.iter().filter(|m| m.status != model::Status::Completed).count();
                            if total_visible > 0 {
                                list_state.select(Some(0));
                            } else {
                                list_state.select(None);
                            }
                        }
                        KeyCode::Backspace => {
                            filter.pop();
                            if filter.is_empty() {
                                filter_editing = false;
                                let total_visible = messages.iter().filter(|m| m.status != model::Status::Completed).count();
                                if total_visible > 0 {
                                    list_state.select(Some(0));
                                } else {
                                    list_state.select(None);
                                }
                            } else {
                                // Clamp selection
                                let fq = filter.to_lowercase();
                                let new_count = messages
                                    .iter()
                                    .filter(|m| m.status != model::Status::Completed)
                                    .filter(|m| {
                                        m.text.to_lowercase().contains(&fq)
                                            || m.display_name.to_lowercase().contains(&fq)
                                            || m.channel_name.to_lowercase().contains(&fq)
                                    })
                                    .count();
                                if new_count > 0 {
                                    let sel = list_state.selected().unwrap_or(0).min(new_count - 1);
                                    list_state.select(Some(sel));
                                } else {
                                    list_state.select(None);
                                }
                            }
                        }
                        KeyCode::Char(c) => {
                            filter.push(c);
                            // Clamp selection
                            let fq = filter.to_lowercase();
                            let new_count = messages
                                .iter()
                                .filter(|m| m.status != model::Status::Completed)
                                .filter(|m| {
                                    m.text.to_lowercase().contains(&fq)
                                        || m.display_name.to_lowercase().contains(&fq)
                                        || m.channel_name.to_lowercase().contains(&fq)
                                })
                                .count();
                            if new_count > 0 {
                                let sel = list_state.selected().unwrap_or(0).min(new_count - 1);
                                list_state.select(Some(sel));
                            } else {
                                list_state.select(None);
                            }
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char(c @ '0'..='9') => {
                            if count_buf > 0 || c != '0' {
                                count_buf = count_buf.saturating_mul(10).saturating_add(c as u32 - '0' as u32);
                            }
                        }
                        KeyCode::Char(':') => {
                            pending_g = None;
                            count_buf = 0;
                            self.command_buf = Some(String::new());
                        }
                        KeyCode::Char('/') => {
                            pending_g = None;
                            count_buf = 0;
                            filter_editing = true;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            pending_g = None;
                            let repeat = if count_buf == 0 { 1 } else { count_buf as usize };
                            count_buf = 0;
                            if visible_count > 0 {
                                let current = list_state.selected().unwrap_or(0);
                                let i = if repeat >= visible_count { 0 } else { current.saturating_sub(repeat) };
                                list_state.select(Some(i));
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            pending_g = None;
                            let repeat = if count_buf == 0 { 1 } else { count_buf as usize };
                            count_buf = 0;
                            if visible_count > 0 {
                                let current = list_state.selected().unwrap_or(0);
                                let i = if current + repeat >= visible_count {
                                    visible_count - 1
                                } else {
                                    current + repeat
                                };
                                list_state.select(Some(i));
                            }
                        }
                        KeyCode::Char('g') => {
                            count_buf = 0;
                            if pending_g == Some('g') {
                                if visible_count > 0 {
                                    list_state.select(Some(0));
                                }
                                pending_g = None;
                            } else {
                                pending_g = Some('g');
                            }
                        }
                        KeyCode::Char('G') => {
                            count_buf = 0;
                            if pending_g == Some('G') {
                                if visible_count > 0 {
                                    list_state.select(Some(visible_count - 1));
                                }
                                pending_g = None;
                            } else {
                                pending_g = Some('G');
                            }
                        }
                        KeyCode::Enter => {
                            pending_g = None;
                            count_buf = 0;
                            if let Some(selected) = list_state.selected() {
                                let visible: Vec<&TrackedMessage> = messages
                                    .iter()
                                    .filter(|m| m.status != model::Status::Completed)
                                    .filter(|m| {
                                        if q.is_empty() {
                                            return true;
                                        }
                                        m.text.to_lowercase().contains(&q)
                                            || m.display_name.to_lowercase().contains(&q)
                                            || m.channel_name.to_lowercase().contains(&q)
                                    })
                                    .collect();
                                if let Some(msg) = visible.get(selected) {
                                    let ts_for_url = msg.ts.replace('.', "");
                                    let url = format!("{}/archives/{}/p{}", self.workspace_url_for_links, msg.channel_id, ts_for_url);
                                    let _ = std::process::Command::new("open").arg(&url).spawn();
                                }
                            }
                        }
                        KeyCode::Esc => {
                            if !filter.is_empty() {
                                filter.clear();
                                let total_visible = messages.iter().filter(|m| m.status != model::Status::Completed).count();
                                if total_visible > 0 {
                                    list_state.select(Some(0));
                                }
                            } else {
                                return TrackResult::BackToSelect;
                            }
                        }
                        _ => {
                            pending_g = None;
                            count_buf = 0;
                        }
                    }
                }
            }

            if last_poll.is_none_or(|t| t.elapsed() >= self.poll_interval) {
                last_poll = Some(Instant::now());

                for (channel_id, channel_name) in &channels {
                    if let Ok(resp) = self.client.conversations_history(channel_id, self.time_window)
                        && let Some(msgs) = resp.messages
                    {
                        for msg in msgs.iter().rev() {
                            let status = model::determine_status(msg, &self.config.reactions);

                            if let Some(&idx) = seen.get(&msg.ts) {
                                messages[idx].status = status;
                            } else {
                                let user_id = msg.user.as_deref().unwrap_or("unknown");
                                let display_name = self.client.resolve_user(user_id);
                                let raw_text = msg.text.as_deref().unwrap_or("").to_string();
                                let text = self.resolve_mentions(&raw_text);

                                seen.insert(msg.ts.clone(), messages.len());
                                messages.push(TrackedMessage {
                                    channel_id: channel_id.clone(),
                                    channel_name: channel_name.clone(),
                                    ts: msg.ts.clone(),
                                    display_name,
                                    text,
                                    status,
                                });
                            }
                        }
                    }
                }
            }

            let command_buf_snapshot = self.command_buf.clone();
            let all_channels = &self.all_channels;
            let config = &self.config;
            let filter_snap = filter.clone();
            let fe = filter_editing;
            let pi = self.poll_interval;
            let pe = last_poll.map(|t| t.elapsed());
            self.terminal
                .draw(|frame| {
                    let area = frame.area();
                    view::message_list::render(
                        frame,
                        area,
                        command_buf_snapshot.as_deref(),
                        fe,
                        &filter_snap,
                        all_channels,
                        &messages,
                        &channels,
                        config,
                        &mut list_state,
                        pi,
                        pe,
                    );
                })
                .expect("failed to draw");
        }
    }
}
