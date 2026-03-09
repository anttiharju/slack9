use serde::Deserialize;

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
pub struct Reaction {
    pub name: String,
    pub count: Option<u32>,
    pub users: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SearchModulesMessagesResponse {
    pub ok: bool,
    pub items: Option<Vec<SearchModulesItem>>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SearchModulesItem {
    pub channel: Option<SearchChannel>,
    pub messages: Option<Vec<SearchModulesMessage>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SearchModulesMessage {
    pub ts: String,
    pub user: Option<String>,
    pub text: Option<String>,
    pub permalink: Option<String>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    pub thread_ts: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SearchChannel {
    pub id: String,
    pub name: String,
}
