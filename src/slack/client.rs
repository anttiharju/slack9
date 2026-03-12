use super::api_log::ApiLog;
use super::types::*;
use std::collections::HashMap;

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

    pub fn resolve_usergroup(&self, usergroup_id: &str) -> String {
        self.usergroups.get(usergroup_id).cloned().unwrap_or_else(|| usergroup_id.to_string())
    }

    fn log_api(&self, method: &str) {
        if let Some(log) = &self.api_log {
            log.log(method);
        }
    }

    fn log_body(&self, body: &str) {
        if let Some(log) = &self.api_log {
            log.log_body(body);
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

    pub fn load_users(&mut self) -> Result<(), String> {
        self.users = self.fetch_all_users()?;
        Ok(())
    }

    pub fn load_usergroups(&mut self) -> Result<(), String> {
        self.usergroups = self.fetch_usergroups()?;
        Ok(())
    }

    fn fetch_usergroups(&self) -> Result<HashMap<String, String>, String> {
        self.log_api("usergroups.list");
        let url = format!("{}/api/usergroups.list", self.workspace_url);
        let body = format!("token={}", self.xoxc);

        let resp: UsergroupsListResponse = self
            .post_form(&url, &body)?
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
                let label = if g.handle.is_empty() { g.name } else { g.handle };
                map.insert(g.id, label);
            }
        }
        Ok(map)
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

    /// Search messages across all channels using the search.modules.messages API.
    pub fn search_modules_messages(&self, query: &str) -> Result<SearchModulesMessagesResponse, String> {
        self.log_api("search.modules.messages");
        let url = format!("{}/api/search.modules.messages?_x_gantry=true", self.workspace_url);

        let raw = self
            .post_form(
                &url,
                &format!(
                    "token={}&query={}&count=100&sort=timestamp&sort_dir=desc&module=messages&extra_message_data=1",
                    self.xoxc, query
                ),
            )?
            .read_to_string()
            .map_err(|e| format!("Failed to read response: {}", e))?;

        self.log_body(&raw);

        serde_json::from_str::<SearchModulesMessagesResponse>(&raw).map_err(|e| format!("Failed to parse response: {}", e))
    }
}
