use anyhow::Result;
use dirs::data_dir;
use std::path::PathBuf;

pub fn app_data_dir() -> PathBuf {
    data_dir()
        .map(|d| d.join("ModDependencyUpdater"))
        .unwrap_or_else(|| PathBuf::from("ModDependencyUpdater"))
}

use once_cell::sync::Lazy;
use anyhow::anyhow;
use tokio::time::{sleep, Duration};
static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent("ModDependencyUpdater/1.0 (Tauri)")
        .pool_idle_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("http client")
});
pub fn http_client() -> Result<reqwest::Client> {
    Ok(CLIENT.clone())
}

pub async fn send_with_retry(rb: reqwest::RequestBuilder, retries: usize) -> anyhow::Result<reqwest::Response> {
    let mut last_err: Option<reqwest::Error> = None;
    for attempt in 0..=retries {
        let cloned = rb
            .try_clone()
            .ok_or_else(|| anyhow!("cannot clone request"))?;
        match cloned.send().await {
            Ok(resp) => return Ok(resp),
            Err(e) => {
                last_err = Some(e);
                if attempt < retries {
                    sleep(Duration::from_millis(200 * (1 << attempt))).await;
                }
            }
        }
    }
    Err(anyhow!(last_err.unwrap()))
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
