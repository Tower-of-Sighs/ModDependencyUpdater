use anyhow::anyhow;
use once_cell::sync::Lazy;
use regex::Regex;
static RE_REPOSITORIES: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*repositories\s*\{").unwrap());
static RE_DEPENDENCIES: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*dependencies\s*\{").unwrap());

fn find_top_level_block_range(src: &str, re: &Regex) -> Option<(usize, usize)> {
    for mat in re.find_iter(src) {
        let mut depth = 0usize;
        for b in src[..mat.start()].bytes() {
            if b == b'{' {
                depth += 1;
            } else if b == b'}' && depth > 0 {
                depth -= 1;
            }
        }
        if depth == 0 {
            let mut brace = 1usize;
            let mut pos = mat.end();
            let bytes = src.as_bytes();
            while pos < bytes.len() && brace > 0 {
                if bytes[pos] == b'{' {
                    brace += 1;
                } else if bytes[pos] == b'}' {
                    brace -= 1;
                }
                pos += 1;
            }
            let end = if brace == 0 { pos - 1 } else { src.len() };
            return Some((mat.end(), end));
        }
    }
    None
}

pub fn ensure_curse_maven_repo(build_gradle: &str) -> String {
    if build_gradle.contains("https://cursemaven.com") || build_gradle.contains("curse.maven") {
        return build_gradle.to_string();
    }

    let curse_repo = r#"    maven {
        name = "Curse Maven"
        url = "https://cursemaven.com"
        content {
            includeGroup "curse.maven"
        }
    }"#;

    if let Some((start, end)) = find_top_level_block_range(build_gradle, &RE_REPOSITORIES) {
        let before = &build_gradle[..start];
        let inside = &build_gradle[start..end];
        let after = &build_gradle[end..];
        let prefix = if inside.ends_with('\n') { "" } else { "\n" };
        return format!("{}{}{}{}\n{}", before, inside, prefix, curse_repo, after);
    }

    if build_gradle.trim_start().starts_with("plugins {") {
        // 插入在 plugins 块之后
        let plugins_end = build_gradle.find('}').map(|i| i + 1).unwrap_or(0);
        let (before, after) = build_gradle.split_at(plugins_end);
        return format!(
            "{}\n\nrepositories {{\n{}\n}}\n\n{}",
            before, curse_repo, after
        );
    } else {
        // 插入在最前面
        return format!("repositories {{\n{}\n}}\n\n{}", curse_repo, build_gradle);
    }
}

pub fn ensure_modrinth_maven_repo(build_gradle: &str) -> String {
    if build_gradle.contains("https://api.modrinth.com/maven") {
        return build_gradle.to_string();
    }

    let modrinth_repo = r#"    maven {
        name = "Modrinth"
        url = "https://api.modrinth.com/maven"
    }"#;

    if let Some((start, end)) = find_top_level_block_range(build_gradle, &RE_REPOSITORIES) {
        let before = &build_gradle[..start];
        let inside = &build_gradle[start..end];
        let after = &build_gradle[end..];
        let prefix = if inside.ends_with('\n') { "" } else { "\n" };
        return format!("{}{}{}{}\n{}", before, inside, prefix, modrinth_repo, after);
    }

    if build_gradle.trim_start().starts_with("plugins {") {
        let plugins_end = build_gradle.find('}').map(|i| i + 1).unwrap_or(0);
        let (before, after) = build_gradle.split_at(plugins_end);
        return format!(
            "{}\n\nrepositories {{\n{}\n}}\n\n{}",
            before, modrinth_repo, after
        );
    } else {
        return format!("repositories {{\n{}\n}}\n\n{}", modrinth_repo, build_gradle);
    }
}

pub fn generate_dep(loader: &str, slug: &str, modid: &str, file_id: u32) -> anyhow::Result<String> {
    let coordinate = format!("curse.maven:{}-{}:{}", slug, modid, file_id);
    match loader.to_lowercase().as_str() {
        "forge" => Ok(format!("    implementation fg.deobf(\"{}\")", coordinate)),
        "fabric" | "quilt" => Ok(format!("    modImplementation \"{}\"", coordinate)),
        "neoforge" => Ok(format!("    implementation \"{}\"", coordinate)),
        _ => Err(anyhow!("Unknown loader: {}", loader)),
    }
}

pub fn generate_mr_dep(loader: &str, slug: &str, version_id: &str) -> anyhow::Result<String> {
    let coordinate = format!("maven.modrinth:{}:{}", slug, version_id);
    match loader.to_lowercase().as_str() {
        "forge" => Ok(format!("    implementation fg.deobf(\"{}\")", coordinate)),
        "fabric" | "quilt" => Ok(format!("    modImplementation \"{}\"", coordinate)),
        "neoforge" => Ok(format!("    implementation \"{}\"", coordinate)),
        _ => Err(anyhow!("Unknown loader: {}", loader)),
    }
}

fn insert_into_dependencies_block(build_gradle: &str, dep_line: &str) -> String {
    if let Some((start, end)) = find_top_level_block_range(build_gradle, &RE_DEPENDENCIES) {
        let before = &build_gradle[..start];
        let inside = &build_gradle[start..end];
        let after = &build_gradle[end..];
        let prefix = if inside.ends_with('\n') { "" } else { "\n" };
        return format!("{}{}{}{}\n{}", before, inside, prefix, dep_line, after);
    }

    // 没有 dependencies 块，创建一个（放在最后）
    format!("{}\ndependencies {{\n{}\n}}\n", build_gradle, dep_line)
}

pub fn update_or_insert_dependency(build_gradle: &str, modid: &str, dep_line: &str) -> String {
    let pattern_str = format!(
        r#"(?m)^\s*.*curse\.maven:[^:]*-{}:\d+.*$"#,
        regex::escape(modid)
    );
    let pattern = Regex::new(&pattern_str).unwrap();

    if let Some(mat) = pattern.find(build_gradle) {
        let before = &build_gradle[..mat.start()];
        let line = &build_gradle[mat.start()..mat.end()];
        let after = &build_gradle[mat.end()..];
        let id_re_str = format!(r"(curse\.maven:[^:]*-{}:)(\d+)", regex::escape(modid));
        let id_re = Regex::new(&id_re_str).unwrap();
        let new_id = Regex::new(r"curse\\.maven:[^:]+-\d+:(\d+)")
            .unwrap()
            .captures(dep_line)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());
        if let Some(nid) = new_id {
            let replaced = id_re.replace(line, format!("$1{}", nid)).to_string();
            return format!("{}{}{}", before, replaced, after);
        }
        return format!("{}{}{}", before, dep_line, after);
    }

    insert_into_dependencies_block(build_gradle, dep_line)
}

pub fn update_or_insert_dependency_mr(
    build_gradle: &str,
    project_slug: &str,
    dep_line: &str,
) -> String {
    let pattern_str = format!(
        r#"(?m)^\s*.*maven\.modrinth:{}:[A-Za-z0-9.-]+.*$"#,
        regex::escape(project_slug)
    );
    let pattern = Regex::new(&pattern_str).unwrap();

    if let Some(mat) = pattern.find(build_gradle) {
        let before = &build_gradle[..mat.start()];
        let line = &build_gradle[mat.start()..mat.end()];
        let after = &build_gradle[mat.end()..];
        let id_re_str = format!(
            r"(maven\.modrinth:{}:)[A-Za-z0-9.-]+",
            regex::escape(project_slug)
        );
        let id_re = Regex::new(&id_re_str).unwrap();
        let new_id = Regex::new(r"maven\\.modrinth:[^:]+:([A-Za-z0-9.-]+)")
            .unwrap()
            .captures(dep_line)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());
        if let Some(nid) = new_id {
            let replaced = id_re.replace(line, format!("$1{}", nid)).to_string();
            return format!("{}{}{}", before, replaced, after);
        }
        return format!("{}{}{}", before, dep_line, after);
    }

    insert_into_dependencies_block(build_gradle, dep_line)
}
