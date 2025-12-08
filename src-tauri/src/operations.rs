use anyhow::{anyhow, Context};
use serde_json::json;
use tokio::fs;
use std::path::Path;
use futures::stream::{self, StreamExt};

use crate::cf::{get_cf_latest_indexes, get_latest_cf_file, get_project_meta};
use crate::gradle::{
    ensure_curse_maven_repo, ensure_modrinth_maven_repo, generate_dep, generate_mr_dep,
    update_or_insert_dependency, update_or_insert_dependency_mr,
};
use crate::mojang::{order_mc_versions, order_mc_versions_cf};
use crate::mr::{get_latest_mr_version, get_versions};
use crate::util::{app_data_dir};

async fn process_update(
    gradle_path: String,
    project_id: String,
    mc_version: String,
    loader: String,
    source: String,
    cf_api_key: Option<String>,
) -> anyhow::Result<String> {
    let gradle_path = Path::new(&gradle_path);
    if !gradle_path.exists() {
        return Err(anyhow!("Build.gradle file not found at {:?}", gradle_path));
    }
    let mut gradle_content =
        fs::read_to_string(gradle_path).await.context("Could not read build.gradle")?;
    if source.to_lowercase() == "curseforge" {
        let api_key = if let Some(key) = cf_api_key.as_deref() {
            if key.trim().is_empty() {
                std::env::var("CF_API_KEY").ok()
            } else {
                Some(key.to_string())
            }
        } else {
            std::env::var("CF_API_KEY").ok()
        };
        let api_key = api_key
            .ok_or_else(|| anyhow!("CF_API_KEY is required for CurseForge (Input or Env Var)"))?;
        let pid = project_id
            .parse::<u32>()
            .context("Project ID must be a number for CurseForge")?;
        let (slug, modid_num) = get_project_meta(pid, &api_key).await?;
        let (file_id, version, level) =
            get_latest_cf_file(pid, &mc_version, &loader, &api_key).await?;
        let file_id = file_id.ok_or_else(|| {
            anyhow!(
                "No matching CurseForge file found for MC {} / {}",
                mc_version,
                loader
            )
        })?;
        let level_msg = match level {
            Some(2) => "⚠ Beta Build used\n",
            Some(3) => "⚠ Alpha Build used\n",
            _ => "",
        };
        gradle_content = ensure_curse_maven_repo(&gradle_content);
        let dep_line = generate_dep(&loader, &slug, &modid_num.to_string(), file_id)?;
        gradle_content =
            update_or_insert_dependency(&gradle_content, &modid_num.to_string(), &dep_line);
        fs::write(gradle_path, gradle_content).await.context("Failed to write build.gradle")?;
        Ok(format!(
            "{}✅ Updated Dependency: {}\n🎉 New Version: {} (File ID: {})",
            level_msg,
            dep_line,
            version.unwrap_or_default(),
            file_id
        ))
    } else if source.to_lowercase() == "modrinth" {
        let (ver_id, version, level) =
            get_latest_mr_version(&project_id, &mc_version, &loader).await?;
        let ver_id = ver_id.ok_or_else(|| {
            anyhow!(
                "No matching Modrinth version found for MC {} / {}",
                mc_version,
                loader
            )
        })?;
        let level_msg = match level.as_deref() {
            Some("beta") => "⚠ Beta Build used\n",
            Some("alpha") => "⚠ Alpha Build used\n",
            _ => "",
        };
        gradle_content = ensure_modrinth_maven_repo(&gradle_content);
        let dep_line = generate_mr_dep(&loader, &project_id, &ver_id)?;
        gradle_content = update_or_insert_dependency_mr(&gradle_content, &project_id, &dep_line);
        fs::write(gradle_path, gradle_content).await.context("Failed to write build.gradle")?;
        Ok(format!(
            "{}✅ Updated Dependency: {}\n🎉 New Version: {} (Version ID: {})",
            level_msg,
            dep_line,
            version.unwrap_or_default(),
            ver_id
        ))
    } else {
        Err(anyhow!("Unknown source: {}", source))
    }
}

#[tauri::command]
pub async fn update_dependency(
    gradle_path: String,
    project_id: String,
    mc_version: String,
    loader: String,
    source: String,
    cf_api_key: Option<String>,
) -> Result<String, String> {
    process_update(
        gradle_path,
        project_id,
        mc_version,
        loader,
        source,
        cf_api_key,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_project_options(
    source: String,
    project_id: String,
    cf_api_key: Option<String>,
) -> Result<serde_json::Value, String> {
    let res = || async {
        if source.to_lowercase() == "curseforge" {
            let api_key = if let Some(key) = cf_api_key.as_deref() {
                if key.trim().is_empty() {
                    std::env::var("CF_API_KEY").ok()
                } else {
                    Some(key.to_string())
                }
            } else {
                std::env::var("CF_API_KEY").ok()
            };
            let api_key = api_key.ok_or_else(|| {
                anyhow!("CF_API_KEY is required for CurseForge (Input or Env Var)")
            })?;
            let pid = project_id
                .parse::<u32>()
                .context("Project ID must be a number for CurseForge")?;
            let indexes = get_cf_latest_indexes(pid, &api_key).await?;
            let mut versions_set = std::collections::BTreeSet::new();
            let mut loaders_set = std::collections::BTreeSet::new();
            let mut v2l: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
                std::collections::BTreeMap::new();
            let mut l2v: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
                std::collections::BTreeMap::new();
            for idx in indexes {
                let tag = match idx.mod_loader {
                    Some(1) => "Forge".to_string(),
                    Some(6) => "NeoForge".to_string(),
                    Some(4) => "Fabric".to_string(),
                    Some(5) => "Quilt".to_string(),
                    Some(3) => "LiteLoader".to_string(),
                    Some(7) => "Rift".to_string(),
                    Some(_) | None => continue,
                };

                let ver = idx.game_version.clone();
                if ver.chars().next().map(|c| c.is_numeric()).unwrap_or(false) {
                    versions_set.insert(ver.clone());
                }
                loaders_set.insert(tag.clone());
                v2l.entry(ver.clone()).or_default().insert(tag.clone());
                l2v.entry(tag.clone()).or_default().insert(ver.clone());
            }
            let mut versions: Vec<String> = versions_set.into_iter().collect();
            versions = order_mc_versions_cf(versions);
            let loaders: Vec<String> = loaders_set.into_iter().collect();
            let v2l_vec = v2l
                .into_iter()
                .map(|(k, v)| (k, v.into_iter().collect::<Vec<_>>()))
                .collect::<std::collections::BTreeMap<_, _>>();
            let l2v_vec = l2v
                .into_iter()
                .map(|(k, v)| (k, order_mc_versions_cf(v.into_iter().collect::<Vec<_>>())))
                .collect::<std::collections::BTreeMap<_, _>>();
            Ok(
                json!({"versions": versions, "loaders": loaders, "id": pid, "version_to_loaders": v2l_vec, "loader_to_versions": l2v_vec}),
            )
        } else if source.to_lowercase() == "modrinth" {
            let versions = get_versions(&project_id).await?;
            let mut vset = std::collections::BTreeSet::new();
            let mut lset = std::collections::BTreeSet::new();
            let mut v2l: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
                std::collections::BTreeMap::new();
            let mut l2v: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
                std::collections::BTreeMap::new();
            for v in versions {
                let tags: Vec<String> = v
                    .loaders
                    .iter()
                    .map(|ld| match ld.as_str() {
                        "forge" => "Forge".to_string(),
                        "neoforge" => "NeoForge".to_string(),
                        "fabric" => "Fabric".to_string(),
                        "quilt" => "Quilt".to_string(),
                        other => other.to_string(),
                    })
                    .collect();
                for gv in v.game_versions.iter() {
                    vset.insert(gv.clone());
                    for t in tags.iter() {
                        lset.insert(t.clone());
                        v2l.entry(gv.clone()).or_default().insert(t.clone());
                        l2v.entry(t.clone()).or_default().insert(gv.clone());
                    }
                }
            }
            let mut versions: Vec<String> = vset.into_iter().collect();
            versions = order_mc_versions(versions);
            let loaders: Vec<String> = lset.into_iter().collect();
            let v2l_vec = v2l
                .into_iter()
                .map(|(k, v)| (k, v.into_iter().collect::<Vec<_>>()))
                .collect::<std::collections::BTreeMap<_, _>>();
            let l2v_vec = l2v
                .into_iter()
                .map(|(k, v)| (k, order_mc_versions(v.into_iter().collect::<Vec<_>>())))
                .collect::<std::collections::BTreeMap<_, _>>();
            Ok(
                json!({"versions": versions, "loaders": loaders, "slug": project_id, "version_to_loaders": v2l_vec, "loader_to_versions": l2v_vec}),
            )
        } else {
            Err(anyhow!("Unknown source: {}", source))
        }
    };
    res().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_dependencies_batch(
    gradle_path: String,
    source: String,
    items: Vec<String>,
    mc_version: String,
    loader: String,
    cf_api_key: Option<String>,
) -> Result<String, String> {
    let limit = 4usize;
    let gradle_path_c = gradle_path.clone();
    let source_c = source.clone();
    let mc_version_c = mc_version.clone();
    let loader_c = loader.clone();
    let cf_api_key_c = cf_api_key.clone();
    let s = stream::iter(items.into_iter().map(|item| {
        let gradle_path = gradle_path_c.clone();
        let source = source_c.clone();
        let mc_version = mc_version_c.clone();
        let loader = loader_c.clone();
        let cf_api_key = cf_api_key_c.clone();
        async move {
            match process_update(gradle_path, item.clone(), mc_version, loader, source, cf_api_key).await {
                Ok(res) => format!("\n[{}] {}\n", item, res),
                Err(err) => format!("\n[{}] ❌ {}\n", item, err),
            }
        }
    }));
    let results = s.buffer_unordered(limit).collect::<Vec<_>>().await;
    Ok(results.into_iter().collect())
}

#[tauri::command]
pub async fn save_log(content: String) -> Result<String, String> {
    let base = app_data_dir().join("logs");
    if let Err(e) = std::fs::create_dir_all(&base) {
        return Err(e.to_string());
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    let path = base.join(format!("log-{}.txt", ts));
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into())
}
