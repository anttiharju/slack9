use super::types::*;
use reqwest::blocking::Client;
use reqwest::header::{CONTENT_TYPE, COOKIE};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct SlackClient {
    client: Client,
    workspace_url: String,
    xoxd: String,
    xoxc: String,
    users: HashMap<String, String>,
}

impl SlackClient {
    pub fn new(workspace_url: String, xoxd: String, xoxc: String) -> Self {
        Self {
            client: Client::new(),
            workspace_url: workspace_url.trim_end_matches('/').to_string(),
            xoxd,
            xoxc,
            users: HashMap::new(),
        }
    }

    pub fn resolve_user(&self, user_id: &str) -> String {
        self.users.get(user_id).cloned().unwrap_or_else(|| user_id.to_string())
    }

    pub fn load_users(&mut self) -> Result<(), String> {
        self.users = self.fetch_all_users()?;
        Ok(())
    }

    fn fetch_all_users(&self) -> Result<HashMap<String, String>, String> {
        let mut map = HashMap::new();
        let mut cursor = String::new();

        loop {
            let url = format!("{}/api/users.list", self.workspace_url);

            let mut body = format!("token={}&limit=1000", self.xoxc);
            if !cursor.is_empty() {
                body.push_str(&format!("&cursor={}", cursor));
            }

            let response = self
                .client
                .post(&url)
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(COOKIE, format!("d={}", self.xoxd))
                .body(body)
                .send()
                .map_err(|e| format!("Request failed: {}", e))?;

            let resp: UsersListResponse = response.json().map_err(|e| format!("Failed to parse response: {}", e))?;

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
        let url = format!("{}/api/auth.test", self.workspace_url);

        let response = self
            .client
            .post(&url)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(COOKIE, format!("d={}", self.xoxd))
            .body(format!("token={}", self.xoxc))
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        response
            .json::<AuthTestResponse>()
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
            let url = format!("{}/api/conversations.list", self.workspace_url);

            let mut body = format!("token={}&types=public_channel,private_channel&limit=1000", self.xoxc);
            if !cursor.is_empty() {
                body.push_str(&format!("&cursor={}", cursor));
            }

            let response = self
                .client
                .post(&url)
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(COOKIE, format!("d={}", self.xoxd))
                .body(body)
                .send()
                .map_err(|e| format!("Request failed: {}", e))?;

            let resp: ConversationsListResponse = response.json().map_err(|e| format!("Failed to parse response: {}", e))?;

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
        let oldest = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64() - time_window.as_secs_f64();

        let url = format!("{}/api/conversations.history", self.workspace_url);

        let response = self
            .client
            .post(&url)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(COOKIE, format!("d={}", self.xoxd))
            .body(format!("token={}&channel={}&oldest={}", self.xoxc, channel, oldest))
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        response
            .json::<ConversationsHistoryResponse>()
            .map_err(|e| format!("Failed to parse response: {}", e))
    }
}
