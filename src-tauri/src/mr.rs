use crate::util::{log_event, shorten};
use anyhow::{anyhow, Context};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct MrVersion {
    pub id: String,
    pub version_number: String,
    pub version_type: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub date_published: String,
}

pub async fn get_latest_mr_version(
    project_slug: &str,
    mc_version: &str,
    loader: &str,
) -> anyhow::Result<(Option<String>, Option<String>, Option<String>)> {
    let client = crate::util::http_client()?;
    let url = format!(
        "https://api.modrinth.com/v2/project/{}/version",
        project_slug
    );
    let resp = crate::util::send_with_retry(
        client.get(&url),
        2,
    )
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
    let mut versions: Vec<MrVersion> = serde_json::from_str(&body_text).map_err(|e| {
        anyhow!(format!(
            "Modrinth parse error: {} body {}",
            e,
            shorten(&body_text, 400)
        ))
    })?;
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

pub async fn get_versions(project_slug: &str) -> anyhow::Result<Vec<MrVersion>> {
    let client = crate::util::http_client()?;
    let url = format!(
        "https://api.modrinth.com/v2/project/{}/version",
        project_slug
    );
    let resp = crate::util::send_with_retry(
        client.get(&url),
        2,
    )
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
