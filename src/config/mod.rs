use indexmap::IndexMap;
use serde::de::Deserializer;
use serde::{Deserialize, Serialize, Serializer};
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
    #[serde(
        default,
        skip_serializing_if = "IndexMap::is_empty",
        deserialize_with = "deserialize_categories",
        serialize_with = "serialize_categories"
    )]
    pub categories: IndexMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "StateConfig::is_default")]
    pub state: StateConfig,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct StateConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
}

impl StateConfig {
    fn is_default(&self) -> bool {
        self.search.is_none()
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

/// Deserialize categories: each value can be a single string or an array of strings.
fn deserialize_categories<'de, D>(deserializer: D) -> Result<IndexMap<String, Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        Single(String),
        Multiple(Vec<String>),
    }

    let raw: IndexMap<String, StringOrVec> = IndexMap::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .map(|(k, v)| match v {
            StringOrVec::Single(s) => (k, vec![s]),
            StringOrVec::Multiple(v) => (k, v),
        })
        .collect())
}

/// Serialize categories: single-element arrays as a plain string, multi-element as array.
fn serialize_categories<S>(map: &IndexMap<String, Vec<String>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    use serde::ser::SerializeMap;
    let mut ser_map = serializer.serialize_map(Some(map.len()))?;
    for (k, v) in map {
        if v.len() == 1 {
            ser_map.serialize_entry(k, &v[0])?;
        } else {
            ser_map.serialize_entry(k, v)?;
        }
    }
    ser_map.end()
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dir = config_dir().unwrap_or_else(|_| PathBuf::from("~/.config/slack9"));
        writeln!(f, "Config ({}/config.toml):", dir.display())?;
        writeln!(f, "  [header]")?;
        writeln!(f, "    past: {}", self.header.past)?;
        writeln!(f, "    poll: {}", self.header.poll)?;
        if !self.categories.is_empty() {
            writeln!(f, "  [categories]")?;
            for (name, emojis) in &self.categories {
                if emojis.len() == 1 {
                    writeln!(f, "    {} = {}", name, emojis[0])?;
                } else {
                    writeln!(f, "    {} = [{}]", name, emojis.join(", "))?;
                }
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

pub fn config_dir() -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("SLACK9_CONFIG_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var("HOME").map_err(|_| "Could not determine home directory".to_string())?;
    Ok(PathBuf::from(home).join(".config/slack9"))
}

fn config_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("config.toml"))
}
