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
    Search(Vec<String>),
    Quit,
}

enum TrackResult {
    BackToSelect,
    Search(Vec<String>),
    Quit,
}

pub struct App {
    client: SlackClient,
    config: Config,
    all_channels: Vec<(String, String)>,
    user_names: Vec<String>,
    team_id: String,
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
        team_id: String,
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
            user_names: Vec::new(),
            team_id,
            terminal,
            command_buf: None,
            time_window,
            poll_interval,
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

            match self.track_messages_filtered(vec![selected], None) {
                TrackResult::Quit => break,
                TrackResult::BackToSelect => continue,
                TrackResult::Search(handles) => match self.track_search(&handles) {
                    TrackResult::Quit => break,
                    TrackResult::BackToSelect | TrackResult::Search(_) => continue,
                },
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
            let user_names = &self.user_names;
            let poll_label = self.config.poll_interval.clone();
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
                        user_names,
                        &mut list_state,
                        &poll_label,
                    );
                })
                .expect("failed to draw");
        }
    }

    fn track_messages_filtered(&mut self, initial_channels: Vec<(String, String)>, initial_filter: Option<String>) -> TrackResult {
        let mut channels: Vec<(String, String)> = initial_channels;
        let mut messages: Vec<TrackedMessage> = Vec::new();
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut last_poll: Option<Instant> = None;
        let mut list_state = ListState::default();
        let mut pending_g: Option<char> = None;
        let mut count_buf: u32 = 0;
        let mut filter = initial_filter.unwrap_or_default();
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
                                    let url = format!("slack://channel?team={}&id={}&message={}", self.team_id, msg.channel_id, msg.ts);
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

            // Select first item if nothing is selected yet
            if list_state.selected().is_none() && !messages.is_empty() {
                list_state.select(Some(0));
            }

            let command_buf_snapshot = self.command_buf.clone();
            let all_channels = &self.all_channels;
            let user_names = &self.user_names;
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
                        user_names,
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

    fn track_search(&mut self, display_names: &[String]) -> TrackResult {
        let search_queries: Vec<String> = display_names
            .iter()
            .map(|name| match self.client.find_user_id(name) {
                Some(id) => format!("<@{}>", id),
                None => format!("@{}", name),
            })
            .collect();
        let search_query = search_queries.join(" ");

        let mut messages: Vec<TrackedMessage> = Vec::new();
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut last_poll: Option<Instant> = None;
        let mut list_state = ListState::default();
        let mut pending_g: Option<char> = None;
        let mut count_buf: u32 = 0;

        let search_label = display_names.iter().map(|n| format!("@{}", n)).collect::<Vec<_>>().join(" ");

        loop {
            let visible_count = messages.iter().filter(|m| m.status != model::Status::Completed).count();

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
                            if let Some(selected) = list_state.selected() {
                                let visible: Vec<&TrackedMessage> = messages.iter().filter(|m| m.status != model::Status::Completed).collect();
                                if let Some(msg) = visible.get(selected) {
                                    let url = format!("slack://channel?team={}&id={}&message={}", self.team_id, msg.channel_id, msg.ts);
                                    let _ = std::process::Command::new("open").arg(&url).spawn();
                                }
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

            if last_poll.is_none_or(|t| t.elapsed() >= self.poll_interval) {
                last_poll = Some(Instant::now());

                if let Ok(resp) = self.client.search_messages(&search_query)
                    && let Some(search_msgs) = resp.messages
                    && let Some(matches) = search_msgs.matches
                {
                    for m in &matches {
                        let status = model::determine_status_from_reactions(&m.reactions, &self.config.reactions);

                        if let Some(&idx) = seen.get(&m.ts) {
                            messages[idx].status = status;
                        } else {
                            let (channel_id, channel_name) = match &m.channel {
                                Some(ch) => (ch.id.clone(), ch.name.clone()),
                                None => ("unknown".to_string(), "unknown".to_string()),
                            };
                            let user_id_str = m.user.as_deref().unwrap_or("unknown");
                            let resolved_name = self.client.resolve_user(user_id_str);
                            let raw_text = m.text.as_deref().unwrap_or("").to_string();
                            let text = self.resolve_mentions(&raw_text);

                            seen.insert(m.ts.clone(), messages.len());
                            messages.push(TrackedMessage {
                                channel_id,
                                channel_name,
                                ts: m.ts.clone(),
                                display_name: resolved_name,
                                text,
                                status,
                            });
                        }
                    }
                }
            }

            // Select first item if nothing is selected yet
            if list_state.selected().is_none() && !messages.is_empty() {
                list_state.select(Some(0));
            }

            let command_buf_snapshot = self.command_buf.clone();
            let all_channels = &self.all_channels;
            let user_names = &self.user_names;
            let config = &self.config;
            let pi = self.poll_interval;
            let pe = last_poll.map(|t| t.elapsed());
            let ping_label_snap = search_label.clone();
            self.terminal
                .draw(|frame| {
                    let area = frame.area();
                    view::message_list::render(
                        frame,
                        area,
                        command_buf_snapshot.as_deref(),
                        false,
                        &ping_label_snap,
                        all_channels,
                        user_names,
                        &messages,
                        &[],
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
