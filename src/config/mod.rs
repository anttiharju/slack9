use serde::Deserialize;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_PAST: &str = "24h";
const DEFAULT_POLL: &str = "10s";

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_past")]
    pub past: String,
    #[serde(default = "default_poll")]
    pub poll: String,
}

impl Default for Config {
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

impl Config {
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
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Config (~/.slack9.toml):")?;
        writeln!(f, "  past: {}", self.past)?;
        write!(f, "  poll: {}", self.poll)?;
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
        "h" => Ok(Duration::from_secs(value * 3600)),
        "d" => Ok(Duration::from_secs(value * 86400)),
        "ms" => Ok(Duration::from_millis(value)),
        _ => Err(format!("unknown duration unit '{}' in '{}'. Use s, m, h, d, or ms", unit, s)),
    }
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

fn config_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "Could not determine home directory".to_string())?;
    Ok(PathBuf::from(home).join(".slack9.toml"))
}
