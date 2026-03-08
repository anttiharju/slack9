use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ApiLog {
    path: PathBuf,
}

impl ApiLog {
    pub fn new() -> Result<Self, String> {
        let home = std::env::var("HOME").map_err(|_| "Could not determine home directory".to_string())?;
        let dir = PathBuf::from(home).join(".config/slack9");
        fs::create_dir_all(&dir).map_err(|e| format!("failed to create log directory: {}", e))?;

        let epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let path = dir.join(format!("session-{}.log", epoch));

        Ok(Self { path })
    }

    pub fn log(&self, api_method: &str) {
        let line = format!("[{}] {}\n", format_utc(SystemTime::now()), api_method);
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&self.path) {
            let _ = file.write_all(line.as_bytes());
        }
    }
}

fn format_utc(t: SystemTime) -> String {
    let ts = jiff::Timestamp::try_from(t).unwrap();
    let dt = ts.to_zoned(jiff::tz::TimeZone::UTC).datetime();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        dt.year(),
        dt.month(),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second(),
        dt.subsec_nanosecond() / 1_000_000
    )
}
