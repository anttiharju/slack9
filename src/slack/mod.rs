use reqwest::blocking::Client;
use reqwest::header::{CONTENT_TYPE, COOKIE};
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

pub fn auth_test(workspace_url: &str, xoxd: &str, xoxc: &str) -> Result<AuthTestResponse, String> {
    let url = format!("{}/api/auth.test", workspace_url.trim_end_matches('/'));

    let client = Client::new();
    let response = client
        .post(&url)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(COOKIE, format!("d={}", xoxd))
        .body(format!("token={}", xoxc))
        .send()
        .map_err(|e| format!("Request failed: {}", e))?;

    response
        .json::<AuthTestResponse>()
        .map_err(|e| format!("Failed to parse response: {}", e))
}
