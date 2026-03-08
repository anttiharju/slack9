use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_PAST: &str = "24h";
const DEFAULT_POLL: &str = "10s";

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Config {
    #[serde(default, skip_serializing_if = "HeaderConfig::is_default")]
    pub header: HeaderConfig,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub reactions: IndexMap<String, String>,
    #[serde(default, skip_serializing_if = "StateConfig::is_default")]
    pub state: StateConfig,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct StateConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
}

impl StateConfig {
    fn is_default(&self) -> bool {
        self.view.is_none()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HeaderConfig {
    #[serde(default = "default_past", skip_serializing_if = "is_default_past")]
    pub past: String,
    #[serde(default = "default_poll", skip_serializing_if = "is_default_poll")]
    pub poll: String,
}

impl Default for HeaderConfig {
    fn default() -> Self {
        Self {
            past: default_past(),
            poll: default_poll(),
        }
    }
}

fn default_past() -> String {
    DEFAULT_PAST.to_string()
}

fn default_poll() -> String {
    DEFAULT_POLL.to_string()
}

fn is_default_past(v: &str) -> bool {
    v == DEFAULT_PAST
}

fn is_default_poll(v: &str) -> bool {
    v == DEFAULT_POLL
}

impl HeaderConfig {
    fn is_default(&self) -> bool {
        self.past == DEFAULT_PAST && self.poll == DEFAULT_POLL
    }

    pub fn past_duration(&self) -> Result<Duration, String> {
        parse_duration(&self.past)
    }

    pub fn poll_duration(&self) -> Result<Duration, String> {
        parse_duration(&self.poll)
    }

    pub fn past_label(&self) -> String {
        if self.past == DEFAULT_PAST {
            format!("{} (default)", self.past)
        } else {
            self.past.clone()
        }
    }

    pub fn poll_label(&self) -> String {
        if self.poll == DEFAULT_POLL {
            format!("{} (default)", self.poll)
        } else {
            self.poll.clone()
        }
    }

    /// Returns `(command_name, value_label)` pairs for config-backed commands.
    pub fn config_labels(&self) -> Vec<(&str, String)> {
        vec![("poll", self.poll_label()), ("time", self.past_label())]
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Config (~/.config/slack9/config.toml):")?;
        writeln!(f, "  [header]")?;
        writeln!(f, "    past: {}", self.header.past)?;
        writeln!(f, "    poll: {}", self.header.poll)?;
        if !self.reactions.is_empty() {
            writeln!(f, "  [reactions]")?;
            for (name, emoji) in &self.reactions {
                writeln!(f, "    {} = {}", name, emoji)?;
            }
        }
        Ok(())
    }
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty duration string".to_string());
    }

    let (num_str, unit) = if let Some(stripped) = s.strip_suffix("ms") {
        (stripped, "ms")
    } else {
        let split = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        (&s[..split], &s[split..])
    };

    let value: u64 = num_str.parse().map_err(|_| format!("invalid number in duration: '{}'", s))?;

    match unit {
        "s" => Ok(Duration::from_secs(value)),
        "m" => Ok(Duration::from_secs(value * 60)),
        "h" => Ok(Duration::from_secs(value * 60 * 60)),
        "d" => Ok(Duration::from_secs(value * 60 * 60 * 24)),
        "w" => Ok(Duration::from_secs(value * 60 * 60 * 24 * 7)),
        "M" => Ok(Duration::from_secs(value * 60 * 60 * 24 * 30)),
        _ => Err(format!("unknown duration unit '{}' in '{}'. Use s, m, h, d, w, or M", unit, s)),
    }
}

pub fn validate_duration(s: &str) -> Result<Duration, String> {
    parse_duration(s)
}

pub fn load() -> Config {
    let path = match config_path() {
        Ok(p) => p,
        Err(_) => return Config::default(),
    };

    let contents = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Config::default(),
    };

    toml::from_str(&contents).unwrap_or_else(|e| {
        eprintln!("Warning: failed to parse {}: {}", path.display(), e);
        Config::default()
    })
}

pub fn save(config: &Config) -> Result<(), String> {
    let path = config_path()?;
    let contents = toml::to_string_pretty(config).map_err(|e| format!("failed to serialize config: {}", e))?;
    if contents.trim().is_empty() {
        let _ = fs::remove_file(&path);
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("failed to create config directory: {}", e))?;
    }
    fs::write(&path, contents).map_err(|e| format!("failed to write {}: {}", path.display(), e))
}

fn config_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "Could not determine home directory".to_string())?;
    Ok(PathBuf::from(home).join(".config/slack9/config.toml"))
}
