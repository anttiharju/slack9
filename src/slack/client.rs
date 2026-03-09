use super::api_log::ApiLog;
use super::types::*;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct SlackClient {
    agent: ureq::Agent,
    workspace_url: String,
    xoxd: String,
    xoxc: String,
    users: HashMap<String, String>,
    usergroups: HashMap<String, String>,
    api_log: Option<ApiLog>,
}

impl SlackClient {
    pub fn new(workspace_url: String, xoxd: String, xoxc: String, debug: bool) -> Self {
        assert!(workspace_url.starts_with("https://"), "workspace URL must start with https://");
        let api_log = if debug {
            Some(ApiLog::new().expect("failed to initialize API log"))
        } else {
            None
        };
        Self {
            agent: ureq::Agent::new_with_defaults(),
            workspace_url: workspace_url.trim_end_matches('/').to_string(),
            xoxd,
            xoxc,
            users: HashMap::new(),
            usergroups: HashMap::new(),
            api_log,
        }
    }

    pub fn resolve_user(&self, user_id: &str) -> String {
        self.users.get(user_id).cloned().unwrap_or_else(|| user_id.to_string())
    }

    fn log_api(&self, method: &str) {
        if let Some(log) = &self.api_log {
            log.log(method);
        }
    }

    fn post_form(&self, url: &str, body: &str) -> Result<ureq::Body, String> {
        let response = self
            .agent
            .post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Cookie", &format!("d={}", self.xoxd))
            .send(body.as_bytes())
            .map_err(|e| format!("Request failed: {}", e))?;
        Ok(response.into_body())
    }

    /// Find user display name by handle (matches against display names and user IDs).
    /// Returns the display name if found.
    pub fn find_user_display_name(&self, handle: &str) -> Option<String> {
        let handle = handle.trim_start_matches('@');
        // Exact match on display name
        if let Some(name) = self.users.values().find(|name| name.eq_ignore_ascii_case(handle)) {
            return Some(name.clone());
        }
        // Exact match on user ID
        if let Some(name) = self.users.get(handle) {
            return Some(name.clone());
        }
        // Prefix match on display name
        let handle_lower = handle.to_lowercase();
        let matches: Vec<_> = self
            .users
            .values()
            .filter(|name| name.to_lowercase().starts_with(&handle_lower))
            .collect();
        if matches.len() == 1 {
            return Some(matches[0].clone());
        }
        None
    }

    pub fn load_users(&mut self) -> Result<(), String> {
        self.users = self.fetch_all_users()?;
        self.usergroups = self.fetch_usergroups().unwrap_or_default();
        Ok(())
    }

    fn fetch_all_users(&self) -> Result<HashMap<String, String>, String> {
        let mut map = HashMap::new();
        let mut cursor = String::new();

        loop {
            self.log_api("users.list");
            let url = format!("{}/api/users.list", self.workspace_url);

            let mut body = format!("token={}&limit=1000", self.xoxc);
            if !cursor.is_empty() {
                body.push_str(&format!("&cursor={}", cursor));
            }

            let resp: UsersListResponse = self
                .post_form(&url, &body)?
                .read_json()
                .map_err(|e| format!("Failed to parse response: {}", e))?;

            if !resp.ok {
                return Err(format!(
                    "users.list failed: {}",
                    resp.error.unwrap_or_else(|| "unknown error".to_string())
                ));
            }

            if let Some(members) = resp.members {
                for u in members {
                    let display = u
                        .profile
                        .as_ref()
                        .and_then(|p| p.display_name.as_deref().filter(|s| !s.is_empty()))
                        .or(u.profile.as_ref().and_then(|p| p.real_name.as_deref().filter(|s| !s.is_empty())))
                        .unwrap_or(&u.name);
                    map.insert(u.id, display.to_string());
                }
            }

            match resp.response_metadata.and_then(|m| m.next_cursor) {
                Some(c) if !c.is_empty() => cursor = c,
                _ => break,
            }
        }

        Ok(map)
    }

    pub fn auth_test(&self) -> Result<AuthTestResponse, String> {
        self.log_api("auth.test");
        let url = format!("{}/api/auth.test", self.workspace_url);

        self.post_form(&url, &format!("token={}", self.xoxc))?
            .read_json::<AuthTestResponse>()
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Lists all accessible channels as (id, name) pairs sorted by name.
    pub fn list_channels(&self) -> Result<Vec<(String, String)>, String> {
        let map = self.fetch_all_channels()?;
        let mut channels: Vec<(String, String)> = map.into_iter().map(|(name, id)| (id, name)).collect();
        channels.sort_by(|a, b| a.1.cmp(&b.1));
        Ok(channels)
    }

    fn fetch_all_channels(&self) -> Result<HashMap<String, String>, String> {
        let mut map = HashMap::new();
        let mut cursor = String::new();

        loop {
            self.log_api("conversations.list");
            let url = format!("{}/api/conversations.list", self.workspace_url);

            let mut body = format!("token={}&types=public_channel,private_channel&limit=1000", self.xoxc);
            if !cursor.is_empty() {
                body.push_str(&format!("&cursor={}", cursor));
            }

            let resp: ConversationsListResponse = self
                .post_form(&url, &body)?
                .read_json()
                .map_err(|e| format!("Failed to parse response: {}", e))?;

            if !resp.ok {
                return Err(format!(
                    "conversations.list failed: {}",
                    resp.error.unwrap_or_else(|| "unknown error".to_string())
                ));
            }

            if let Some(channels) = resp.channels {
                for ch in channels {
                    map.insert(ch.name, ch.id);
                }
            }

            match resp.response_metadata.and_then(|m| m.next_cursor) {
                Some(c) if !c.is_empty() => cursor = c,
                _ => break,
            }
        }

        Ok(map)
    }

    pub fn conversations_history(&self, channel: &str, time_window: Duration) -> Result<ConversationsHistoryResponse, String> {
        self.log_api("conversations.history");
        let oldest = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64() - time_window.as_secs_f64();

        let url = format!("{}/api/conversations.history", self.workspace_url);

        self.post_form(&url, &format!("token={}&channel={}&oldest={}", self.xoxc, channel, oldest))?
            .read_json::<ConversationsHistoryResponse>()
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    fn fetch_usergroups(&self) -> Result<HashMap<String, String>, String> {
        self.log_api("usergroups.list");
        let url = format!("{}/api/usergroups.list", self.workspace_url);

        let resp: UsergroupsListResponse = self
            .post_form(&url, &format!("token={}&include_users=true", self.xoxc))?
            .read_json()
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        if !resp.ok {
            return Err(format!(
                "usergroups.list failed: {}",
                resp.error.unwrap_or_else(|| "unknown error".to_string())
            ));
        }

        let mut map = HashMap::new();
        if let Some(groups) = resp.usergroups {
            for g in groups {
                map.insert(g.handle, g.id);
            }
        }
        Ok(map)
    }

    /// Find user group ID by handle.
    pub fn find_usergroup_id(&self, handle: &str) -> Option<&String> {
        let handle = handle.trim_start_matches('@');
        self.usergroups.get(handle).or_else(|| {
            let handle_lower = handle.to_lowercase();
            let matches: Vec<_> = self
                .usergroups
                .iter()
                .filter(|(h, _)| h.to_lowercase().starts_with(&handle_lower))
                .collect();
            if matches.len() == 1 { Some(matches[0].1) } else { None }
        })
    }

    /// Find user ID by display name (reverse lookup).
    pub fn find_user_id(&self, display_name: &str) -> Option<String> {
        let display_name = display_name.trim_start_matches('@');
        self.users
            .iter()
            .find(|(_, name)| name.eq_ignore_ascii_case(display_name))
            .map(|(id, _)| id.clone())
    }

    /// Search messages across all channels.
    pub fn search_messages(&self, query: &str) -> Result<SearchMessagesResponse, String> {
        self.log_api("search.messages");
        let url = format!("{}/api/search.messages", self.workspace_url);

        self.post_form(
            &url,
            &format!("token={}&query={}&count=100&sort=timestamp&sort_dir=desc", self.xoxc, query),
        )?
        .read_json::<SearchMessagesResponse>()
        .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Fetch reactions for a specific message.
    pub fn reactions_get(&self, channel: &str, timestamp: &str) -> Result<ReactionsGetResponse, String> {
        self.log_api("reactions.get");
        let url = format!("{}/api/reactions.get", self.workspace_url);

        self.post_form(&url, &format!("token={}&channel={}&timestamp={}", self.xoxc, channel, timestamp))?
            .read_json::<ReactionsGetResponse>()
            .map_err(|e| format!("Failed to parse response: {}", e))
    }
}
