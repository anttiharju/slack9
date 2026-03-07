use crate::config::{self, Config};
use crate::input;
use crate::model::TrackedMessage;
use crate::slack::SlackClient;
use crate::view;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

enum SelectResult {
    Channel(String, String),
    Search(Vec<String>),
    Quit,
}

enum TrackResult {
    BackToSelect,
    Search(Vec<String>),
    Quit,
}

#[derive(Clone)]
enum MessageSource {
    Channels(Vec<(String, String)>),
    Search(String),
}

pub struct App {
    client: Arc<SlackClient>,
    config: Config,
    all_channels: Vec<(String, String)>,
    user_names: Vec<String>,
    team_id: String,
    team_name: String,
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    command_buf: Option<String>,
    past: Duration,
    poll: Duration,
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
    }
}

impl App {
    pub fn new(
        client: SlackClient,
        config: Config,
        all_channels: Vec<(String, String)>,
        team_id: String,
        team_name: String,
        past: Duration,
        poll: Duration,
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
            client: Arc::new(client),
            config,
            all_channels,
            user_names: Vec::new(),
            team_id,
            team_name,
            terminal,
            command_buf: None,
            past,
            poll,
        }
    }

    pub fn run(mut self) {
        self.user_names = self.client.user_names();

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
                SelectResult::Search(handles) => match self.track_search(&handles) {
                    TrackResult::Quit => break,
                    TrackResult::BackToSelect | TrackResult::Search(_) => continue,
                },
                SelectResult::Quit => break,
            };

            match self.track_messages(vec![selected]) {
                TrackResult::Quit => break,
                TrackResult::BackToSelect => continue,
                TrackResult::Search(handles) => match self.track_search(&handles) {
                    TrackResult::Quit => break,
                    TrackResult::BackToSelect | TrackResult::Search(_) => continue,
                },
            }
        }
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

    /// Handle `:past <val>` and `:poll <val>` commands.
    /// Returns true if the command was recognized and handled.
    fn handle_config_command(&mut self, cmd: &str) -> bool {
        if let Some(val) = cmd.strip_prefix("past ") {
            let val = val.trim();
            match config::validate_duration(val) {
                Ok(d) => {
                    self.config.header.past = val.to_string();
                    self.past = d;
                    let _ = config::save(&self.config);
                }
                Err(e) => eprintln!("Invalid past duration: {}", e),
            }
            true
        } else if let Some(val) = cmd.strip_prefix("poll ") {
            let val = val.trim();
            match config::validate_duration(val) {
                Ok(d) => {
                    self.config.header.poll = val.to_string();
                    self.poll = d;
                    let _ = config::save(&self.config);
                }
                Err(e) => eprintln!("Invalid poll duration: {}", e),
            }
            true
        } else {
            false
        }
    }

    fn select_channel(&mut self) -> SelectResult {
        let mut list_state = ListState::default();
        if !self.all_channels.is_empty() {
            list_state.select(Some(0));
        }

        loop {
            let channel_count = self.all_channels.len();

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
                            self.handle_config_command(&cmd);
                            let channel_arg = cmd.strip_prefix("c ").or_else(|| cmd.strip_prefix("channel "));
                            if let Some(name) = channel_arg
                                && let Some(ch) = self.find_channel(name)
                            {
                                return SelectResult::Channel(ch.0, ch.1);
                            }
                            if let Some(rest) = cmd.strip_prefix("search ") {
                                let handles: Vec<String> = rest
                                    .split_whitespace()
                                    .filter_map(|h| {
                                        let h = h.trim_start_matches('@');
                                        self.client.find_user_display_name(h)
                                    })
                                    .collect();
                                if !handles.is_empty() {
                                    return SelectResult::Search(handles);
                                }
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
                            input::tab_complete_channel(buf, &self.all_channels, &self.user_names);
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
                                && i + 1 < channel_count
                            {
                                list_state.select(Some(i + 1));
                            }
                        }
                        KeyCode::Char('g') => {
                            if channel_count > 0 {
                                list_state.select(Some(0));
                            }
                        }
                        KeyCode::Char('G') => {
                            if channel_count > 0 {
                                list_state.select(Some(channel_count - 1));
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(sel) = list_state.selected()
                                && sel < channel_count
                            {
                                let ch = &self.all_channels[sel];
                                return SelectResult::Channel(ch.0.clone(), ch.1.clone());
                            }
                        }
                        KeyCode::Char('q') => {
                            return SelectResult::Quit;
                        }
                        KeyCode::Char(':') => {
                            self.command_buf = Some(String::new());
                        }
                        _ => {}
                    }
                }
            }

            let command_buf_snapshot = self.command_buf.clone();
            let all_channels = &self.all_channels;
            let user_names = &self.user_names;
            let poll_label = self.config.header.poll_label();
            let workspace_label = self.team_name.clone();
            let past_label = self.config.header.past_label();
            self.terminal
                .draw(|frame| {
                    let area = frame.area();
                    view::channel_select::render(
                        frame,
                        area,
                        command_buf_snapshot.as_deref(),
                        all_channels,
                        user_names,
                        &mut list_state,
                        &poll_label,
                        &workspace_label,
                        &past_label,
                    );
                })
                .expect("failed to draw");
        }
    }

    fn track_messages(&mut self, channels: Vec<(String, String)>) -> TrackResult {
        self.track(MessageSource::Channels(channels))
    }

    fn track_search(&mut self, display_names: &[String]) -> TrackResult {
        let search_queries: Vec<String> = display_names
            .iter()
            .map(|name| match self.client.find_user_id(name) {
                Some(id) => format!("<@{}>", id),
                None => format!("@{}", name),
            })
            .collect();
        self.track(MessageSource::Search(search_queries.join(" ")))
    }

    fn poll_messages(&self, source: &MessageSource, messages: &mut Vec<TrackedMessage>, seen: &mut HashMap<String, usize>) {
        let new_msgs = fetch_messages(&self.client, source, self.past, &seen.keys().cloned().collect());
        for msg in new_msgs {
            if !seen.contains_key(&msg.ts) {
                seen.insert(msg.ts.clone(), messages.len());
                messages.push(msg);
            }
        }
    }

    fn track(&mut self, mut source: MessageSource) -> TrackResult {
        let mut messages: Vec<TrackedMessage> = Vec::new();
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut last_poll: Option<Instant> = None;
        let mut list_state = ListState::default();
        let mut pending_g: Option<char> = None;
        let mut count_buf: u32 = 0;
        let (tx, rx) = mpsc::channel::<(u64, Vec<TrackedMessage>)>();
        let mut poll_generation: u64 = 0;
        let mut poll_in_flight = false;

        // Do first poll synchronously so there's data on the first frame
        self.poll_messages(&source, &mut messages, &mut seen);
        last_poll = Some(Instant::now());

        loop {
            let visible_count = messages.len();

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
                            self.handle_config_command(&cmd);
                            if let MessageSource::Channels(ref mut channels) = source {
                                let channel_arg = cmd.strip_prefix("c ").or_else(|| cmd.strip_prefix("channel "));
                                if let Some(name) = channel_arg
                                    && let Some(ch) = self.find_channel(name)
                                {
                                    *channels = vec![ch];
                                    messages.clear();
                                    seen.clear();
                                    last_poll = None;
                                    list_state = ListState::default();
                                    poll_generation += 1;
                                    poll_in_flight = false;
                                }
                            }
                            if let Some(rest) = cmd.strip_prefix("search ") {
                                let handles: Vec<String> = rest
                                    .split_whitespace()
                                    .filter_map(|h| {
                                        let h = h.trim_start_matches('@');
                                        self.client.find_user_display_name(h)
                                    })
                                    .collect();
                                if !handles.is_empty() {
                                    return TrackResult::Search(handles);
                                }
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
                            input::tab_complete_channel(buf, &self.all_channels, &self.user_names);
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
                            if let Some(selected) = list_state.selected()
                                && let Some(msg) = messages.get(selected)
                            {
                                let url = format!("slack://channel?team={}&id={}&message={}", self.team_id, msg.channel_id, msg.ts);
                                let _ = std::process::Command::new("open").arg(&url).spawn();
                            }
                        }
                        KeyCode::Esc => {
                            return TrackResult::BackToSelect;
                        }
                        _ => {
                            pending_g = None;
                            count_buf = 0;
                        }
                    }
                }
            }

            // Receive results from background poll
            if let Ok((generation, new_msgs)) = rx.try_recv() {
                poll_in_flight = false;
                if generation == poll_generation {
                    for msg in new_msgs {
                        if !seen.contains_key(&msg.ts) {
                            seen.insert(msg.ts.clone(), messages.len());
                            messages.push(msg);
                        }
                    }
                }
            }

            // Spawn background poll when timer expires and none in-flight
            if !poll_in_flight && last_poll.is_none_or(|t| t.elapsed() >= self.poll) {
                last_poll = Some(Instant::now());
                poll_in_flight = true;
                let client = Arc::clone(&self.client);
                let source_clone = source.clone();
                let past = self.past;
                let seen_keys: HashSet<String> = seen.keys().cloned().collect();
                let generation = poll_generation;
                let tx = tx.clone();
                std::thread::spawn(move || {
                    let results = fetch_messages(&client, &source_clone, past, &seen_keys);
                    let _ = tx.send((generation, results));
                });
            }

            if list_state.selected().is_none() && !messages.is_empty() {
                list_state.select(Some(0));
            }

            let command_buf_snapshot = self.command_buf.clone();
            let all_channels = &self.all_channels;
            let user_names = &self.user_names;
            let config = &self.config;
            let pi = self.poll;
            let pe = last_poll.map(|t| t.elapsed());
            let team_name = &self.team_name;
            let tracked_channels: &[(String, String)] = match &source {
                MessageSource::Channels(channels) => channels,
                MessageSource::Search(_) => &[],
            };
            self.terminal
                .draw(|frame| {
                    let area = frame.area();
                    view::message_list::render(
                        frame,
                        area,
                        command_buf_snapshot.as_deref(),
                        all_channels,
                        user_names,
                        &messages,
                        tracked_channels,
                        config,
                        &mut list_state,
                        pi,
                        pe,
                        team_name,
                    );
                })
                .expect("failed to draw");
        }
    }
}

fn resolve_mentions(client: &SlackClient, text: &str) -> String {
    let mut result = text.to_string();
    while let Some(start) = result.find("<@") {
        if let Some(end) = result[start..].find('>') {
            let user_id = &result[start + 2..start + end];
            let name = client.resolve_user(user_id);
            result.replace_range(start..start + end + 1, &format!("@{}", name));
        } else {
            break;
        }
    }
    result
}

fn fetch_messages(client: &SlackClient, source: &MessageSource, past: Duration, seen_keys: &HashSet<String>) -> Vec<TrackedMessage> {
    let mut results = Vec::new();
    match source {
        MessageSource::Channels(channels) => {
            for (channel_id, channel_name) in channels {
                if let Ok(resp) = client.conversations_history(channel_id, past)
                    && let Some(msgs) = resp.messages
                {
                    for msg in msgs.iter().rev() {
                        if !seen_keys.contains(&msg.ts) {
                            let user_id = msg.user.as_deref().unwrap_or("unknown");
                            let display_name = client.resolve_user(user_id);
                            let raw_text = msg.text.as_deref().unwrap_or("").to_string();
                            let text = resolve_mentions(client, &raw_text);

                            results.push(TrackedMessage {
                                channel_id: channel_id.clone(),
                                channel_name: channel_name.clone(),
                                ts: msg.ts.clone(),
                                display_name,
                                text,
                            });
                        }
                    }
                }
            }
        }
        MessageSource::Search(query) => {
            if let Ok(resp) = client.search_messages(query)
                && let Some(search_msgs) = resp.messages
                && let Some(matches) = search_msgs.matches
            {
                for m in &matches {
                    if !seen_keys.contains(&m.ts) {
                        let (channel_id, channel_name) = match &m.channel {
                            Some(ch) => (ch.id.clone(), ch.name.clone()),
                            None => ("unknown".to_string(), "unknown".to_string()),
                        };
                        let user_id_str = m.user.as_deref().unwrap_or("unknown");
                        let display_name = client.resolve_user(user_id_str);
                        let raw_text = m.text.as_deref().unwrap_or("").to_string();
                        let text = resolve_mentions(client, &raw_text);

                        results.push(TrackedMessage {
                            channel_id,
                            channel_name,
                            ts: m.ts.clone(),
                            display_name,
                            text,
                        });
                    }
                }
            }
        }
    }
    results
}
