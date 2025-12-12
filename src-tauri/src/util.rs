use anyhow::Result;
use base64::Engine;
use dirs::data_dir;
use std::path::PathBuf;

pub fn app_data_dir() -> PathBuf {
    data_dir()
        .map(|d| d.join("ModDependencyUpdater"))
        .unwrap_or_else(|| PathBuf::from("ModDependencyUpdater"))
}

use anyhow::anyhow;
use once_cell::sync::Lazy;
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

pub async fn send_with_retry(
    rb: reqwest::RequestBuilder,
    retries: usize,
) -> anyhow::Result<reqwest::Response> {
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

pub fn resolve_cf_api_key(cf_api_key: Option<String>) -> anyhow::Result<String> {
    let api_key = if let Some(key) = cf_api_key {
        if key.trim().is_empty() {
            std::env::var("CF_API_KEY").ok()
        } else {
            Some(key)
        }
    } else {
        std::env::var("CF_API_KEY").ok()
    };
    api_key
        .ok_or_else(|| anyhow::anyhow!("CF_API_KEY is required for CurseForge (Input or Env Var)"))
}

pub fn loader_name_to_tag(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "forge" => "Forge".to_string(),
        "neoforge" => "NeoForge".to_string(),
        "fabric" => "Fabric".to_string(),
        "quilt" => "Quilt".to_string(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
            }
        }
    }
}

pub fn release_type_str(code: u8) -> &'static str {
    match code {
        1 => "release",
        2 => "beta",
        3 => "alpha",
        _ => "unknown",
    }
}

pub async fn cache_icon_from_url(source: &str, key: &str, url: &str) -> anyhow::Result<String> {
    let dir = app_data_dir().join("icons");
    let _ = std::fs::create_dir_all(&dir);
    let lower = url.to_lowercase();
    let ext = if lower.ends_with(".png") {
        "png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "jpg"
    } else if lower.ends_with(".webp") {
        "webp"
    } else {
        "img"
    };
    let path = dir.join(format!("{}-{}.{}", source, key, ext));
    let ttl_secs: u64 = 24 * 60 * 60;
    let mut use_cached = false;
    if let Ok(meta) = std::fs::metadata(&path) {
        if let Ok(modified) = meta.modified() {
            if let Ok(age) = std::time::SystemTime::now().duration_since(modified) {
                if age.as_secs() <= ttl_secs {
                    use_cached = true;
                }
            }
        }
    }
    if path.exists() && use_cached {
        log_event(
            "info",
            &format!(
                "icon_cache_hit {} {} -> {}",
                source,
                key,
                path.to_string_lossy()
            ),
        );
        return Ok(path.to_string_lossy().into());
    }
    let client = http_client()?;
    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 0..3 {
        match send_with_retry(client.get(url), 0).await {
            Ok(resp) => match resp.bytes().await {
                Ok(bytes) => {
                    if let Err(e) = std::fs::write(&path, &bytes) {
                        last_err = Some(anyhow::anyhow!(e));
                    } else {
                        log_event(
                            "info",
                            &format!(
                                "icon_cached {} {} -> {}",
                                source,
                                key,
                                path.to_string_lossy()
                            ),
                        );
                        return Ok(path.to_string_lossy().into());
                    }
                }
                Err(e) => last_err = Some(anyhow::anyhow!(e)),
            },
            Err(e) => last_err = Some(e),
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(150 * (1 << attempt))).await;
    }
    if path.exists() {
        log_event(
            "warn",
            &format!(
                "icon_download_failed_using_cached {} {} url {}",
                source,
                key,
                shorten(url, 200)
            ),
        );
        return Ok(path.to_string_lossy().into());
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("icon download failed")))
}

fn mime_for_ext(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

pub fn file_to_data_url(path: &std::path::Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let mime = mime_for_ext(ext);
    Ok(format!("data:{};base64,{}", mime, b64))
}
