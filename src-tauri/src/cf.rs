use crate::cache::{now_millis, read_bincode, write_bincode};
use crate::util::{log_event, shorten};
use anyhow::{anyhow, Context};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
static VERSION_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\d+(?:\.\d+)*(?:[-+][a-zA-Z0-9_.-]+)?").unwrap());

#[derive(Deserialize, Debug)]
struct CfModResponse {
    data: CfModData,
}

#[derive(Deserialize, Debug)]
pub struct CfLatestFileIndex {
    #[serde(rename = "gameVersion")]
    pub game_version: String,
    #[serde(rename = "fileId")]
    pub file_id: u32,
    #[serde(rename = "filename")]
    pub filename: String,
    #[serde(rename = "releaseType")]
    pub release_type: u8,
    #[serde(rename = "modLoader")]
    pub mod_loader: Option<u8>,
}

#[derive(Deserialize, Debug)]
struct CfModData {
    id: u32,
    slug: String,
    name: String,
    logo: CfLogo,
    #[serde(rename = "latestFilesIndexes")]
    latest_files_indexes: Vec<CfLatestFileIndex>,
}

#[derive(Deserialize, Debug)]
struct CfLogo {
    #[serde(rename = "url")]
    url: String,
    #[serde(rename = "thumbnailUrl")]
    thumbnail_url: Option<String>,
}

pub fn extract_version(text: &str) -> Option<String> {
    for cap in VERSION_RE.captures_iter(text) {
        let m = cap.get(0)?.as_str();
        if m.contains('.') {
            return Some(m.to_string());
        }
    }
    None
}

pub fn strip_jar_suffix(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.ends_with(".jar") {
        name[..name.len().saturating_sub(4)].to_string()
    } else {
        name.to_string()
    }
}

pub async fn get_cf_mod_brief(
    project_id: u32,
    api_key: &str,
) -> anyhow::Result<(String, Option<String>)> {
    let client = crate::util::http_client()?;
    let url = format!("https://api.curseforge.com/v1/mods/{}", project_id);
    let resp = crate::util::send_with_retry(
        client
            .get(&url)
            .header("x-api-key", api_key)
            .header("Accept", "application/json"),
        2,
    )
    .await
    .context("Failed to fetch mod detail from CurseForge")?;
    let status = resp.status();
    let body_text = resp
        .text()
        .await
        .context("Failed to read mod detail body")?;
    if !status.is_success() {
        crate::util::log_event(
            "error",
            &format!(
                "CF detail status {} url {} body {}",
                status,
                url,
                crate::util::shorten(&body_text, 400)
            ),
        );
        return Err(anyhow!(format!(
            "CurseForge API Error (Mod Detail): {} body {}",
            status,
            crate::util::shorten(&body_text, 400)
        )));
    }
    let body: CfModResponse = serde_json::from_str(&body_text).map_err(|e| {
        anyhow!(format!(
            "CurseForge parse error: {} body {}",
            e,
            crate::util::shorten(&body_text, 400)
        ))
    })?;
    let icon = body
        .data
        .logo
        .thumbnail_url
        .clone()
        .or(Some(body.data.logo.url.clone()));
    log_event(
        "info",
        &format!(
            "cf_mod_brief {} {}",
            body.data.name,
            icon.clone().unwrap_or_default()
        ),
    );
    Ok((body.data.name, icon))
}

pub async fn get_project_meta(project_id: u32, api_key: &str) -> anyhow::Result<(String, u32)> {
    let client = crate::util::http_client()?;
    let url = format!("https://api.curseforge.com/v1/mods/{}", project_id);
    let resp = crate::util::send_with_retry(
        client
            .get(&url)
            .header("x-api-key", api_key)
            .header("Accept", "application/json"),
        2,
    )
    .await
    .context("Failed to connect to CurseForge API")?;
    let status = resp.status();
    let body_text = resp
        .text()
        .await
        .context("Failed to read CurseForge response body")?;
    if !status.is_success() {
        log_event(
            "error",
            &format!(
                "CF meta status {} url {} body {}",
                status,
                url,
                shorten(&body_text, 400)
            ),
        );
        return Err(anyhow!(format!(
            "CurseForge API Error: {} body {}",
            status,
            shorten(&body_text, 400)
        )));
    }
    let body: CfModResponse = serde_json::from_str(&body_text).map_err(|e| {
        anyhow!(format!(
            "CurseForge parse error: {} body {}",
            e,
            shorten(&body_text, 400)
        ))
    })?;
    Ok((body.data.slug, body.data.id))
}

pub async fn get_latest_cf_file(
    project_id: u32,
    mc_version: &str,
    loader: &str,
    api_key: &str,
) -> anyhow::Result<(Option<u32>, Option<String>, Option<u8>)> {
    let client = crate::util::http_client()?;
    let url = format!("https://api.curseforge.com/v1/mods/{}", project_id);
    let resp = crate::util::send_with_retry(
        client
            .get(&url)
            .header("x-api-key", api_key)
            .header("Accept", "application/json"),
        2,
    )
    .await
    .context("Failed to fetch mod detail from CurseForge")?;
    let status = resp.status();
    let body_text = resp
        .text()
        .await
        .context("Failed to read mod detail body")?;
    if !status.is_success() {
        log_event(
            "error",
            &format!(
                "CF detail status {} url {} body {}",
                status,
                url,
                shorten(&body_text, 400)
            ),
        );
        return Err(anyhow!(format!(
            "CurseForge API Error (Mod Detail): {} body {}",
            status,
            shorten(&body_text, 400)
        )));
    }
    let body: CfModResponse = serde_json::from_str(&body_text).map_err(|e| {
        anyhow!(format!(
            "CurseForge parse error: {} body {}",
            e,
            shorten(&body_text, 400)
        ))
    })?;
    let target_loader = crate::util::loader_name_to_tag(&loader);
    for release_type in [1u8, 2, 3] {
        for idx in &body.data.latest_files_indexes {
            let tag = idx
                .mod_loader
                .map(|code| cf_mod_loader_to_tag(code))
                .unwrap_or("Unknown");
            if idx.release_type != release_type {
                continue;
            }
            if idx.game_version == mc_version && tag == target_loader.as_str() {
                let version =
                    extract_version(&idx.filename).unwrap_or_else(|| idx.file_id.to_string());
                return Ok((Some(idx.file_id), Some(version), Some(idx.release_type)));
            }
        }
    }
    Ok((None, None, None))
}

pub fn cf_mod_loader_to_tag(code: u8) -> &'static str {
    match code {
        1 => "Forge",
        6 => "NeoForge",
        4 => "Fabric",
        5 => "Quilt",
        3 => "LiteLoader",
        7 => "Rift",
        _ => "Unknown",
    }
}

pub async fn get_cf_latest_indexes(
    project_id: u32,
    api_key: &str,
) -> anyhow::Result<Vec<CfLatestFileIndex>> {
    let client = crate::util::http_client()?;
    let url = format!("https://api.curseforge.com/v1/mods/{}", project_id);
    let resp = crate::util::send_with_retry(
        client
            .get(&url)
            .header("x-api-key", api_key)
            .header("Accept", "application/json"),
        2,
    )
    .await
    .context("Failed to fetch mod detail from CurseForge")?;
    let status = resp.status();
    let body_text = resp
        .text()
        .await
        .context("Failed to read mod detail body")?;
    if !status.is_success() {
        log_event(
            "error",
            &format!(
                "CF detail status {} url {} body {}",
                status,
                url,
                shorten(&body_text, 400)
            ),
        );
        return Err(anyhow!(format!(
            "CurseForge API Error (Mod Detail): {} body {}",
            status,
            shorten(&body_text, 400)
        )));
    }
    let body: CfModResponse = serde_json::from_str(&body_text).map_err(|e| {
        anyhow!(format!(
            "CurseForge parse error: {} body {}",
            e,
            shorten(&body_text, 400)
        ))
    })?;
    Ok(body.data.latest_files_indexes)
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CfFileItem {
    #[serde(rename = "id")]
    pub id: u32,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(rename = "fileDate")]
    pub file_date: String,
    #[serde(rename = "releaseType")]
    pub release_type: u8,
    #[serde(rename = "gameVersions")]
    pub game_versions: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct CfFilesResponse {
    data: Vec<CfFileItem>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct CfFilesCache {
    files: Vec<CfFileItem>,
    fetched_at: u64,
}

fn cf_cache_name(project_id: u32, mc_version: &str, loader_code: u8) -> String {
    let v = crate::cache::safe_key_segment(mc_version);
    format!("cf-files-{}-{}-{}.bin", project_id, v, loader_code)
}

pub fn cf_mod_loader_code_from_name(name: &str) -> Option<u8> {
    match name.to_lowercase().as_str() {
        "forge" => Some(1),
        "neoforge" => Some(6),
        "fabric" => Some(4),
        "quilt" => Some(5),
        _ => None,
    }
}

pub async fn get_cf_files_filtered(
    project_id: u32,
    mc_version: &str,
    loader_code: u8,
    api_key: &str,
    use_cache: bool,
) -> anyhow::Result<Vec<CfFileItem>> {
    let client = crate::util::http_client()?;
    const TTL_MS: u64 = 6 * 60 * 60 * 1000;
    if use_cache {
        if let Ok(cache) =
            read_bincode::<CfFilesCache>(&cf_cache_name(project_id, mc_version, loader_code))
        {
            let age = now_millis().saturating_sub(cache.fetched_at);
            if age <= TTL_MS {
                return Ok(cache.files);
            }
        }
    }
    let page_size: u32 = 50;
    let mut index: u32 = 0;
    let mut all: Vec<CfFileItem> = Vec::new();
    const MAX_FILES: usize = 500;
    loop {
        let url = format!(
            "https://api.curseforge.com/v1/mods/{}/files?gameVersion={}&modLoaderType={}&pageSize={}&index={}",
            project_id, mc_version, loader_code, page_size, index
        );
        let resp = crate::util::send_with_retry(
            client
                .get(&url)
                .header("x-api-key", api_key)
                .header("Accept", "application/json"),
            2,
        )
        .await
        .context("Failed to fetch mod files from CurseForge")?;
        let status = resp.status();
        let body_text = resp.text().await.context("Failed to read mod files body")?;
        if !status.is_success() {
            log_event(
                "error",
                &format!(
                    "CF files status {} url {} body {}",
                    status,
                    url,
                    shorten(&body_text, 400)
                ),
            );
            return Err(anyhow!(format!(
                "CurseForge API Error (Mod Files): {} body {}",
                status,
                shorten(&body_text, 400)
            )));
        }
        let body: CfFilesResponse = serde_json::from_str(&body_text).map_err(|e| {
            anyhow!(format!(
                "CurseForge files parse error: {} body {}",
                e,
                shorten(&body_text, 400)
            ))
        })?;
        let count = body.data.len();
        all.extend(body.data.into_iter());
        if count < page_size as usize || all.len() >= MAX_FILES {
            break;
        }
        index += 1;
    }
    if use_cache {
        let cache = CfFilesCache {
            files: all.clone(),
            fetched_at: now_millis(),
        };
        let _ = write_bincode(&cf_cache_name(project_id, mc_version, loader_code), &cache);
    }
    Ok(all)
}
