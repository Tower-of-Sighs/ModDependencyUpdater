use anyhow::Result;
use dirs::data_dir;
use std::path::PathBuf;

pub fn app_data_dir() -> PathBuf {
    data_dir()
        .map(|d| d.join("ModDependencyUpdater"))
        .unwrap_or_else(|| PathBuf::from("ModDependencyUpdater"))
}

pub fn http_client() -> Result<reqwest::Client> {
    let client = reqwest::Client::builder()
        .user_agent("ModDependencyUpdater/1.0 (Tauri)")
        .build()?;
    Ok(client)
}

pub fn log_event(level: &str, msg: &str) {
    let dir = app_data_dir().join("logs");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("runtime.log");
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let line = format!("[{}][{}] {}\n", ts, level, msg);
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
    println!("{}", line.trim_end());
}

pub fn shorten(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}
