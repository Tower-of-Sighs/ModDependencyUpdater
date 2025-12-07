use anyhow::{anyhow, Context};
use regex::Regex;
use serde::Deserialize;
use serde_json::json;
use std::fs;
use std::path::Path;

// ========== Data Structures ==========

#[derive(Deserialize, Debug)]
struct CfFile {
    id: u32,
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "fileName")]
    file_name: String,
    #[serde(rename = "releaseType")]
    release_type: u8, // 1: Release, 2: Beta, 3: Alpha
    #[serde(rename = "gameVersions")]
    game_versions: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct CfModResponse {
    data: CfModData,
}

#[derive(Deserialize, Debug)]
struct CfModData {
    id: u32,
    slug: String,
}

#[derive(Deserialize, Debug)]
struct CfFilesResponse {
    data: Vec<CfFile>,
}

#[derive(Deserialize, Debug)]
struct MrVersion {
    id: String,
    version_number: String,
    version_type: String, // release, beta, alpha
    game_versions: Vec<String>,
    loaders: Vec<String>,
    date_published: String,
}


fn extract_version(text: &str) -> Option<String> {
    let re = Regex::new(r"\d+(?:\.\d+)*(?:[-+][a-zA-Z0-9_.-]+)?").unwrap();
    for cap in re.captures_iter(text) {
        let match_str = cap.get(0)?.as_str();
        if match_str.contains('.') {
            return Some(match_str.to_string());
        }
    }
    None
}

async fn get_project_meta(project_id: u32, api_key: &str) -> anyhow::Result<(String, u32)> {
    let client = reqwest::Client::new();
    let url = format!("https://api.curseforge.com/v1/mods/{}", project_id);
    let resp = client
        .get(&url)
        .header("x-api-key", api_key)
        .header("Accept", "application/json")
        .send()
        .await
        .context("Failed to connect to CurseForge API")?;
    
    if !resp.status().is_success() {
        return Err(anyhow!("CurseForge API Error: {}", resp.status()));
    }

    let body: CfModResponse = resp.json().await.context("Failed to parse CurseForge response")?;
    Ok((body.data.slug, body.data.id))
}

async fn get_latest_cf_file(
    project_id: u32,
    mc_version: &str,
    loader: &str,
    api_key: &str,
) -> anyhow::Result<(Option<u32>, Option<String>, Option<u8>)> {
    let client = reqwest::Client::new();
    let url = format!("https://api.curseforge.com/v1/mods/{}/files", project_id);
    let resp = client
        .get(&url)
        .header("x-api-key", api_key)
        .header("Accept", "application/json")
        .send()
        .await
        .context("Failed to fetch files from CurseForge")?;

    if !resp.status().is_success() {
        return Err(anyhow!("CurseForge API Error (Files): {}", resp.status()));
    }

    let body: CfFilesResponse = resp.json().await.context("Failed to parse files response")?;
    let mut files = body.data;

    // Sort by ID descending (newest first)
    files.sort_by(|a, b| b.id.cmp(&a.id));

    let loader_tag = match loader.to_lowercase().as_str() {
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        "fabric" => "Fabric",
        _ => loader, // Fallback to capitalizing if needed, or just passing raw
    };
    // Simple capitalization for fallback if not in map
    let loader_tag_fallback = if !["Forge", "NeoForge", "Fabric"].contains(&loader_tag) {
        let mut chars = loader.chars();
        match chars.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
        }
    } else {
        loader_tag.to_string()
    };
    let target_loader = if ["Forge", "NeoForge", "Fabric"].contains(&loader_tag) { loader_tag } else { &loader_tag_fallback };


    for release_type in [1, 2, 3] { // Release, Beta, Alpha
        for f in &files {
            if f.release_type != release_type {
                continue;
            }
            if f.game_versions.iter().any(|v| v == mc_version) && f.game_versions.iter().any(|v| v == target_loader) {
                let version = extract_version(&f.display_name)
                    .or_else(|| extract_version(&f.file_name))
                    .unwrap_or_else(|| f.id.to_string());
                return Ok((Some(f.id), Some(version), Some(f.release_type)));
            }
        }
    }

    Ok((None, None, None))
}

async fn get_cf_files(project_id: u32, api_key: &str) -> anyhow::Result<Vec<CfFile>> {
    let client = reqwest::Client::new();
    let url = format!("https://api.curseforge.com/v1/mods/{}/files", project_id);
    let resp = client
        .get(&url)
        .header("x-api-key", api_key)
        .header("Accept", "application/json")
        .send()
        .await
        .context("Failed to fetch files from CurseForge")?;
    if !resp.status().is_success() {
        return Err(anyhow!("CurseForge API Error (Files): {}", resp.status()));
    }
    let body: CfFilesResponse = resp.json().await.context("Failed to parse files response")?;
    Ok(body.data)
}

async fn get_latest_mr_version(
    project_slug: &str,
    mc_version: &str,
    loader: &str,
) -> anyhow::Result<(Option<String>, Option<String>, Option<String>)> {
    let client = reqwest::Client::builder()
        .user_agent("ModDependencyUpdater/1.0 (Tauri)")
        .build()?;
    
    let url = format!("https://api.modrinth.com/v2/project/{}/version", project_slug);
    let resp = client
        .get(&url)
        .send()
        .await
        .context("Failed to connect to Modrinth API")?;

    if !resp.status().is_success() {
        return Err(anyhow!("Modrinth API Error: {}", resp.status()));
    }

    let mut versions: Vec<MrVersion> = resp.json().await.context("Failed to parse Modrinth response")?;
    // Sort by date published desc
    versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));

    let priority_order = ["release", "beta", "alpha"];
    let loader_lower = loader.to_lowercase();

    for vtype in priority_order {
        for ver in &versions {
            if ver.version_type != vtype {
                continue;
            }
            if ver.game_versions.contains(&mc_version.to_string()) 
               && ver.loaders.iter().any(|l| l.to_lowercase() == loader_lower) {
                return Ok((Some(ver.id.clone()), Some(ver.version_number.clone()), Some(ver.version_type.clone())));
            }
        }
    }

    Ok((None, None, None))
}

fn ensure_curse_maven_repo(build_gradle: &str) -> String {
    if build_gradle.contains("https://www.cursemaven.com") {
        return build_gradle.to_string();
    }

    let repo_block = r#"
repositories {
    maven {
        name = "Curse Maven"
        url = "https://www.cursemaven.com"
        content {
            includeGroup "curse.maven"
        }
    }
}
"#;

    if build_gradle.contains("repositories {") {
        build_gradle.replacen("repositories {", repo_block, 1)
    } else {
        format!("{}\n{}", repo_block, build_gradle)
    }
}

fn ensure_modrinth_maven_repo(build_gradle: &str) -> String {
    if build_gradle.contains("https://api.modrinth.com/maven") {
        return build_gradle.to_string();
    }

    let repo_block = r#"
repositories {
    maven {
        name = "Modrinth"
        url = "https://api.modrinth.com/maven"
    }
}
"#;

    if build_gradle.contains("repositories {") {
        build_gradle.replacen("repositories {", repo_block, 1)
    } else {
        format!("{}\n{}", repo_block, build_gradle)
    }
}

fn generate_dep(loader: &str, slug: &str, modid: &str, file_id: u32) -> anyhow::Result<String> {
    let coordinate = format!("curse.maven:{}-{}:{}", slug, modid, file_id);
    match loader.to_lowercase().as_str() {
        "forge" => Ok(format!("implementation fg.deobf(\"{}\")", coordinate)),
        "fabric" => Ok(format!("modImplementation \"{}\"", coordinate)),
        "neoforge" => Ok(format!("implementation \"{}\"", coordinate)),
        _ => Err(anyhow!("Unknown loader: {}", loader)),
    }
}

fn generate_mr_dep(loader: &str, slug: &str, version_id: &str) -> anyhow::Result<String> {
    let coordinate = format!("maven.modrinth:{}:{}", slug, version_id);
    match loader.to_lowercase().as_str() {
        "forge" => Ok(format!("implementation fg.deobf(\"{}\")", coordinate)),
        "fabric" => Ok(format!("modImplementation \"{}\"", coordinate)),
        "neoforge" => Ok(format!("implementation \"{}\"", coordinate)),
        _ => Err(anyhow!("Unknown loader: {}", loader)),
    }
}

fn update_or_insert_dependency(build_gradle: &str, modid: &str, dep_line: &str) -> String {
    // Regex to find existing dependency
    // Pattern: .*curse\.maven:.*-{modid}:\d+.*
    let pattern_str = format!(r".*curse\.maven:.*-{}:\d+.*", regex::escape(modid));
    let pattern = Regex::new(&pattern_str).unwrap();

    if pattern.is_match(build_gradle) {
        pattern.replace(build_gradle, dep_line).to_string()
    } else {
        if build_gradle.contains("dependencies {") {
            build_gradle.replacen("dependencies {", &format!("dependencies {{\n    {}", dep_line), 1)
        } else {
            format!("{}\ndependencies {{\n    {}\n}}\n", build_gradle, dep_line)
        }
    }
}

fn update_or_insert_dependency_mr(build_gradle: &str, project_slug: &str, dep_line: &str) -> String {
    // Regex: .*maven\.modrinth:{slug}:[A-Za-z0-9]+.*
    let pattern_str = format!(r".*maven\.modrinth:{}:[A-Za-z0-9]+.*", regex::escape(project_slug));
    let pattern = Regex::new(&pattern_str).unwrap();

    if pattern.is_match(build_gradle) {
        pattern.replace(build_gradle, dep_line).to_string()
    } else {
        if build_gradle.contains("dependencies {") {
            build_gradle.replacen("dependencies {", &format!("dependencies {{\n    {}", dep_line), 1)
        } else {
            format!("{}\ndependencies {{\n    {}\n}}\n", build_gradle, dep_line)
        }
    }
}


// ========== Command ==========

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
        .context("Could not read build.gradle")?;

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
        let api_key = api_key.ok_or_else(|| anyhow!("CF_API_KEY is required for CurseForge (Input or Env Var)"))?;

        let pid = project_id.parse::<u32>().context("Project ID must be a number for CurseForge")?;

        let (slug, modid_num) = get_project_meta(pid, &api_key).await?;

        let (file_id, version, level) = get_latest_cf_file(pid, &mc_version, &loader, &api_key).await?;
        let file_id = file_id.ok_or_else(|| anyhow!("No matching CurseForge file found for MC {} / {}", mc_version, loader))?;

        let level_msg = match level {
            Some(2) => "⚠ Beta Build used\n",
            Some(3) => "⚠ Alpha Build used\n",
            _ => "",
        };

        gradle_content = ensure_curse_maven_repo(&gradle_content);
        let dep_line = generate_dep(&loader, &slug, &modid_num.to_string(), file_id)?;
        gradle_content = update_or_insert_dependency(&gradle_content, &modid_num.to_string(), &dep_line);

        fs::write(gradle_path, gradle_content).context("Failed to write build.gradle")?;

        Ok(format!("{}✅ Updated Dependency: {}\n🎉 New Version: {} (File ID: {})", level_msg, dep_line, version.unwrap_or_default(), file_id))
    } else if source.to_lowercase() == "modrinth" {
        let (ver_id, version, level) = get_latest_mr_version(&project_id, &mc_version, &loader).await?;
        let ver_id = ver_id.ok_or_else(|| anyhow!("No matching Modrinth version found for MC {} / {}", mc_version, loader))?;

        let level_msg = match level.as_deref() {
            Some("beta") => "⚠ Beta Build used\n",
            Some("alpha") => "⚠ Alpha Build used\n",
            _ => "",
        };

        gradle_content = ensure_modrinth_maven_repo(&gradle_content);
        let dep_line = generate_mr_dep(&loader, &project_id, &ver_id)?;
        gradle_content = update_or_insert_dependency_mr(&gradle_content, &project_id, &dep_line);

        fs::write(gradle_path, gradle_content).context("Failed to write build.gradle")?;
        Ok(format!("{}✅ Updated Dependency: {}\n🎉 New Version: {} (Version ID: {})", level_msg, dep_line, version.unwrap_or_default(), ver_id))
    } else {
        Err(anyhow!("Unknown source: {}", source))
    }
}

#[tauri::command]
async fn update_dependency(
    gradle_path: String,
    project_id: String,
    mc_version: String,
    loader: String,
    source: String,
    cf_api_key: Option<String>,
) -> Result<String, String> {
    process_update(gradle_path, project_id, mc_version, loader, source, cf_api_key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_project_options(
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
            let api_key = api_key.ok_or_else(|| anyhow!("CF_API_KEY is required for CurseForge (Input or Env Var)"))?;
            let pid = project_id.parse::<u32>().context("Project ID must be a number for CurseForge")?;
            let files = get_cf_files(pid, &api_key).await?;
            let mut versions_set = std::collections::BTreeSet::new();
            let mut loaders_set = std::collections::BTreeSet::new();
            for f in files {
                for v in f.game_versions {
                    versions_set.insert(v.clone());
                    match v.as_str() {
                        "Forge" | "NeoForge" | "Fabric" => { loaders_set.insert(v.clone()); }
                        _ => {}
                    }
                }
            }
            let versions: Vec<String> = versions_set.into_iter().filter(|v| v.chars().next().map(|c| c.is_numeric()).unwrap_or(false)).collect();
            let loaders: Vec<String> = loaders_set.into_iter().collect();
            Ok(json!({"versions": versions, "loaders": loaders, "id": pid}))
        } else if source.to_lowercase() == "modrinth" {
            let client = reqwest::Client::builder().user_agent("ModDependencyUpdater/1.0 (Tauri)").build()?;
            let url = format!("https://api.modrinth.com/v2/project/{}/version", project_id);
            let resp = client.get(&url).send().await.context("Failed to connect to Modrinth API")?;
            if !resp.status().is_success() {
                return Err(anyhow!("Modrinth API Error: {}", resp.status()));
            }
            let versions: Vec<MrVersion> = resp.json().await.context("Failed to parse Modrinth response")?;
            let mut vset = std::collections::BTreeSet::new();
            let mut lset = std::collections::BTreeSet::new();
            for v in versions {
                for gv in v.game_versions { vset.insert(gv); }
                for ld in v.loaders { lset.insert(ld.to_lowercase()); }
            }
            let mut loaders: Vec<String> = lset.into_iter().collect();
            for l in &mut loaders { *l = match l.as_str() { "forge" => "Forge".to_string(), "neoforge" => "NeoForge".to_string(), "fabric" => "Fabric".to_string(), other => other.to_string() }; }
            Ok(json!({"versions": vset.into_iter().collect::<Vec<_>>(), "loaders": loaders, "slug": project_id}))
        } else {
            Err(anyhow!("Unknown source: {}", source))
        }
    };
    res().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_dependencies_batch(
    gradle_path: String,
    source: String,
    items: Vec<String>,
    mc_version: String,
    loader: String,
    cf_api_key: Option<String>,
) -> Result<String, String> {
    let mut output = String::new();
    for item in items {
        match process_update(gradle_path.clone(), item.clone(), mc_version.clone(), loader.clone(), source.clone(), cf_api_key.clone()).await {
            Ok(res) => {
                output.push_str(&format!("\n[{}] {}\n", item, res));
            }
            Err(err) => {
                output.push_str(&format!("\n[{}] ❌ {}\n", item, err));
            }
        }
    }
    Ok(output)
}

#[tauri::command]
async fn save_log(content: String) -> Result<String, String> {
    let base = std::path::PathBuf::from("logs");
    if let Err(e) = std::fs::create_dir_all(&base) { return Err(e.to_string()); }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    let path = base.join(format!("log-{}.txt", ts));
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![update_dependency, get_project_options, update_dependencies_batch, save_log])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
