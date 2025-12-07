use anyhow::Context;
use bincode::{deserialize, serialize};
use regex::Regex;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use crate::util::{app_data_dir, log_event, shorten};

#[derive(Deserialize)]
struct MojangVersion {
    id: String,
}

#[derive(Deserialize)]
struct MojangManifest {
    versions: Vec<MojangVersion>,
}

fn cache_file() -> PathBuf {
    let base = app_data_dir().join("cache");
    let _ = std::fs::create_dir_all(&base);
    base.join("mc_versions.bin")
}

pub async fn refresh_manifest_cache_on_startup() -> anyhow::Result<()> {
    let client = crate::util::http_client()?;
    let url = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";
    let resp = client
        .get(url)
        .send()
        .await
        .context("Failed to fetch Mojang manifest")?;
    let status = resp.status();
    let body_text = resp
        .text()
        .await
        .context("Failed to read Mojang manifest body")?;
    if !status.is_success() {
        log_event(
            "error",
            &format!(
                "Mojang status {} url {} body {}",
                status,
                url,
                shorten(&body_text, 400)
            ),
        );
        return Err(anyhow::anyhow!(format!(
            "Mojang API Error: {} body {}",
            status,
            shorten(&body_text, 400)
        )));
    }
    let manifest: MojangManifest = serde_json::from_str(&body_text).map_err(|e| {
        anyhow::anyhow!(format!(
            "Mojang parse error: {} body {}",
            e,
            shorten(&body_text, 400)
        ))
    })?;
    let mut map: HashMap<String, u16> = HashMap::new();
    for (i, v) in manifest.versions.into_iter().enumerate() {
        if i <= u16::MAX as usize {
            map.insert(v.id, i as u16);
        }
    }
    let data = serialize(&map)?;
    fs::write(cache_file(), data).context("Failed to write manifest cache")?;
    Ok(())
}

pub fn order_mc_versions(input: Vec<String>) -> Vec<String> {
    let path = cache_file();
    let raw = fs::read(path);
    if let Ok(bytes) = raw {
        if let Ok(index) = deserialize::<HashMap<String, u16>>(&bytes) {
            let mut items: Vec<(u16, String)> = input
                .into_iter()
                .map(|s| (index.get(&s).copied().unwrap_or(u16::MAX), s))
                .collect();
            items.sort_by(|a, b| a.0.cmp(&b.0));
            let mut seen: HashSet<String> = HashSet::new();
            let mut out = Vec::new();
            for (_, s) in items {
                if seen.insert(s.clone()) {
                    out.push(s);
                }
            }
            return out;
        }
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for s in input {
        if seen.insert(s.clone()) {
            out.push(s);
        }
    }
    out
}

pub fn order_mc_versions_cf(input: Vec<String>) -> Vec<String> {
    let path = cache_file();
    let raw = fs::read(path);
    if let Ok(bytes) = raw {
        if let Ok(index) = deserialize::<HashMap<String, u16>>(&bytes) {
            let base_re = Regex::new(r"^\d+(?:\.\d+)+").unwrap();
            let rc_re = Regex::new(r"(?i)-rc(\d+)").unwrap();
            let pre_re = Regex::new(r"(?i)-pre(\d+)").unwrap();
            let mut items: Vec<(u16, u8, u16, String)> = Vec::new();
            for s in input {
                let sl = s.to_lowercase();
                let mut idx = *index.get(&s).unwrap_or(&u16::MAX);
                if idx == u16::MAX {
                    if let Some(m) = base_re.find(&sl) {
                        let base = &sl[m.start()..m.end()];
                        if let Some(bi) = index.get(base) {
                            idx = *bi;
                        }
                    }
                }
                let mut kind: u8 = 0;
                let mut rank: u16 = 0;
                if let Some(cap) = rc_re.captures(&sl) {
                    kind = 1;
                    rank = cap
                        .get(1)
                        .and_then(|g| g.as_str().parse::<u16>().ok())
                        .map(|n| u16::MAX - n)
                        .unwrap_or(u16::MAX);
                } else if let Some(cap) = pre_re.captures(&sl) {
                    kind = 2;
                    rank = cap
                        .get(1)
                        .and_then(|g| g.as_str().parse::<u16>().ok())
                        .map(|n| u16::MAX - n)
                        .unwrap_or(u16::MAX);
                } else if sl.contains("snapshot") {
                    kind = 3;
                }
                items.push((idx, kind, rank, s));
            }
            items.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
            let mut seen: HashSet<String> = HashSet::new();
            let mut out = Vec::new();
            for (_, _, _, s) in items {
                if seen.insert(s.clone()) {
                    out.push(s);
                }
            }
            return out;
        }
    }
    input
}
