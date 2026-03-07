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

enum TrackResult {
    Restart,
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
    user_id: String,
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
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        client: SlackClient,
        config: Config,
        all_channels: Vec<(String, String)>,
        team_id: String,
        team_name: String,
        user_id: String,
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
            user_id,
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

        let default_source = MessageSource::Search(format!("<@{}>", self.user_id));

        while let TrackResult::Restart = self.track(default_source.clone()) {}
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

    fn poll_messages(&self, source: &MessageSource, messages: &mut Vec<TrackedMessage>, seen: &mut HashMap<String, usize>) {
        let (new_msgs, _) = fetch_messages(&self.client, source, self.past, &seen.keys().cloned().collect(), &self.config.header.hide);
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
        let mut last_poll: Option<Instant>;
        let mut list_state = ListState::default();
        let mut pending_g: Option<char> = None;
        let mut count_buf: u32 = 0;
        let (tx, rx) = mpsc::channel::<(u64, Vec<TrackedMessage>, Vec<String>)>();
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
                            self.handle_config_command(&cmd);
                            let channel_arg = cmd.strip_prefix("c ").or_else(|| cmd.strip_prefix("channel "));
                            if let Some(name) = channel_arg
                                && let Some(ch) = self.find_channel(name)
                            {
                                source = MessageSource::Channels(vec![ch]);
                                messages.clear();
                                seen.clear();
                                last_poll = None;
                                list_state = ListState::default();
                                poll_generation += 1;
                                poll_in_flight = false;
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
                                    let search_queries: Vec<String> = handles
                                        .iter()
                                        .map(|name| match self.client.find_user_id(name) {
                                            Some(id) => format!("<@{}>", id),
                                            None => format!("@{}", name),
                                        })
                                        .collect();
                                    source = MessageSource::Search(search_queries.join(" "));
                                    messages.clear();
                                    seen.clear();
                                    last_poll = None;
                                    list_state = ListState::default();
                                    poll_generation += 1;
                                    poll_in_flight = false;
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
                            return TrackResult::Restart;
                        }
                        _ => {
                            pending_g = None;
                            count_buf = 0;
                        }
                    }
                }
            }

            // Receive results from background poll
            if let Ok((generation, new_msgs, hide_ts)) = rx.try_recv() {
                poll_in_flight = false;
                if generation == poll_generation {
                    // Remove messages that now have hidden reactions
                    if !hide_ts.is_empty() {
                        messages.retain(|m| !hide_ts.contains(&m.ts));
                        seen.clear();
                        for (i, m) in messages.iter().enumerate() {
                            seen.insert(m.ts.clone(), i);
                        }
                        // Fix selection if it's now out of bounds
                        if let Some(sel) = list_state.selected()
                            && sel >= messages.len()
                        {
                            list_state.select(if messages.is_empty() { None } else { Some(messages.len() - 1) });
                        }
                    }
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
                let hide = self.config.header.hide.clone();
                let tx = tx.clone();
                std::thread::spawn(move || {
                    let (results, hide_ts) = fetch_messages(&client, &source_clone, past, &seen_keys, &hide);
                    let _ = tx.send((generation, results, hide_ts));
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

fn has_hidden_reaction(reactions: &[crate::slack::Reaction], hide: &[String]) -> bool {
    if hide.is_empty() {
        return false;
    }
    reactions.iter().any(|r| hide.iter().any(|h| h == &r.name))
}

fn fetch_messages(
    client: &SlackClient,
    source: &MessageSource,
    past: Duration,
    seen_keys: &HashSet<String>,
    hide: &[String],
) -> (Vec<TrackedMessage>, Vec<String>) {
    let mut results = Vec::new();
    let mut hide_ts = Vec::new();
    match source {
        MessageSource::Channels(channels) => {
            for (channel_id, channel_name) in channels {
                if let Ok(resp) = client.conversations_history(channel_id, past)
                    && let Some(msgs) = resp.messages
                {
                    for msg in msgs.iter().rev() {
                        let dominated = has_hidden_reaction(&msg.reactions, hide);
                        if seen_keys.contains(&msg.ts) {
                            // Already shown — check if it should now be hidden
                            if dominated {
                                hide_ts.push(msg.ts.clone());
                            }
                        } else if !dominated {
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
                    let (channel_id, channel_name) = match &m.channel {
                        Some(ch) => (ch.id.clone(), ch.name.clone()),
                        None => ("unknown".to_string(), "unknown".to_string()),
                    };
                    if !hide.is_empty()
                        && let Ok(rr) = client.reactions_get(&channel_id, &m.ts)
                        && let Some(msg) = &rr.message
                        && has_hidden_reaction(&msg.reactions, hide)
                    {
                        if seen_keys.contains(&m.ts) {
                            hide_ts.push(m.ts.clone());
                        }
                        continue;
                    }
                    if seen_keys.contains(&m.ts) {
                        continue;
                    }
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
    (results, hide_ts)
}
