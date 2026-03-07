use serde::Deserialize;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_TIME_WINDOW: &str = "24h";
const DEFAULT_POLL_INTERVAL: &str = "10s";

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_time_window")]
    pub time_window: String,
    #[serde(default = "default_poll_interval")]
    pub poll_interval: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            time_window: default_time_window(),
            poll_interval: default_poll_interval(),
        }
    }
}

fn default_time_window() -> String {
    DEFAULT_TIME_WINDOW.to_string()
}

fn default_poll_interval() -> String {
    DEFAULT_POLL_INTERVAL.to_string()
}

impl Config {
    pub fn time_window_duration(&self) -> Result<Duration, String> {
        parse_duration(&self.time_window)
    }

    pub fn poll_interval_duration(&self) -> Result<Duration, String> {
        parse_duration(&self.poll_interval)
    }

    pub fn time_window_label(&self) -> String {
        if self.time_window == DEFAULT_TIME_WINDOW {
            format!("{} (default)", self.time_window)
        } else {
            self.time_window.clone()
        }
    }

    pub fn poll_interval_label(&self) -> String {
        if self.poll_interval == DEFAULT_POLL_INTERVAL {
            format!("{} (default)", self.poll_interval)
        } else {
            self.poll_interval.clone()
        }
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Config (~/.slack9.toml):")?;
        writeln!(f, "  time_window: {}", self.time_window)?;
        write!(f, "  poll_interval: {}", self.poll_interval)?;
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
