use serde::Deserialize;
use serde_json::Value;

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
    #[serde(default)]
    pub blocks: Vec<Value>,
    #[serde(default)]
    pub attachments: Vec<Value>,
}

impl SearchModulesMessage {
    /// Return the top-level `text` if non-empty, otherwise concatenate text
    /// extracted from blocks (section, header, and context blocks),
    /// falling back to attachment blocks.
    pub fn effective_text(&self) -> String {
        if let Some(ref t) = self.text
            && !t.is_empty()
        {
            return t.clone();
        }
        let from_blocks = Self::text_from_blocks(&self.blocks);
        if !from_blocks.is_empty() {
            return from_blocks;
        }
        // Fall back to attachments
        for attachment in &self.attachments {
            if let Some(blocks) = attachment.get("blocks").and_then(|v| v.as_array()) {
                let text = Self::text_from_blocks(blocks);
                if !text.is_empty() {
                    return text;
                }
            }
        }
        String::new()
    }

    fn text_from_blocks(blocks: &[Value]) -> String {
        let mut parts: Vec<String> = Vec::new();
        for block in blocks {
            let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match block_type {
                "section" | "header" => {
                    if let Some(text) = block.get("text").and_then(|v| v.get("text")).and_then(|v| v.as_str()) {
                        parts.push(text.to_string());
                    }
                    if let Some(fields) = block.get("fields").and_then(|v| v.as_array()) {
                        for field in fields {
                            if let Some(text) = field.get("text").and_then(|v| v.as_str()) {
                                parts.push(text.to_string());
                            }
                        }
                    }
                }
                "context" => {
                    if let Some(elements) = block.get("elements").and_then(|v| v.as_array()) {
                        for elem in elements {
                            if let Some(text) = elem.get("text").and_then(|v| v.as_str()) {
                                parts.push(text.to_string());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        parts.join("\n")
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SearchChannel {
    pub id: String,
    pub name: String,
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
    pub name: String,
}
