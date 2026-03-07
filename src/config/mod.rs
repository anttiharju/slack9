use serde::Deserialize;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default = "default_time_window")]
    pub time_window: String,
    #[serde(default = "default_poll_interval")]
    pub poll_interval: String,
}

fn default_time_window() -> String {
    "24h".to_string()
}

fn default_poll_interval() -> String {
    "10s".to_string()
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
