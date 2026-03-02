use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ratkit::widgets::markdown_preview::SourceState;
use serde::Deserialize;

pub const ROOT_AGENTS_PATH: &str = ".agents";
pub const DEFAULT_SKILL_PATH: &str = ".agents/skills/ratkit/SKILL.md";

#[derive(Clone, Debug)]
pub struct SkillTreeNode {
    pub dir_name: String,
    pub display_name: String,
    pub skill_file: Option<PathBuf>,
    pub children: Vec<SkillTreeNode>,
}

pub fn load_source_from_path(path: impl AsRef<Path>) -> io::Result<SourceState> {
    let path = path.as_ref();

    let mut source = SourceState::default();
    source.set_source_file(path)?;
    Ok(source)
}

pub fn fallback_title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Preview")
        .to_string()
}

pub fn preview_relative_to_skills(path: &Path) -> String {
    let skills_root = PathBuf::from(ROOT_AGENTS_PATH).join("skills");
    if let Ok(relative) = path.strip_prefix(&skills_root) {
        return relative.display().to_string();
    }
    if let Ok(relative) = path.strip_prefix(PathBuf::from(ROOT_AGENTS_PATH)) {
        return relative.display().to_string();
    }
    let global_agents_root = global_agents_skill_root();
    if let Ok(relative) = path.strip_prefix(&global_agents_root) {
        return relative.display().to_string();
    }
    for (provider, root) in provider_global_skill_roots() {
        if let Ok(relative) = path.strip_prefix(&root) {
            return format!("{}/{}", provider, relative.display());
        }
    }
    path.display().to_string()
}

pub fn extract_skill_name_from_frontmatter(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }

    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("name:") {
            let value = rest.trim().trim_matches('"').trim_matches('"');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub fn skill_node_at_path<'a>(
    nodes: &'a [SkillTreeNode],
    path: &[usize],
) -> Option<&'a SkillTreeNode> {
    let mut current_nodes = nodes;
    let mut current_node: Option<&SkillTreeNode> = None;
    for idx in path {
        let node = current_nodes.get(*idx)?;
        current_node = Some(node);
        current_nodes = &node.children;
    }
    current_node
}

pub fn collect_expanded_skill_paths(
    nodes: &[SkillTreeNode],
    base: &mut Vec<usize>,
    expanded: &mut HashSet<Vec<usize>>,
) {
    for (idx, node) in nodes.iter().enumerate() {
        base.push(idx);
        expanded.insert(base.clone());
        collect_expanded_skill_paths(&node.children, base, expanded);
        let _ = base.pop();
    }
}

#[derive(Debug, Deserialize)]
struct SkillsLockFile {
    skills: HashMap<String, SkillsLockEntry>,
}

#[derive(Debug, Deserialize)]
struct SkillsLockEntry {
    source: String,
}

pub fn load_skill_slug_from_lock(skill_file: &Path, skill_name: &str) -> Option<String> {
    for ancestor in skill_file.ancestors() {
        let lock_path = ancestor.join("skills-lock.json");
        if !lock_path.exists() {
            continue;
        }

        let raw = fs::read_to_string(&lock_path).ok()?;
        let parsed: SkillsLockFile = serde_json::from_str(&raw).ok()?;
        let entry = parsed.skills.get(skill_name)?;
        return Some(format!("{}/{}", entry.source, skill_name));
    }

    None
}

pub fn skill_remove_target_from_path(path: &Path) -> String {
    let skill_parent = path.parent().unwrap_or(path);
    let project_root = PathBuf::from(ROOT_AGENTS_PATH).join("skills");
    if let Ok(relative) = skill_parent.strip_prefix(&project_root) {
        return relative.display().to_string();
    }

    let global_root = global_agents_skill_root();
    if let Ok(relative) = skill_parent.strip_prefix(&global_root) {
        return relative.display().to_string();
    }

    for (provider, root) in provider_global_skill_roots() {
        if let Ok(relative) = skill_parent.strip_prefix(&root) {
            return format!("{}/{}", provider, relative.display());
        }
    }

    skill_parent
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string()
}

fn insert_skill_node(
    nodes: &mut Vec<SkillTreeNode>,
    comps: &[String],
    skill_file: PathBuf,
    skill_name: String,
) {
    if comps.is_empty() {
        return;
    }
    let current = &comps[0];

    let index = if let Some(idx) = nodes.iter().position(|n| &n.dir_name == current) {
        idx
    } else {
        nodes.push(SkillTreeNode {
            dir_name: current.clone(),
            display_name: current.clone(),
            skill_file: None,
            children: Vec::new(),
        });
        nodes.len() - 1
    };

    if comps.len() == 1 {
        nodes[index].skill_file = Some(skill_file);
        nodes[index].display_name = skill_name;
    } else {
        insert_skill_node(
            &mut nodes[index].children,
            &comps[1..],
            skill_file,
            skill_name,
        );
    }
}

fn collect_skill_files(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_skill_files(&path, out)?;
        } else if path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("SKILL.md"))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
    Ok(())
}

fn add_skill_nodes_from_root(
    nodes: &mut Vec<SkillTreeNode>,
    start: &Path,
    provider_prefix: Option<&str>,
) -> io::Result<()> {
    if !start.exists() {
        return Ok(());
    }

    let mut skill_files = Vec::new();
    collect_skill_files(start, &mut skill_files)?;

    for file in skill_files {
        let Some(parent) = file.parent() else {
            continue;
        };
        let Ok(relative) = parent.strip_prefix(start) else {
            continue;
        };

        let mut comps: Vec<String> = Vec::new();
        if let Some(prefix) = provider_prefix {
            comps.push(prefix.to_string());
        }
        comps.extend(
            relative
                .iter()
                .filter_map(|c| c.to_str().map(|s| s.to_string())),
        );

        if comps.is_empty() {
            continue;
        }

        let skill_name = extract_skill_name_from_frontmatter(&file)
            .unwrap_or_else(|| comps.last().cloned().unwrap_or_else(|| "skill".to_string()));
        insert_skill_node(nodes, &comps, file.clone(), skill_name);
    }

    Ok(())
}

pub fn load_project_skill_hierarchy() -> io::Result<Vec<SkillTreeNode>> {
    let root = PathBuf::from(ROOT_AGENTS_PATH);
    let skills_root = root.join("skills");
    let start = if skills_root.exists() {
        skills_root
    } else {
        root
    };

    let mut nodes = Vec::new();
    add_skill_nodes_from_root(&mut nodes, &start, None)?;
    Ok(nodes)
}

pub fn global_agents_skill_root() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("~"));
    home.join(".agents/skills")
}

pub fn provider_global_skill_roots() -> Vec<(String, PathBuf)> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("~"));

    vec![
        ("claude-code".to_string(), home.join(".claude/skills")),
        ("opencode".to_string(), home.join(".config/opencode/skills")),
        ("cursor".to_string(), home.join(".cursor/skills")),
        ("gemini-cli".to_string(), home.join(".gemini/skills")),
        (
            "windsurf".to_string(),
            home.join(".codeium/windsurf/skills"),
        ),
        ("goose".to_string(), home.join(".config/goose/skills")),
    ]
}

pub fn load_global_skill_hierarchy() -> io::Result<Vec<SkillTreeNode>> {
    let mut nodes = Vec::new();
    let agents_root = global_agents_skill_root();
    if agents_root.exists() {
        add_skill_nodes_from_root(&mut nodes, &agents_root, None)?;
        return Ok(nodes);
    }

    for (provider, root) in provider_global_skill_roots() {
        add_skill_nodes_from_root(&mut nodes, &root, Some(&provider))?;
    }
    Ok(nodes)
}

pub fn first_skill_file(nodes: &[SkillTreeNode]) -> Option<PathBuf> {
    for node in nodes {
        if let Some(file) = &node.skill_file {
            return Some(file.clone());
        }
        if let Some(file) = first_skill_file(&node.children) {
            return Some(file);
        }
    }
    None
}
