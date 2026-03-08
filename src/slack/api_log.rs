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
    let dur = t.duration_since(UNIX_EPOCH).unwrap();
    let secs = dur.as_secs();
    let millis = dur.subsec_millis();

    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Convert days since epoch to Y-M-D
    let (year, month, day) = days_to_date(days);

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        year, month, day, hours, minutes, seconds, millis
    )
}

fn days_to_date(days_since_epoch: u64) -> (u64, u64, u64) {
    // Civil calendar algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days_since_epoch + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
