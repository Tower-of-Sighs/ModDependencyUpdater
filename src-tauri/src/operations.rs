use anyhow::{anyhow, Context};
use chrono::Local;
use futures::stream::{self, StreamExt};
use serde::Serialize;
use serde_json::json;
use std::path::Path;
use tokio::fs;

use crate::cf::{get_cf_latest_indexes, get_latest_cf_file, get_project_meta};
use crate::gradle::{
    ensure_curse_maven_repo, ensure_modrinth_maven_repo, generate_dep, generate_mr_dep,
    update_or_insert_dependency, update_or_insert_dependency_mr,
};
use crate::mojang::{order_mc_versions, order_mc_versions_cf};
use crate::mr::{get_latest_mr_version, get_mr_mod_brief, get_versions, get_versions_filtered};
use crate::util::app_data_dir;

#[derive(Serialize)]
struct VersionChoice {
    id: String,
    label: String,
    kind: String,
}

#[derive(Serialize)]
struct BatchModBrief {
    key: String,
    name: String,
    icon: String,
    icon_data: String,
}

#[tauri::command]
pub async fn list_versions(
    source: String,
    project_id: String,
    mc_version: String,
    loader: String,
    cf_api_key: Option<String>,
    use_cache: Option<bool>,
) -> Result<serde_json::Value, String> {
    let res = || async {
        let use_cache = use_cache.unwrap_or(false);
        if source.to_lowercase() == "curseforge" {
            let api_key = crate::util::resolve_cf_api_key(cf_api_key.clone())?;
            let pid = project_id
                .parse::<u32>()
                .context("Project ID must be a number for CurseForge")?;
            if let Some(code) = crate::cf::cf_mod_loader_code_from_name(&loader) {
                let mut files =
                    crate::cf::get_cf_files_filtered(pid, &mc_version, code, &api_key, use_cache)
                        .await?;
                files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
                let mut choices: Vec<VersionChoice> = Vec::new();
                for f in files {
                    if !f.game_versions.iter().any(|v| v == &mc_version) {
                        continue;
                    }
                    let level = crate::util::release_type_str(f.release_type);
                    let name = crate::cf::strip_jar_suffix(&f.file_name);
                    choices.push(VersionChoice {
                        id: f.id.to_string(),
                        label: format!("{} ({})", name, level),
                        kind: level.to_string(),
                    });
                }
                Ok(json!({"choices": choices}))
            } else {
                let indexes = get_cf_latest_indexes(pid, &api_key).await?;
                let target_loader: String = crate::util::loader_name_to_tag(&loader);
                let mut items: Vec<(u8, VersionChoice)> = Vec::new();
                for idx in indexes {
                    let tag = idx
                        .mod_loader
                        .map(|code| crate::cf::cf_mod_loader_to_tag(code))
                        .unwrap_or("Unknown");
                    if idx.game_version != mc_version || tag != target_loader.as_str() {
                        continue;
                    }
                    let level = crate::util::release_type_str(idx.release_type);
                    let name = crate::cf::strip_jar_suffix(&idx.filename);
                    items.push((
                        idx.release_type,
                        VersionChoice {
                            id: idx.file_id.to_string(),
                            label: format!("{} ({})", name, level),
                            kind: level.to_string(),
                        },
                    ));
                }
                items.sort_by(|a, b| a.0.cmp(&b.0));
                let choices: Vec<VersionChoice> = items.into_iter().map(|(_, c)| c).collect();
                Ok(json!({"choices": choices}))
            }
        } else if source.to_lowercase() == "modrinth" {
            let mut versions =
                get_versions_filtered(&project_id, &mc_version, &loader, use_cache).await?;
            versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
            let loader_lower = loader.to_lowercase();
            let mut choices: Vec<VersionChoice> = Vec::new();
            for v in versions.into_iter() {
                if !v.game_versions.contains(&mc_version) {
                    continue;
                }
                if !v.loaders.iter().any(|l| l.to_lowercase() == loader_lower) {
                    continue;
                }
                choices.push(VersionChoice {
                    id: v.id.clone(),
                    label: format!("{} ({})", v.version_number, v.version_type),
                    kind: v.version_type,
                });
            }
            Ok(json!({"choices": choices}))
        } else {
            Err(anyhow!("Unknown source: {}", source))
        }
    };
    res().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_batch_mod_briefs(
    source: String,
    items: Vec<String>,
    cf_api_key: Option<String>,
) -> Result<serde_json::Value, String> {
    let res = || async {
        let mut mods: Vec<BatchModBrief> = Vec::new();
        if source.to_lowercase() == "curseforge" {
            let api_key = crate::util::resolve_cf_api_key(cf_api_key.clone())?;
            let tasks = items.into_iter().map(|it| {
                let api_key = api_key.clone();
                async move {
                    let pid = it.parse::<u32>()?;
                    let (name, icon_url) = crate::cf::get_cf_mod_brief(pid, &api_key).await?;
                    let icon_path = if let Some(url) = icon_url {
                        crate::util::cache_icon_from_url("cf", &pid.to_string(), &url).await?
                    } else {
                        String::new()
                    };
                    let icon_data = if icon_path.is_empty() {
                        String::new()
                    } else {
                        crate::util::file_to_data_url(std::path::Path::new(&icon_path))
                            .unwrap_or_default()
                    };
                    Ok::<BatchModBrief, anyhow::Error>(BatchModBrief {
                        key: pid.to_string(),
                        name,
                        icon: icon_path,
                        icon_data,
                    })
                }
            });
            let mut stream = stream::iter(tasks).buffer_unordered(4);
            while let Some(res) = stream.next().await {
                match res {
                    Ok(b) => mods.push(b),
                    Err(e) => return Err(e),
                }
            }
        } else if source.to_lowercase() == "modrinth" {
            let tasks = items.into_iter().map(|slug| async move {
                let (name, icon_url) = get_mr_mod_brief(&slug).await?;
                let icon_path = if let Some(url) = icon_url {
                    crate::util::cache_icon_from_url("mr", &slug, &url).await?
                } else {
                    String::new()
                };
                let icon_data = if icon_path.is_empty() {
                    String::new()
                } else {
                    crate::util::file_to_data_url(std::path::Path::new(&icon_path))
                        .unwrap_or_default()
                };
                Ok::<BatchModBrief, anyhow::Error>(BatchModBrief {
                    key: slug,
                    name,
                    icon: icon_path,
                    icon_data,
                })
            });
            let mut stream = stream::iter(tasks).buffer_unordered(4);
            while let Some(res) = stream.next().await {
                match res {
                    Ok(b) => mods.push(b),
                    Err(e) => return Err(e),
                }
            }
        } else {
            return Err(anyhow!("Unknown source: {}", source));
        }
        Ok(json!({"mods": mods}))
    };
    res().await.map_err(|e| e.to_string())
}

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
    let mut gradle_content = fs::read_to_string(gradle_path)
        .await
        .context("Could not read build.gradle")?;
    if source.to_lowercase() == "curseforge" {
        let api_key = crate::util::resolve_cf_api_key(cf_api_key.clone())?;
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
        fs::write(gradle_path, gradle_content)
            .await
            .context("Failed to write build.gradle")?;
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
        fs::write(gradle_path, gradle_content)
            .await
            .context("Failed to write build.gradle")?;
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
pub async fn apply_selected_version(
    gradle_path: String,
    source: String,
    project_id: String,
    loader: String,
    selected_id: String,
    cf_api_key: Option<String>,
) -> Result<String, String> {
    let res = || async {
        let gradle_path_p = Path::new(&gradle_path);
        if !gradle_path_p.exists() {
            return Err(anyhow!(
                "Build.gradle file not found at {:?}",
                gradle_path_p
            ));
        }
        let mut gradle_content = fs::read_to_string(gradle_path_p)
            .await
            .context("Could not read build.gradle")?;
        if source.to_lowercase() == "curseforge" {
            let api_key = crate::util::resolve_cf_api_key(cf_api_key.clone())?;
            let pid = project_id
                .parse::<u32>()
                .context("Project ID must be a number for CurseForge")?;
            let (slug, modid_num) = get_project_meta(pid, &api_key).await?;
            let file_id = selected_id
                .parse::<u32>()
                .context("Selected ID must be a number for CurseForge")?;
            gradle_content = ensure_curse_maven_repo(&gradle_content);
            let dep_line = generate_dep(&loader, &slug, &modid_num.to_string(), file_id)?;
            gradle_content =
                update_or_insert_dependency(&gradle_content, &modid_num.to_string(), &dep_line);
            fs::write(gradle_path_p, gradle_content)
                .await
                .context("Failed to write build.gradle")?;
            Ok(format!(
                "✅ Updated Dependency: {}\n🎉 Applied File ID: {}",
                dep_line, file_id
            ))
        } else if source.to_lowercase() == "modrinth" {
            gradle_content = ensure_modrinth_maven_repo(&gradle_content);
            let dep_line = generate_mr_dep(&loader, &project_id, &selected_id)?;
            gradle_content =
                update_or_insert_dependency_mr(&gradle_content, &project_id, &dep_line);
            fs::write(gradle_path_p, gradle_content)
                .await
                .context("Failed to write build.gradle")?;
            Ok(format!(
                "✅ Updated Dependency: {}\n🎉 Applied Version ID: {}",
                dep_line, selected_id
            ))
        } else {
            Err(anyhow!("Unknown source: {}", source))
        }
    };
    res().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_project_options(
    source: String,
    project_id: String,
    cf_api_key: Option<String>,
) -> Result<serde_json::Value, String> {
    let res = || async {
        let use_cache = true;
        if source.to_lowercase() == "curseforge" {
            let api_key = crate::util::resolve_cf_api_key(cf_api_key.clone())?;
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
            let versions = get_versions(&project_id, use_cache).await?;
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
    let mut out = String::new();
    for item in items.into_iter() {
        match process_update(
            gradle_path.clone(),
            item.clone(),
            mc_version.clone(),
            loader.clone(),
            source.clone(),
            cf_api_key.clone(),
        )
        .await
        {
            Ok(res) => out.push_str(&format!("\n[{}] {}\n", item, res)),
            Err(err) => out.push_str(&format!("\n[{}] ❌ {}\n", item, err)),
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn save_log(content: String) -> Result<String, String> {
    let base = app_data_dir().join("logs");
    if let Err(e) = std::fs::create_dir_all(&base) {
        return Err(e.to_string());
    }
    let ts = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let path = base.join(format!("log-{}.txt", ts));
    let backend_path = base.join("runtime.log");
    let backend_content = std::fs::read_to_string(&backend_path).unwrap_or_default();
    let combined = format!(
        "[FRONTEND]\n{}\n\n[BACKEND]\n{}\n",
        content, backend_content
    );
    std::fs::write(&path, combined).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into())
}

#[tauri::command]
pub async fn clear_all_caches() -> Result<(), String> {
    crate::cache::clear_all_cache().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn refresh_mojang_cache() -> Result<(), String> {
    crate::mojang::refresh_manifest_cache_on_startup()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_log_dir() -> Result<String, String> {
    let base = app_data_dir().join("logs");
    if let Err(e) = std::fs::create_dir_all(&base) {
        return Err(e.to_string());
    }
    Ok(base.to_string_lossy().into())
}

#[tauri::command]
pub async fn apply_selected_versions_batch(
    gradle_path: String,
    source: String,
    selections: Vec<(String, String)>,
    loader: String,
    cf_api_key: Option<String>,
) -> Result<String, String> {
    let res = || async {
        let gradle_path_p = Path::new(&gradle_path);
        if !gradle_path_p.exists() {
            return Err(anyhow!(format!(
                "Build.gradle file not found at {:?}",
                gradle_path_p
            )));
        }
        let mut gradle_content = fs::read_to_string(gradle_path_p)
            .await
            .context("Could not read build.gradle")?;
        let mut summary = String::new();

        if source.to_lowercase() == "curseforge" {
            let api_key = crate::util::resolve_cf_api_key(cf_api_key.clone())?;
            gradle_content = ensure_curse_maven_repo(&gradle_content);
            for (pid_s, selected_id_s) in selections.iter() {
                let pid = pid_s
                    .parse::<u32>()
                    .context("Project ID must be a number for CurseForge")?;
                let file_id = selected_id_s
                    .parse::<u32>()
                    .context("Selected ID must be a number for CurseForge")?;
                let (slug, modid_num) = get_project_meta(pid, &api_key).await?;
                let dep_line = generate_dep(&loader, &slug, &modid_num.to_string(), file_id)?;
                gradle_content =
                    update_or_insert_dependency(&gradle_content, &modid_num.to_string(), &dep_line);
                summary.push_str(&format!("✅ {} → File ID: {}\n", dep_line, file_id));
            }
        } else if source.to_lowercase() == "modrinth" {
            gradle_content = ensure_modrinth_maven_repo(&gradle_content);
            for (slug, ver_id) in selections.iter() {
                let dep_line = generate_mr_dep(&loader, slug, ver_id)?;
                gradle_content = update_or_insert_dependency_mr(&gradle_content, slug, &dep_line);
                summary.push_str(&format!("✅ {} → Version ID: {}\n", dep_line, ver_id));
            }
        } else {
            return Err(anyhow!("Unknown source: {}", source));
        }

        fs::write(gradle_path_p, gradle_content)
            .await
            .context("Failed to write build.gradle")?;
        Ok(summary)
    };
    res().await.map_err(|e| e.to_string())
}
