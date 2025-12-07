use anyhow::anyhow;
use regex::Regex;

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

    // 尝试在已有的 repositories 块中插入
    let repo_pattern = Regex::new(r"(?m)^\s*repositories\s*\{").unwrap();
    if let Some(mat) = repo_pattern.find(build_gradle) {
        let start = mat.end();
        // 找到对应的闭合 '}'
        let mut brace_count = 1;
        let mut pos = start;
        let chars: Vec<char> = build_gradle.chars().collect();
        while pos < chars.len() && brace_count > 0 {
            if chars[pos] == '{' {
                brace_count += 1;
            } else if chars[pos] == '}' {
                brace_count -= 1;
            }
            pos += 1;
        }
        let end = if brace_count == 0 {
            pos - 1
        } else {
            build_gradle.len()
        };

        let before = &build_gradle[..start];
        let inside = &build_gradle[start..end];
        let after = &build_gradle[end..];

        // 插入到 repositories 块内部（在已有内容后）
        return format!("{}{}\n{}\n{}", before, inside, curse_repo, after);
    }

    // 如果没有 repositories 块，创建一个新的（放在文件顶部或 plugins 之后）
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

    let repo_pattern = Regex::new(r"(?m)^\s*repositories\s*\{").unwrap();
    if let Some(mat) = repo_pattern.find(build_gradle) {
        let start = mat.end();
        let mut brace_count = 1;
        let mut pos = start;
        let chars: Vec<char> = build_gradle.chars().collect();
        while pos < chars.len() && brace_count > 0 {
            if chars[pos] == '{' {
                brace_count += 1;
            } else if chars[pos] == '}' {
                brace_count -= 1;
            }
            pos += 1;
        }
        let end = if brace_count == 0 {
            pos - 1
        } else {
            build_gradle.len()
        };

        let before = &build_gradle[..start];
        let inside = &build_gradle[start..end];
        let after = &build_gradle[end..];

        return format!("{}{}\n{}\n{}", before, inside, modrinth_repo, after);
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
        "fabric" => Ok(format!("    modImplementation \"{}\"", coordinate)),
        "neoforge" => Ok(format!("    implementation \"{}\"", coordinate)),
        _ => Err(anyhow!("Unknown loader: {}", loader)),
    }
}

pub fn generate_mr_dep(loader: &str, slug: &str, version_id: &str) -> anyhow::Result<String> {
    let coordinate = format!("maven.modrinth:{}:{}", slug, version_id);
    match loader.to_lowercase().as_str() {
        "forge" => Ok(format!("    implementation fg.deobf(\"{}\")", coordinate)),
        "fabric" => Ok(format!("    modImplementation \"{}\"", coordinate)),
        "neoforge" => Ok(format!("    implementation \"{}\"", coordinate)),
        _ => Err(anyhow!("Unknown loader: {}", loader)),
    }
}

fn insert_into_dependencies_block(build_gradle: &str, dep_line: &str) -> String {
    let dep_pattern = Regex::new(r"(?m)^\s*dependencies\s*\{").unwrap();
    if let Some(mat) = dep_pattern.find(build_gradle) {
        let start = mat.end();
        let mut brace_count = 1;
        let mut pos = start;
        let chars: Vec<char> = build_gradle.chars().collect();
        while pos < chars.len() && brace_count > 0 {
            if chars[pos] == '{' {
                brace_count += 1;
            } else if chars[pos] == '}' {
                brace_count -= 1;
            }
            pos += 1;
        }
        let end = if brace_count == 0 {
            pos - 1
        } else {
            build_gradle.len()
        };

        let before = &build_gradle[..start];
        let inside = &build_gradle[start..end];
        let after = &build_gradle[end..];

        return format!("{}{}\n{}\n{}", before, inside, dep_line, after);
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
        // 替换整行
        let start = build_gradle[..mat.start()].rfind('\n').map_or(0, |i| i + 1);
        let end = build_gradle[mat.end()..]
            .find('\n')
            .map_or(build_gradle.len(), |i| mat.end() + i);
        let before = &build_gradle[..start];
        let after = &build_gradle[end..];
        return format!("{}{}\n{}", before, dep_line, after);
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
        let start = build_gradle[..mat.start()].rfind('\n').map_or(0, |i| i + 1);
        let end = build_gradle[mat.end()..]
            .find('\n')
            .map_or(build_gradle.len(), |i| mat.end() + i);
        let before = &build_gradle[..start];
        let after = &build_gradle[end..];
        return format!("{}{}\n{}", before, dep_line, after);
    }

    insert_into_dependencies_block(build_gradle, dep_line)
}
