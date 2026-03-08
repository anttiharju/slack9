use crate::config::{self, Config};
use crate::input;
use crate::model::TrackedMessage;
use crate::slack::SlackClient;
use crate::view;
use crate::view::header::wave_fraction;

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
use std::time::{SystemTime, UNIX_EPOCH};

enum TrackResult {
    Restart,
    Quit,
}

#[derive(Clone)]
enum MessageSource {
    Channels(Vec<(String, String)>),
    Search(Vec<String>),
}

pub struct App {
    client: Arc<SlackClient>,
    config: Config,
    all_channels: Vec<(String, String)>,
    channels_loaded: bool,
    user_names: Vec<String>,
    team_id: String,
    team_name: String,
    user_id: String,
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    command_buf: Option<String>,
    command_error: bool,
    past: Duration,
    poll: Duration,
    active_reactions: HashSet<String>,
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
    }
}

impl App {
    #[allow(clippy::too_many_arguments)]
    pub fn new(client: SlackClient, config: Config, team_id: String, team_name: String, user_id: String, past: Duration, poll: Duration) -> Self {
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

        let active_reactions = config.reactions.keys().cloned().collect();

        Self {
            client: Arc::new(client),
            config,
            all_channels: Vec::new(),
            channels_loaded: false,
            user_names: Vec::new(),
            team_id,
            team_name,
            user_id,
            terminal,
            command_buf: None,
            command_error: false,
            past,
            poll,
            active_reactions,
        }
    }

    pub fn run(mut self) {
        let mut names = self.client.user_names();
        names.extend(self.client.usergroup_handles());
        names.sort();
        names.dedup();
        self.user_names = names;

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

        let default_source = self.resolve_initial_source();

        while let TrackResult::Restart = self.track(default_source.clone()) {}
    }

    fn find_channel(&mut self, name: &str) -> Option<(String, String)> {
        self.ensure_channels_loaded();
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

    /// Handle `:time <val>` and `:poll <val>` commands.
    /// Returns true if the command was recognized and handled.
    fn handle_config_command(&mut self, cmd: &str) -> bool {
        if let Some(val) = cmd.strip_prefix("time ") {
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

    fn save_view(&mut self, view: &str) {
        self.config.state.view = Some(view.to_string());
        let _ = config::save(&self.config);
    }

    fn ensure_channels_loaded(&mut self) {
        if !self.channels_loaded {
            self.channels_loaded = true;
            match self.client.list_channels() {
                Ok(channels) => self.all_channels = channels,
                Err(e) => eprintln!("Error listing channels: {}", e),
            }
        }
    }

    fn resolve_initial_source(&mut self) -> MessageSource {
        if let Some(view) = self.config.state.view.clone() {
            if let Some(rest) = view.strip_prefix("search ") {
                let queries = self.resolve_search_handles(rest);
                if !queries.is_empty() {
                    return MessageSource::Search(queries);
                }
            } else if let Some(rest) = view.strip_prefix("channel ")
                && let Some(ch) = self.find_channel(rest)
            {
                return MessageSource::Channels(vec![ch]);
            }
        }
        MessageSource::Search(vec![format!("<@{}>", self.user_id)])
    }

    /// Resolve search handles (users and user groups) into `<@USER_ID>` queries.
    fn resolve_search_handles(&self, input: &str) -> Vec<String> {
        let mut queries = Vec::new();
        for h in input.split_whitespace() {
            let h = h.trim_start_matches('@');
            // Try user first
            if let Some(name) = self.client.find_user_display_name(h)
                && let Some(id) = self.client.find_user_id(&name) {
                    queries.push(format!("<@{}>", id));
                    continue;
                }
            // Try user group
            if let Some(member_ids) = self.client.find_usergroup_member_ids(h) {
                for id in member_ids {
                    let q = format!("<@{}>", id);
                    if !queries.contains(&q) {
                        queries.push(q);
                    }
                }
            }
        }
        queries
    }

    /// Collect the display-name / group-handle tokens for saving to config.
    fn resolve_search_save_names(&self, input: &str) -> Vec<String> {
        let mut names = Vec::new();
        for h in input.split_whitespace() {
            let h = h.trim_start_matches('@');
            if let Some(name) = self.client.find_user_display_name(h) {
                names.push(name);
            } else if self.client.find_usergroup_member_ids(h).is_some() {
                names.push(h.to_string());
            }
        }
        names
    }

    fn active_show_emojis(&self) -> Vec<String> {
        let active = &self.active_reactions;
        self.config
            .reactions
            .iter()
            .filter(|(name, _)| active.contains(*name))
            .map(|(_, emoji)| emoji.clone())
            .collect()
    }

    fn all_configured_emojis(&self) -> Vec<String> {
        self.config.reactions.values().cloned().collect()
    }

    fn poll_messages(&self, source: &MessageSource, messages: &mut Vec<TrackedMessage>, seen: &mut HashMap<String, usize>) {
        let new_msgs = fetch_messages(&self.client, source, self.past);
        messages.clear();
        seen.clear();
        for msg in new_msgs {
            seen.insert(msg.ts.clone(), messages.len());
            messages.push(msg);
        }
    }

    fn track(&mut self, mut source: MessageSource) -> TrackResult {
        let mut messages: Vec<TrackedMessage> = Vec::new();
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut last_poll: Option<Instant>;
        let mut list_state = ListState::default();
        let mut pending_g: Option<char> = None;
        let mut pending_o = false;
        let (tx, rx) = mpsc::channel::<(u64, Vec<TrackedMessage>)>();
        let mut poll_generation: u64 = 0;
        let mut poll_in_flight = false;
        let mut poll_fired_this_cycle = false;
        let mut drain_start: Option<Instant> = None;

        // Do first poll synchronously so there's data on the first frame
        self.poll_messages(&source, &mut messages, &mut seen);
        last_poll = Some(Instant::now());

        loop {
            // Client-side filtering: show messages that have no configured reaction or at least one active reaction
            let show_emojis = self.active_show_emojis();
            let all_emojis = self.all_configured_emojis();
            let visible_count = messages
                .iter()
                .filter(|m| {
                    let configured: Vec<&String> = m.reaction_emojis.iter().filter(|e| all_emojis.contains(e)).collect();
                    configured.is_empty() || configured.iter().any(|e| show_emojis.contains(e))
                })
                .count();

            if event::poll(Duration::from_millis(100)).unwrap_or(false)
                && let Ok(Event::Key(key)) = event::read()
                && key.kind == KeyEventKind::Press
            {
                let mut needs_tab_complete = false;
                if let Some(ref mut buf) = self.command_buf {
                    match key.code {
                        KeyCode::Enter => {
                            let cmd = buf.trim().to_string();
                            let mut handled = false;
                            if cmd == "q" || cmd == "q!" {
                                return TrackResult::Quit;
                            }
                            if self.handle_config_command(&cmd) {
                                handled = true;
                            }
                            if let Some(rest) = cmd.strip_prefix("reaction ") {
                                let mut parts = rest.split_whitespace();
                                if let (Some(name), Some(emoji)) = (parts.next(), parts.next()) {
                                    self.config.reactions.insert(name.to_string(), emoji.to_string());
                                    let _ = config::save(&self.config);
                                    handled = true;
                                }
                            }
                            let channel_arg = cmd.strip_prefix("c ").or_else(|| cmd.strip_prefix("channel "));
                            if let Some(name) = channel_arg
                                && let Some(ch) = self.find_channel(name)
                            {
                                self.save_view(&format!("channel {}", ch.1));
                                source = MessageSource::Channels(vec![ch]);
                                messages.clear();
                                seen.clear();
                                last_poll = None;
                                list_state = ListState::default();
                                poll_generation += 1;
                                poll_in_flight = false;
                                poll_fired_this_cycle = false;
                                drain_start = None;
                                handled = true;
                            }
                            if let Some(rest) = cmd.strip_prefix("search ") {
                                let search_queries = self.resolve_search_handles(rest);
                                if !search_queries.is_empty() {
                                    let save_names = self.resolve_search_save_names(rest);
                                    self.save_view(&format!("search {}", save_names.join(" ")));
                                    source = MessageSource::Search(search_queries);
                                    messages.clear();
                                    seen.clear();
                                    last_poll = None;
                                    list_state = ListState::default();
                                    poll_generation += 1;
                                    poll_in_flight = false;
                                    poll_fired_this_cycle = false;
                                    drain_start = None;
                                    handled = true;
                                }
                            }
                            if handled {
                                self.command_buf = None;
                                self.command_error = false;
                            } else {
                                self.command_error = true;
                            }
                        }
                        KeyCode::Esc | KeyCode::Char('\x03') => {
                            self.command_buf = None;
                            self.command_error = false;
                        }
                        KeyCode::Backspace => {
                            buf.pop();
                            if buf.is_empty() {
                                self.command_buf = None;
                            }
                            self.command_error = false;
                        }
                        KeyCode::Char(c) => {
                            if c == ' ' && !buf.contains(' ') {
                                let abbrev = buf.as_str();
                                const COMMANDS: &[&str] = &["channel", "poll", "reaction", "search", "time"];
                                let matches: Vec<&&str> = COMMANDS.iter().filter(|cmd| cmd.starts_with(abbrev)).collect();
                                if matches.len() == 1 {
                                    buf.clear();
                                    buf.push_str(matches[0]);
                                }
                            }
                            buf.push(c);
                            self.command_error = false;
                        }
                        KeyCode::Tab => {
                            needs_tab_complete = true;
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char(c @ '1'..='9') => {
                            pending_g = None;
                            let idx = (c as u32 - '1' as u32) as usize;
                            let reaction_names: Vec<String> = self.config.reactions.keys().cloned().collect();
                            if idx < reaction_names.len() {
                                if pending_o {
                                    self.active_reactions.clear();
                                    self.active_reactions.insert(reaction_names[idx].clone());
                                } else {
                                    let name = &reaction_names[idx];
                                    if self.active_reactions.contains(name) {
                                        self.active_reactions.remove(name);
                                    } else {
                                        self.active_reactions.insert(name.clone());
                                    }
                                }
                            }
                            pending_o = false;
                        }
                        KeyCode::Char('o') => {
                            pending_g = None;
                            pending_o = true;
                        }
                        KeyCode::Char(':') => {
                            pending_g = None;
                            pending_o = false;
                            self.command_buf = Some(String::new());
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            pending_g = None;
                            pending_o = false;
                            if visible_count > 0 {
                                let current = list_state.selected().unwrap_or(0);
                                list_state.select(Some(current.saturating_sub(1)));
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            pending_g = None;
                            pending_o = false;
                            if visible_count > 0 {
                                let current = list_state.selected().unwrap_or(0);
                                let i = if current + 1 >= visible_count { visible_count - 1 } else { current + 1 };
                                list_state.select(Some(i));
                            }
                        }
                        KeyCode::Char('g') => {
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
                            pending_o = false;
                            if let Some(selected) = list_state.selected() {
                                let visible: Vec<&TrackedMessage> = messages
                                    .iter()
                                    .filter(|m| {
                                        let configured: Vec<&String> = m.reaction_emojis.iter().filter(|e| all_emojis.contains(e)).collect();
                                        configured.is_empty() || configured.iter().any(|e| show_emojis.contains(e))
                                    })
                                    .collect();
                                if let Some(msg) = visible.get(selected) {
                                    let url = format!("slack://channel?team={}&id={}&message={}", self.team_id, msg.channel_id, msg.ts);
                                    let _ = std::process::Command::new("open").arg(&url).spawn();
                                }
                            }
                        }
                        KeyCode::Esc => {
                            return TrackResult::Restart;
                        }
                        _ => {
                            pending_g = None;
                            pending_o = false;
                        }
                    }
                }
                if needs_tab_complete {
                    self.ensure_channels_loaded();
                    if let Some(ref mut buf) = self.command_buf {
                        input::tab_complete_channel(buf, &self.all_channels, &self.user_names);
                    }
                }
            }

            // Receive results from background poll
            if let Ok((generation, new_msgs)) = rx.try_recv() {
                poll_in_flight = false;
                if generation == poll_generation {
                    drain_start = Some(Instant::now());
                    // Replace messages with latest API results
                    messages.clear();
                    seen.clear();
                    for msg in new_msgs {
                        seen.insert(msg.ts.clone(), messages.len());
                        messages.push(msg);
                    }
                }
            }

            // Spawn background poll at the end of the wave phase
            let wf = wave_fraction();
            let wave_secs = self.poll.as_secs_f64() * wf;
            if !poll_in_flight && !poll_fired_this_cycle && last_poll.is_none_or(|t| t.elapsed().as_secs_f64() >= wave_secs) {
                poll_in_flight = true;
                poll_fired_this_cycle = true;
                let client = Arc::clone(&self.client);
                let source_clone = source.clone();
                let past = self.past;
                let generation = poll_generation;
                let tx = tx.clone();
                std::thread::spawn(move || {
                    let results = fetch_messages(&client, &source_clone, past);
                    let _ = tx.send((generation, results));
                });
            }

            // Reset animation cycle after drain completes
            let drain_secs = self.poll.as_secs_f64() * (1.0 - wf);
            if let Some(ds) = drain_start {
                if ds.elapsed().as_secs_f64() >= drain_secs {
                    last_poll = Some(Instant::now());
                    poll_fired_this_cycle = false;
                    drain_start = None;
                }
            } else if !poll_in_flight && last_poll.is_none() {
                // Initial cycle start
                last_poll = Some(Instant::now());
            }

            if list_state.selected().is_none() && visible_count > 0 {
                list_state.select(Some(0));
            }
            // Clamp selection to visible range
            if let Some(sel) = list_state.selected() {
                if visible_count == 0 {
                    list_state.select(None);
                } else if sel >= visible_count {
                    list_state.select(Some(visible_count - 1));
                }
            }

            // Build filtered visible messages for rendering
            let oldest = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64() - self.past.as_secs_f64();
            let visible_messages: Vec<&TrackedMessage> = messages
                .iter()
                .filter(|m| {
                    let msg_ts: f64 = m.ts.parse().unwrap_or(0.0);
                    if msg_ts < oldest {
                        return false;
                    }
                    let configured: Vec<&String> = m.reaction_emojis.iter().filter(|e| all_emojis.contains(e)).collect();
                    configured.is_empty() || configured.iter().any(|e| show_emojis.contains(e))
                })
                .collect();

            let command_buf_snapshot = self.command_buf.clone();
            let command_error = self.command_error;
            let all_channels = &self.all_channels;
            let user_names = &self.user_names;
            let config = &self.config;
            let poll_state = view::header::PollState {
                interval: self.poll,
                elapsed: last_poll.map(|t| t.elapsed()),
                in_flight: poll_in_flight,
                drain_elapsed: drain_start.map(|t| t.elapsed()),
            };
            let team_name = &self.team_name;
            let tracked_channels: &[(String, String)] = match &source {
                MessageSource::Channels(channels) => channels,
                MessageSource::Search(_) => &[],
            };
            let active_reactions = &self.active_reactions;
            self.terminal
                .draw(|frame| {
                    let area = frame.area();
                    view::message_list::render(
                        frame,
                        area,
                        command_buf_snapshot.as_deref(),
                        command_error,
                        all_channels,
                        user_names,
                        &visible_messages,
                        tracked_channels,
                        config,
                        &mut list_state,
                        &poll_state,
                        team_name,
                        active_reactions,
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
            let inner = &result[start + 2..start + end];
            let user_id = inner.split('|').next().unwrap_or(inner);
            let name = client.resolve_user(user_id);
            result.replace_range(start..start + end + 1, &format!("@{}", name));
        } else {
            break;
        }
    }
    result
}

fn fetch_messages(client: &SlackClient, source: &MessageSource, past: Duration) -> Vec<TrackedMessage> {
    let mut results = Vec::new();
    match source {
        MessageSource::Channels(channels) => {
            for (channel_id, channel_name) in channels {
                if let Ok(resp) = client.conversations_history(channel_id, past)
                    && let Some(msgs) = resp.messages
                {
                    for msg in msgs.iter().rev() {
                        let reaction_emojis: Vec<String> = msg.reactions.iter().map(|r| r.name.clone()).collect();
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
                            reaction_emojis,
                        });
                    }
                }
            }
        }
        MessageSource::Search(queries) => {
            let oldest = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64() - past.as_secs_f64();
            let mut all_matches = Vec::new();
            let mut seen_ts = std::collections::HashSet::new();
            for query in queries {
                if let Ok(resp) = client.search_messages(query)
                    && let Some(search_msgs) = resp.messages
                    && let Some(matches) = search_msgs.matches
                {
                    for m in matches {
                        let msg_ts: f64 = m.ts.parse().unwrap_or(0.0);
                        if msg_ts >= oldest && seen_ts.insert(m.ts.clone()) {
                            all_matches.push(m);
                        }
                    }
                }
            }
            for m in &all_matches {
                let (channel_id, channel_name) = match &m.channel {
                    Some(ch) => (ch.id.clone(), ch.name.clone()),
                    None => ("unknown".to_string(), "unknown".to_string()),
                };
                let reaction_emojis: Vec<String> = if let Ok(rr) = client.reactions_get(&channel_id, &m.ts)
                    && let Some(msg) = &rr.message
                {
                    msg.reactions.iter().map(|r| r.name.clone()).collect()
                } else {
                    Vec::new()
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
                    reaction_emojis,
                });
            }
        }
    }
    results
}
