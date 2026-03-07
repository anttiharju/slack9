use serde::Deserialize;
use std::fmt;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub channels: Vec<String>,
    pub time_window: String,
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Config (~/slackemon.toml):")?;
        writeln!(f, "  channels: {}", self.channels.join(", "))?;
        write!(f, "  time_window: {}", self.time_window)
    }
}

pub fn load() -> Result<Config, String> {
    let path = config_path()?;

    let contents = fs::read_to_string(&path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    toml::from_str(&contents).map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
}

fn config_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "Could not determine home directory".to_string())?;
    Ok(PathBuf::from(home).join("slackemon.toml"))
}
