use reqwest::blocking::Client;
use reqwest::header::{CONTENT_TYPE, COOKIE};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct AuthTestResponse {
    pub ok: bool,
    pub url: Option<String>,
    pub team: Option<String>,
    pub user: Option<String>,
    pub team_id: Option<String>,
    pub user_id: Option<String>,
    pub is_enterprise_install: Option<bool>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ConversationsListResponse {
    pub ok: bool,
    pub channels: Option<Vec<Channel>>,
    pub response_metadata: Option<ResponseMetadata>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Channel {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ResponseMetadata {
    pub next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct UsersListResponse {
    pub ok: bool,
    pub members: Option<Vec<User>>,
    pub response_metadata: Option<ResponseMetadata>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct User {
    pub id: String,
    pub name: String,
    pub profile: Option<UserProfile>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct UserProfile {
    pub display_name: Option<String>,
    pub real_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ConversationsHistoryResponse {
    pub ok: bool,
    pub messages: Option<Vec<Message>>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Reaction {
    pub name: String,
    pub users: Vec<String>,
    pub count: u32,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Message {
    pub ts: String,
    pub user: Option<String>,
    pub text: Option<String>,
    #[serde(rename = "type")]
    pub msg_type: Option<String>,
    pub subtype: Option<String>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
}

impl Message {
    pub fn has_reaction(&self, name: &str) -> bool {
        self.reactions.iter().any(|r| r.name == name)
    }

    pub fn has_any_reaction(&self, names: &[String]) -> bool {
        names.iter().any(|n| self.has_reaction(n))
    }

    pub fn timestamp(&self) -> String {
        if let Some(dot) = self.ts.find('.')
            && let Ok(secs) = self.ts[..dot].parse::<u64>()
        {
            return format_unix_timestamp(secs);
        }
        self.ts.clone()
    }
}

fn format_unix_timestamp(secs: u64) -> String {
    // Simple UTC formatting without external crates
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Days since 1970-01-01
    let (year, month, day) = days_to_ymd(days);

    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, hours, minutes, seconds)
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    // Civil days to Y-M-D (algorithm from Howard Hinnant)
    days += 719468;
    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

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

    /// Resolves channel names to (id, name) pairs. Returns an error listing any names that couldn't be found.
    pub fn resolve_channels(&self, names: &[String]) -> Result<Vec<(String, String)>, String> {
        let name_to_id = self.fetch_all_channels()?;

        let mut resolved = Vec::new();
        let mut missing = Vec::new();

        for name in names {
            if let Some(id) = name_to_id.get(name.as_str()) {
                resolved.push((id.clone(), name.clone()));
            } else {
                missing.push(name.as_str());
            }
        }

        if !missing.is_empty() {
            return Err(format!("Could not find channels: {}", missing.join(", ")));
        }

        Ok(resolved)
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

    pub fn reactions_add(&self, channel: &str, timestamp: &str, name: &str) -> Result<(), String> {
        let url = format!("{}/api/reactions.add", self.workspace_url);

        let response = self
            .client
            .post(&url)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(COOKIE, format!("d={}", self.xoxd))
            .body(format!("token={}&channel={}&timestamp={}&name={}", self.xoxc, channel, timestamp, name))
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        #[derive(Deserialize)]
        struct Resp {
            ok: bool,
            error: Option<String>,
        }

        let resp: Resp = response.json().map_err(|e| format!("Failed to parse response: {}", e))?;

        if !resp.ok {
            let err = resp.error.unwrap_or_else(|| "unknown error".to_string());
            if err != "already_reacted" {
                return Err(format!("reactions.add failed: {}", err));
            }
        }

        Ok(())
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
