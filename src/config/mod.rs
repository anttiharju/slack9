use serde::Deserialize;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct ReactionsConfig {
    pub backlog: Vec<String>,
    pub taking_a_look: Vec<String>,
    pub blocked: Vec<String>,
    pub completed: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub channels: Vec<String>,
    pub time_window: String,
    pub poll_interval: String,
    pub reactions: ReactionsConfig,
}

impl Config {
    pub fn time_window_duration(&self) -> Result<Duration, String> {
        parse_duration(&self.time_window)
    }

    pub fn poll_interval_duration(&self) -> Result<Duration, String> {
        parse_duration(&self.poll_interval)
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Config (~/slack9s.toml):")?;
        writeln!(f, "  channels: {}", self.channels.join(", "))?;
        writeln!(f, "  time_window: {}", self.time_window)?;
        writeln!(f, "  poll_interval: {}", self.poll_interval)?;
        writeln!(f, "  reactions:")?;
        writeln!(f, "    backlog: {}", self.reactions.backlog.join(", "))?;
        writeln!(f, "    taking_a_look: {}", self.reactions.taking_a_look.join(", "))?;
        writeln!(f, "    blocked: {}", self.reactions.blocked.join(", "))?;
        write!(f, "    completed: {}", self.reactions.completed.join(", "))
    }
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty duration string".to_string());
    }

    let (num_str, unit) = if s.ends_with("ms") {
        (&s[..s.len() - 2], "ms")
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

pub fn load() -> Result<Config, String> {
    let path = config_path()?;

    let contents = fs::read_to_string(&path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    toml::from_str(&contents).map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
}

fn config_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "Could not determine home directory".to_string())?;
    Ok(PathBuf::from(home).join("slack9s.toml"))
}
