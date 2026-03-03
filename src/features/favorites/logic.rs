use std::collections::{HashMap, HashSet};

use crate::app::skills_tree::{
    load_skill_slug_from_lock, skill_remove_target_from_path, SkillTreeNode,
};
use lazyskills::config::FavoriteSkill;

pub fn display_slug_for_node(node: &SkillTreeNode) -> Option<String> {
    let skill_file = node.skill_file.as_ref()?;
    Some(skill_remove_target_from_path(skill_file))
}

pub fn install_skill_for_node(node: &SkillTreeNode) -> Option<String> {
    let skill_file = node.skill_file.as_ref()?;
    let display_slug = skill_remove_target_from_path(skill_file);
    let parts = display_slug.split('/').collect::<Vec<_>>();

    for start in 0..parts.len() {
        for end in (start + 1..=parts.len()).rev() {
            let candidate = parts[start..end].join("/");
            if load_skill_slug_from_lock(skill_file, &candidate).is_some() {
                return Some(candidate);
            }
        }
    }

    parts.first().map(|first| (*first).to_string())
}

pub fn favorite_for_node(node: &SkillTreeNode) -> Option<FavoriteSkill> {
    let skill_file = node.skill_file.as_ref()?;
    let display_slug = skill_remove_target_from_path(skill_file);
    let install_skill = install_skill_for_node(node)?;
    let source = load_skill_slug_from_lock(skill_file, &install_skill)
        .and_then(|slug| slug.rsplit_once('/').map(|(src, _)| src.to_string()));

    Some(FavoriteSkill {
        display_slug,
        install_skill,
        source,
        source_type: Some("github".to_string()),
    })
}

pub fn toggle(entries: &mut Vec<FavoriteSkill>, favorite: FavoriteSkill) -> bool {
    if let Some(idx) = entries
        .iter()
        .position(|item| same_favorite_identity(item, &favorite))
    {
        entries.remove(idx);
        false
    } else {
        entries.push(favorite);
        true
    }
}

fn same_favorite_identity(left: &FavoriteSkill, right: &FavoriteSkill) -> bool {
    if let (Some(left_source), Some(right_source)) = (left.source.as_ref(), right.source.as_ref()) {
        return left_source == right_source && left.install_skill == right.install_skill;
    }
    left.display_slug == right.display_slug
}

pub fn favorite_matches_search_slug(favorite: &FavoriteSkill, slug: &str) -> bool {
    if favorite.display_slug == slug || favorite.install_skill == slug {
        return true;
    }

    let mut parts = slug.split('/');
    let owner = parts.next();
    let repo = parts.next();
    let skill = parts.next();
    if let (Some(owner), Some(repo), Some(skill)) = (owner, repo, skill) {
        if let Some(source) = favorite.source.as_ref() {
            return source == &format!("{owner}/{repo}") && favorite.install_skill == skill;
        }

        // Legacy fallback for entries without source metadata.
        if favorite.display_slug == skill || favorite.install_skill == skill {
            return true;
        }
    }

    false
}

pub fn contains_display_slug(entries: &[FavoriteSkill], display_slug: &str) -> bool {
    entries.iter().any(|item| {
        if item.display_slug == display_slug || item.install_skill == display_slug {
            return true;
        }
        if let Some(source) = item.source.as_ref() {
            let full = format!("{}/{}", source, item.install_skill);
            return full == display_slug
                || display_slug.ends_with(&format!("/{}", item.install_skill));
        }
        false
    })
}

fn collect_slug_lookup(nodes: &[SkillTreeNode], out: &mut HashMap<String, SkillTreeNode>) {
    for node in nodes {
        if let Some(skill_file) = node.skill_file.as_ref() {
            let slug = skill_remove_target_from_path(skill_file);
            out.entry(slug).or_insert_with(|| SkillTreeNode {
                dir_name: node.dir_name.clone(),
                display_name: node.display_name.clone(),
                skill_file: node.skill_file.clone(),
                children: Vec::new(),
            });
        }
        collect_slug_lookup(&node.children, out);
    }
}

fn collect_favorite_nodes_from(
    favorites: &[FavoriteSkill],
    slug_lookup: &HashMap<String, SkillTreeNode>,
    out: &mut Vec<SkillTreeNode>,
    seen: &mut HashSet<String>,
) {
    for favorite in favorites {
        let Some(slug) = resolve_favorite_lookup_slug(favorite, slug_lookup) else {
            continue;
        };
        if seen.contains(&slug) {
            continue;
        }

        if let Some(node) = slug_lookup.get(&slug) {
            out.push(SkillTreeNode {
                dir_name: slug.to_string(),
                display_name: node.display_name.clone(),
                skill_file: node.skill_file.clone(),
                children: Vec::new(),
            });
            seen.insert(slug);
        }
    }
}

fn resolve_favorite_lookup_slug(
    favorite: &FavoriteSkill,
    slug_lookup: &HashMap<String, SkillTreeNode>,
) -> Option<String> {
    if slug_lookup.contains_key(&favorite.display_slug) {
        return Some(favorite.display_slug.clone());
    }
    if slug_lookup.contains_key(&favorite.install_skill) {
        return Some(favorite.install_skill.clone());
    }
    if let Some(source) = favorite.source.as_ref() {
        let full = format!("{}/{}", source, favorite.install_skill);
        if slug_lookup.contains_key(&full) {
            return Some(full);
        }
        let suffix = format!("/{}", favorite.install_skill);
        if let Some((slug, _)) = slug_lookup.iter().find(|(slug, _)| slug.ends_with(&suffix)) {
            return Some(slug.clone());
        }
    }
    None
}

pub fn rebuild_nodes(
    favorites: &[FavoriteSkill],
    project_nodes: &[SkillTreeNode],
    global_nodes: &[SkillTreeNode],
) -> Vec<SkillTreeNode> {
    let mut lookup = HashMap::new();
    collect_slug_lookup(project_nodes, &mut lookup);
    collect_slug_lookup(global_nodes, &mut lookup);

    let mut nodes = Vec::new();
    let mut seen = HashSet::new();
    collect_favorite_nodes_from(favorites, &lookup, &mut nodes, &mut seen);

    for favorite in favorites {
        let slug = if favorite.display_slug.is_empty() {
            &favorite.install_skill
        } else {
            &favorite.display_slug
        };
        if seen.contains(slug) {
            continue;
        }
        nodes.push(SkillTreeNode {
            dir_name: slug.clone(),
            display_name: slug.clone(),
            skill_file: None,
            children: Vec::new(),
        });
    }

    nodes
}
