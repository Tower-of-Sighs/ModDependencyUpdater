use crate::cache::{now_millis, read_bincode, write_bincode};
use crate::util::{log_event, shorten};
use anyhow::{anyhow, Context};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
struct MrProjectBrief {
    title: String,
    icon_url: Option<String>,
}

pub async fn get_mr_mod_brief(project_slug: &str) -> anyhow::Result<(String, Option<String>)> {
    let client = crate::util::http_client()?;
    let url = format!("https://api.modrinth.com/v2/project/{}", project_slug);
    let resp = crate::util::send_with_retry(client.get(&url), 2)
        .await
        .context("Failed to connect to Modrinth API")?;
    let status = resp.status();
    let body_text = resp
        .text()
        .await
        .context("Failed to read Modrinth response body")?;
    if !status.is_success() {
        log_event(
            "error",
            &format!(
                "MR status {} url {} body {}",
                status,
                url,
                shorten(&body_text, 400)
            ),
        );
        return Err(anyhow!(format!(
            "Modrinth API Error: {} body {}",
            status,
            shorten(&body_text, 400)
        )));
    }
    let proj: MrProjectBrief = serde_json::from_str(&body_text).map_err(|e| {
        anyhow!(format!(
            "Modrinth parse error: {} body {}",
            e,
            shorten(&body_text, 400)
        ))
    })?;
    log_event(
        "info",
        &format!(
            "mr_mod_brief {} {}",
            proj.title,
            proj.icon_url.clone().unwrap_or_default()
        ),
    );
    Ok((proj.title, proj.icon_url))
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MrVersion {
    pub id: String,
    pub version_number: String,
    pub version_type: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub date_published: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct MrVersionCache {
    versions: Vec<MrVersion>,
    fetched_at: u64,
}

fn mr_cache_name(project_slug: &str) -> String {
    let key = crate::cache::safe_key_segment(project_slug);
    format!("mr-versions-{}.bin", key)
}

async fn fetch_versions(url: &str) -> anyhow::Result<Vec<MrVersion>> {
    let client = crate::util::http_client()?;
    let resp = crate::util::send_with_retry(client.get(url), 2)
        .await
        .context("Failed to connect to Modrinth API")?;
    let status = resp.status();
    let body_text = resp
        .text()
        .await
        .context("Failed to read Modrinth response body")?;
    if !status.is_success() {
        log_event(
            "error",
            &format!(
                "MR status {} url {} body {}",
                status,
                url,
                shorten(&body_text, 400)
            ),
        );
        return Err(anyhow!(format!(
            "Modrinth API Error: {} body {}",
            status,
            shorten(&body_text, 400)
        )));
    }
    let versions: Vec<MrVersion> = serde_json::from_str(&body_text).map_err(|e| {
        anyhow!(format!(
            "Modrinth parse error: {} body {}",
            e,
            shorten(&body_text, 400)
        ))
    })?;
    Ok(versions)
}

async fn fetch_and_store_versions(project_slug: &str) -> anyhow::Result<Vec<MrVersion>> {
    let url = format!(
        "https://api.modrinth.com/v2/project/{}/version",
        project_slug
    );
    let versions = fetch_versions(&url).await?;
    let cache = MrVersionCache {
        versions: versions.clone(),
        fetched_at: now_millis(),
    };
    write_bincode(&mr_cache_name(project_slug), &cache)?;
    Ok(versions)
}

async fn load_versions_from_cache(project_slug: &str) -> Option<MrVersionCache> {
    read_bincode(&mr_cache_name(project_slug)).ok()
}

pub async fn get_latest_mr_version(
    project_slug: &str,
    mc_version: &str,
    loader: &str,
) -> anyhow::Result<(Option<String>, Option<String>, Option<String>)> {
    let mut versions = fetch_and_store_versions(project_slug).await?;
    versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    let priority_order = ["release", "beta", "alpha"];
    let loader_lower = loader.to_lowercase();
    for vtype in priority_order {
        for ver in &versions {
            if ver.version_type != vtype {
                continue;
            }
            if ver.game_versions.contains(&mc_version.to_string())
                && ver.loaders.iter().any(|l| l.to_lowercase() == loader_lower)
            {
                return Ok((
                    Some(ver.id.clone()),
                    Some(ver.version_number.clone()),
                    Some(ver.version_type.clone()),
                ));
            }
        }
    }
    Ok((None, None, None))
}

pub async fn get_versions(project_slug: &str, use_cache: bool) -> anyhow::Result<Vec<MrVersion>> {
    const TTL_MS: u64 = 6 * 60 * 60 * 1000;
    if use_cache {
        if let Some(cache) = load_versions_from_cache(project_slug).await {
            let age = now_millis().saturating_sub(cache.fetched_at);
            if age > TTL_MS {
                let slug = project_slug.to_string();
                tokio::spawn(async move {
                    let _ = fetch_and_store_versions(&slug).await;
                });
            }
            return Ok(cache.versions);
        }
    }
    fetch_and_store_versions(project_slug).await
}

pub async fn get_versions_filtered(
    project_slug: &str,
    mc_version: &str,
    loader: &str,
    use_cache: bool,
) -> anyhow::Result<Vec<MrVersion>> {
    if use_cache {
        let all = get_versions(project_slug, true).await?;
        let loader_lower = loader.to_lowercase();
        let mc = mc_version.to_string();
        let filtered: Vec<MrVersion> = all
            .into_iter()
            .filter(|v| {
                v.game_versions.contains(&mc)
                    && v.loaders.iter().any(|l| l.to_lowercase() == loader_lower)
            })
            .collect();
        return Ok(filtered);
    }
    let loader_lower = loader.to_lowercase();
    let url = format!(
        "https://api.modrinth.com/v2/project/{}/version?game_versions={}&loaders={}",
        project_slug, mc_version, loader_lower
    );
    fetch_versions(&url).await
}
