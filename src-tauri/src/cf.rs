use crate::util::{log_event, shorten};
use anyhow::{anyhow, Context};
use regex::Regex;
use serde::Deserialize;

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
    #[serde(rename = "latestFilesIndexes")]
    latest_files_indexes: Vec<CfLatestFileIndex>,
}

fn extract_version(text: &str) -> Option<String> {
    let re = Regex::new(r"\d+(?:\.\d+)*(?:[-+][a-zA-Z0-9_.-]+)?").unwrap();
    for cap in re.captures_iter(text) {
        let m = cap.get(0)?.as_str();
        if m.contains('.') {
            return Some(m.to_string());
        }
    }
    None
}

pub async fn get_project_meta(project_id: u32, api_key: &str) -> anyhow::Result<(String, u32)> {
    let client = crate::util::http_client()?;
    let url = format!("https://api.curseforge.com/v1/mods/{}", project_id);
    let resp = client
        .get(&url)
        .header("x-api-key", api_key)
        .header("Accept", "application/json")
        .send()
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
    let resp = client
        .get(&url)
        .header("x-api-key", api_key)
        .header("Accept", "application/json")
        .send()
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
    let loader_tag = match loader.to_lowercase().as_str() {
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        "fabric" => "Fabric",
        "quilt" => "Quilt",
        _ => loader,
    };
    let loader_tag_fallback = if !["Forge", "NeoForge", "Fabric"].contains(&loader_tag) {
        let mut chars = loader.chars();
        match chars.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
        }
    } else {
        loader_tag.to_string()
    };
    let target_loader = if ["Forge", "NeoForge", "Fabric"].contains(&loader_tag) {
        loader_tag
    } else {
        &loader_tag_fallback
    };
    for release_type in [1u8, 2, 3] {
        for idx in &body.data.latest_files_indexes {
            let tag = idx
                .mod_loader
                .map(|code| cf_mod_loader_to_tag(code))
                .unwrap_or("Unknown");
            if idx.release_type != release_type {
                continue;
            }
            if idx.game_version == mc_version && tag == target_loader {
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
        2 => "Cauldron",
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
    let resp = client
        .get(&url)
        .header("x-api-key", api_key)
        .header("Accept", "application/json")
        .send()
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
