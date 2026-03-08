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
pub struct Message {
    pub ts: String,
    pub thread_ts: Option<String>,
    pub user: Option<String>,
    pub text: Option<String>,
    #[serde(rename = "type")]
    pub msg_type: Option<String>,
    pub subtype: Option<String>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
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
pub struct SearchMessagesResponse {
    pub ok: bool,
    pub messages: Option<SearchMessages>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SearchMessages {
    pub matches: Option<Vec<SearchMatch>>,
    pub total: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SearchMatch {
    pub ts: String,
    pub text: Option<String>,
    pub user: Option<String>,
    pub permalink: Option<String>,
    pub channel: Option<SearchChannel>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SearchChannel {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ReactionsGetResponse {
    pub ok: bool,
    pub message: Option<ReactionsGetMessage>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ReactionsGetMessage {
    #[serde(default)]
    pub reactions: Vec<Reaction>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct UsergroupsListResponse {
    pub ok: bool,
    pub usergroups: Option<Vec<Usergroup>>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Usergroup {
    pub id: String,
    pub handle: String,
    #[serde(default)]
    pub users: Vec<String>,
}
