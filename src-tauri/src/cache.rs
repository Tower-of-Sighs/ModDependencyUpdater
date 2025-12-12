use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};

use crate::util::app_data_dir;

pub fn cache_dir() -> PathBuf {
    let base = app_data_dir().join("cache");
    let _ = fs::create_dir_all(&base);
    base
}

pub fn cache_path(name: &str) -> PathBuf {
    cache_dir().join(name)
}

pub fn safe_key_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "_".to_string()
    } else {
        out
    }
}

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn read_bincode<T: DeserializeOwned>(name: &str) -> Result<T> {
    let path = cache_path(name);
    let bytes = fs::read(path)?;
    let value = bincode::deserialize::<T>(&bytes)?;
    Ok(value)
}

pub fn write_bincode<T: Serialize>(name: &str, value: &T) -> Result<()> {
    let path = cache_path(name);
    let bytes = bincode::serialize(value)?;
    fs::write(path, bytes)?;
    Ok(())
}

pub fn clear_all_cache() -> Result<()> {
    let dir = cache_dir();
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    Ok(())
}
