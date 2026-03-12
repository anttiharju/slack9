use crate::config::{self, Config};
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
    Search(Vec<String>),
}

pub struct App {
    client: Arc<SlackClient>,
    config: Config,
    team_id: String,
    team_name: String,
    user_id: String,
    user_name: String,
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    command_buf: Option<String>,
    command_error: bool,
    past: Duration,
    poll: Duration,
    active_categories: HashSet<String>,
    show_uncategorised: bool,
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
        team_id: String,
        team_name: String,
        user_id: String,
        user_name: String,
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

        let active_categories: HashSet<String> = match &config.state.active_categories {
            Some(saved) => saved.iter().cloned().collect(),
            None => config.categories.keys().cloned().collect(),
        };
        let show_uncategorised = config.state.show_uncategorised;

        Self {
            client: Arc::new(client),
            config,
            team_id,
            team_name,
            user_id,
            user_name,
            terminal,
            command_buf: None,
            command_error: false,
            past,
            poll,
            active_categories,
            show_uncategorised,
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

        let default_source = self.resolve_initial_source();

        while let TrackResult::Restart = self.track(default_source.clone()) {}
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

    fn save_query(&mut self, queries: &[String]) {
        self.config.state.search = Some(queries.to_vec());
        let _ = config::save(&self.config);
    }

    fn save_category_state(&mut self) {
        self.config.state.active_categories = Some(self.active_categories.iter().cloned().collect());
        self.config.state.show_uncategorised = self.show_uncategorised;
        let _ = config::save(&self.config);
    }

    fn resolve_initial_source(&mut self) -> MessageSource {
        let mut queries: Vec<String> = self
            .config
            .state
            .search
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|q| !q.trim().is_empty())
            .collect();

        if self.config.state.user_pings {
            let user_query = format!("<@{}>", self.user_id);
            if !queries.contains(&user_query) {
                queries.push(user_query);
            }
        }

        if queries.is_empty() {
            queries.push(format!("<@{}>", self.user_id));
        }

        MessageSource::Search(queries)
    }

    /// Returns the user_pings query and display name if user_pings is enabled,
    /// used to filter results from that query to only messages containing @user_name.
    fn user_ping_filter(&self) -> Option<(String, String)> {
        if self.config.state.user_pings {
            Some((format!("<@{}>", self.user_id), self.user_name.clone()))
        } else {
            None
        }
    }

    fn active_show_emojis(&self) -> Vec<String> {
        let active = &self.active_categories;
        self.config
            .categories
            .iter()
            .filter(|(name, _)| active.contains(*name))
            .flat_map(|(_, emojis)| emojis.iter().cloned())
            .collect()
    }

    fn all_configured_emojis(&self) -> Vec<String> {
        self.config.categories.values().flatten().cloned().collect()
    }

    fn poll_messages(&self, source: &MessageSource, messages: &mut Vec<TrackedMessage>, seen: &mut HashMap<String, usize>) {
        let user_ping_filter = self.user_ping_filter();
        let new_msgs = fetch_messages(&self.client, source, self.past, user_ping_filter.as_ref());
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
            let show_uncategorised = self.show_uncategorised;
            let visible_count = messages
                .iter()
                .filter(|m| {
                    let configured: Vec<&String> = m.reaction_emojis.iter().filter(|e| all_emojis.contains(e)).collect();
                    if configured.is_empty() {
                        show_uncategorised
                    } else {
                        configured.iter().any(|e| show_emojis.contains(e))
                    }
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
                            let mut handled = false;
                            if cmd == "q" || cmd == "q!" {
                                return TrackResult::Quit;
                            }
                            if self.handle_config_command(&cmd) {
                                handled = true;
                            }

                            if let Some(rest) = cmd.strip_prefix("search ") {
                                let queries: Vec<String> = rest.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                                if !queries.is_empty() {
                                    self.save_query(&queries);
                                    source = MessageSource::Search(queries);
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
                                const COMMANDS: &[&str] = &["poll", "search", "time"];
                                let matches: Vec<&&str> = COMMANDS.iter().filter(|cmd| cmd.starts_with(abbrev)).collect();
                                if matches.len() == 1 {
                                    buf.clear();
                                    buf.push_str(matches[0]);
                                }
                            }
                            buf.push(c);
                            self.command_error = false;
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('0') => {
                            pending_g = None;
                            if pending_o {
                                self.active_categories.clear();
                                self.show_uncategorised = true;
                            } else {
                                self.show_uncategorised = !self.show_uncategorised;
                            }
                            pending_o = false;
                            self.save_category_state();
                        }
                        KeyCode::Char(c @ '1'..='9') => {
                            pending_g = None;
                            let idx = (c as u32 - '1' as u32) as usize;
                            let category_names: Vec<String> = self.config.categories.keys().cloned().collect();
                            if idx < category_names.len() {
                                if pending_o {
                                    self.active_categories.clear();
                                    self.active_categories.insert(category_names[idx].clone());
                                    self.show_uncategorised = false;
                                } else {
                                    let name = &category_names[idx];
                                    if self.active_categories.contains(name) {
                                        self.active_categories.remove(name);
                                    } else {
                                        self.active_categories.insert(name.clone());
                                    }
                                }
                                self.save_category_state();
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
                                        if configured.is_empty() {
                                            show_uncategorised
                                        } else {
                                            configured.iter().any(|e| show_emojis.contains(e))
                                        }
                                    })
                                    .collect();
                                if let Some(msg) = visible.get(selected) {
                                    let link_ts = msg.thread_ts.as_deref().unwrap_or(&msg.ts);
                                    let url = format!("slack://channel?team={}&id={}&message={}", self.team_id, msg.channel_id, link_ts);
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
                let user_ping_filter = self.user_ping_filter();
                std::thread::spawn(move || {
                    let results = fetch_messages(&client, &source_clone, past, user_ping_filter.as_ref());
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
                    if configured.is_empty() {
                        show_uncategorised
                    } else {
                        configured.iter().any(|e| show_emojis.contains(e))
                    }
                })
                .collect();

            let command_buf_snapshot = self.command_buf.clone();
            let command_error = self.command_error;
            let config = &self.config;
            let poll_state = view::header::PollState {
                interval: self.poll,
                elapsed: last_poll.map(|t| t.elapsed()),
                in_flight: poll_in_flight,
                drain_elapsed: drain_start.map(|t| t.elapsed()),
            };
            let team_name = &self.team_name;
            let user_name = &self.user_name;
            let active_categories = &self.active_categories;
            let show_uncategorised = self.show_uncategorised;
            self.terminal
                .draw(|frame| {
                    let area = frame.area();
                    view::message_list::render(
                        frame,
                        area,
                        command_buf_snapshot.as_deref(),
                        command_error,
                        &visible_messages,
                        config,
                        &mut list_state,
                        &poll_state,
                        team_name,
                        user_name,
                        active_categories,
                        show_uncategorised,
                    );
                })
                .expect("failed to draw");
        }
    }
}

fn resolve_mentions(client: &SlackClient, text: &str) -> String {
    let mut result = text.to_string();
    // Resolve user mentions: <@U...>
    while let Some(start) = result.find("<@") {
        if let Some(end) = result[start..].find('>') {
            let inner = &result[start + 2..start + end];
            let user_id = inner.split('|').next().unwrap_or(inner);
            let had_highlight = inner.contains('\u{E000}');
            let user_id_clean = user_id.replace(['\u{E000}', '\u{E001}'], "");
            let name = client.resolve_user(&user_id_clean);
            let replacement = if had_highlight {
                format!("\u{E000}@{}\u{E001}", name)
            } else {
                format!("@{}", name)
            };
            result.replace_range(start..start + end + 1, &replacement);
        } else {
            break;
        }
    }
    // Resolve channel mentions: <#C...> or <#C...|name>
    while let Some(start) = result.find("<#") {
        if let Some(end) = result[start..].find('>') {
            let inner = &result[start + 2..start + end];
            let had_highlight = inner.contains('\u{E000}');
            let clean = inner.replace(['\u{E000}', '\u{E001}'], "");
            let name = if let Some(pipe) = clean.find('|') {
                clean[pipe + 1..].to_string()
            } else {
                client.resolve_channel(&clean)
            };
            let replacement = if had_highlight {
                format!("\u{E000}#{}\u{E001}", name)
            } else {
                format!("#{}", name)
            };
            result.replace_range(start..start + end + 1, &replacement);
        } else {
            break;
        }
    }
    // Resolve usergroup mentions: <!subteam^S...>
    while let Some(start) = result.find("<!subteam^") {
        if let Some(end) = result[start..].find('>') {
            let inner = &result[start + "<!subteam^".len()..start + end];
            let group_id = inner.split('|').next().unwrap_or(inner);
            let had_highlight = inner.contains('\u{E000}');
            let group_id_clean = group_id.replace(['\u{E000}', '\u{E001}'], "");
            let name = client.resolve_usergroup(&group_id_clean);
            let replacement = if had_highlight {
                format!("\u{E000}@{}\u{E001}", name)
            } else {
                format!("@{}", name)
            };
            result.replace_range(start..start + end + 1, &replacement);
        } else {
            break;
        }
    }
    // Resolve bare usergroup IDs: <S08G72CNAA3> (with optional highlight markers)
    loop {
        let start = result.find("<S").or_else(|| result.find("<\u{E000}S"));
        let Some(start) = start else { break };
        let Some(rel_end) = result[start..].find('>') else { break };
        let end = start + rel_end;
        let inner = &result[start + 1..end];
        let had_highlight = inner.contains('\u{E000}');
        let clean = inner.replace(['\u{E000}', '\u{E001}'], "");
        if clean.len() > 1 && clean[1..].chars().all(|c| c.is_ascii_alphanumeric()) {
            let name = client.resolve_usergroup(&clean);
            if name != clean {
                let replacement = if had_highlight {
                    format!("\u{E000}@{}\u{E001}", name)
                } else {
                    format!("@{}", name)
                };
                result.replace_range(start..end + 1, &replacement);
                continue;
            }
        }
        break;
    }
    // Resolve links: <https://...|label> or <https://...>
    let mut i = 0;
    while i < result.len() {
        if let Some(rel_start) = result[i..].find('<') {
            let start = i + rel_start;
            let after = &result[start + 1..];
            let check = after.trim_start_matches('\u{E000}');
            if (check.starts_with("http://") || check.starts_with("https://") || check.starts_with("mailto:"))
                && let Some(rel_end) = result[start..].find('>')
            {
                let end = start + rel_end;
                let inner = &result[start + 1..end];
                let clean = inner.replace(['\u{E000}', '\u{E001}'], "");
                let display = if let Some(pipe) = clean.find('|') {
                    clean[pipe + 1..].to_string()
                } else {
                    clean
                };
                result.replace_range(start..end + 1, &display);
                i = start + display.len();
                continue;
            }
            i = start + 1;
        } else {
            break;
        }
    }
    result
}

fn fetch_messages(client: &SlackClient, source: &MessageSource, past: Duration, user_ping_filter: Option<&(String, String)>) -> Vec<TrackedMessage> {
    let mut results = Vec::new();
    match source {
        MessageSource::Search(queries) => {
            let oldest = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64() - past.as_secs_f64();
            let mut seen_ts = std::collections::HashSet::new();
            for query in queries {
                let is_user_ping_query = user_ping_filter.as_ref().is_some_and(|(q, _)| q == query);
                if let Ok(resp) = client.search_modules_messages(query)
                    && let Some(items) = resp.items
                {
                    for item in items {
                        let (channel_id, channel_name) = match &item.channel {
                            Some(ch) => {
                                let name = if is_user_id(&ch.name) {
                                    client.resolve_user(&ch.name)
                                } else {
                                    ch.name.clone()
                                };
                                (ch.id.clone(), name)
                            }
                            None => ("unknown".to_string(), "unknown".to_string()),
                        };
                        if let Some(messages) = item.messages {
                            for m in messages {
                                let msg_ts: f64 = m.ts.parse().unwrap_or(0.0);
                                if msg_ts < oldest || !seen_ts.insert(m.ts.clone()) {
                                    continue;
                                }
                                let reaction_emojis: Vec<String> = m.reactions.iter().map(|r| r.name.clone()).collect();
                                let user_id_str = m.user.as_deref().unwrap_or("unknown");
                                let display_name = client.resolve_user(user_id_str);
                                let raw_text = m.effective_text();
                                let mut text = resolve_mentions(client, &raw_text).replace('\n', " ");
                                while text.contains("  ") {
                                    text = text.replace("  ", " ");
                                }

                                // For user_pings query, only keep messages that mention @user_name
                                if is_user_ping_query && let Some((_, user_name)) = user_ping_filter {
                                    let at_user = format!("@{}", user_name);
                                    let text_clean = text.replace(['\u{E000}', '\u{E001}'], "");
                                    if !text_clean.contains(&at_user) {
                                        continue;
                                    }
                                }

                                let thread_ts = m.thread_ts.clone().or_else(|| {
                                    m.permalink.as_deref().and_then(|p| {
                                        p.split('?')
                                            .nth(1)
                                            .and_then(|qs| qs.split('&').find_map(|param| param.strip_prefix("thread_ts=").map(String::from)))
                                    })
                                });

                                results.push(TrackedMessage {
                                    channel_id: channel_id.clone(),
                                    channel_name: channel_name.clone(),
                                    ts: m.ts.clone(),
                                    thread_ts,
                                    display_name,
                                    text,
                                    reaction_emojis,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    results.sort_by(|a, b| a.ts.partial_cmp(&b.ts).unwrap_or(std::cmp::Ordering::Equal));
    results
}

/// Check if a string looks like a Slack user ID (e.g. "U05315SPC9Y").
fn is_user_id(s: &str) -> bool {
    s.len() > 1 && s.starts_with('U') && s[1..].chars().all(|c| c.is_ascii_alphanumeric())
}
