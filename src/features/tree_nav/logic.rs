use crate::app::skills_tree::SkillTreeNode;

use super::state::{ExpandedSkillPaths, SkillPath};

pub fn collect_visible_skill_paths(
    nodes: &[SkillTreeNode],
    expanded: &ExpandedSkillPaths,
    base: &mut SkillPath,
    out: &mut Vec<SkillPath>,
) {
    for (idx, node) in nodes.iter().enumerate() {
        base.push(idx);
        let path = base.clone();
        out.push(path.clone());
        if expanded.contains(&path) {
            collect_visible_skill_paths(&node.children, expanded, base, out);
        }
        let _ = base.pop();
    }
}

pub fn visible_skill_paths(
    nodes: &[SkillTreeNode],
    expanded: &ExpandedSkillPaths,
) -> Vec<SkillPath> {
    let mut out = Vec::new();
    collect_visible_skill_paths(nodes, expanded, &mut Vec::new(), &mut out);
    out
}

pub fn ensure_skill_selection_visible(
    nodes: &[SkillTreeNode],
    expanded: &ExpandedSkillPaths,
    selected_path: &mut Option<SkillPath>,
    offset: &mut usize,
) {
    let visible = visible_skill_paths(nodes, expanded);
    if visible.is_empty() {
        *selected_path = None;
        *offset = 0;
        return;
    }

    let valid = selected_path
        .as_ref()
        .map(|path| visible.iter().any(|visible_path| visible_path == path))
        .unwrap_or(false);
    if !valid {
        *selected_path = Some(visible[0].clone());
    }
}

pub fn select_next_skill(
    nodes: &[SkillTreeNode],
    expanded: &ExpandedSkillPaths,
    selected_path: &mut Option<SkillPath>,
) {
    let visible = visible_skill_paths(nodes, expanded);
    if visible.is_empty() {
        return;
    }

    let current = selected_path
        .as_ref()
        .and_then(|path| visible.iter().position(|visible_path| visible_path == path))
        .unwrap_or(0);
    let next = (current + 1).min(visible.len().saturating_sub(1));
    *selected_path = Some(visible[next].clone());
}

pub fn select_prev_skill(
    nodes: &[SkillTreeNode],
    expanded: &ExpandedSkillPaths,
    selected_path: &mut Option<SkillPath>,
) {
    let visible = visible_skill_paths(nodes, expanded);
    if visible.is_empty() {
        return;
    }

    let current = selected_path
        .as_ref()
        .and_then(|path| visible.iter().position(|visible_path| visible_path == path))
        .unwrap_or(0);
    let prev = current.saturating_sub(1);
    *selected_path = Some(visible[prev].clone());
}

pub fn expand_selected_skill(selected_path: &Option<SkillPath>, expanded: &mut ExpandedSkillPaths) {
    if let Some(path) = selected_path.clone() {
        expanded.insert(path);
    }
}

pub fn collapse_selected_skill(
    selected_path: &mut Option<SkillPath>,
    expanded: &mut ExpandedSkillPaths,
) {
    if let Some(path) = selected_path.clone() {
        if expanded.contains(&path) {
            expanded.remove(&path);
        } else if path.len() > 1 {
            let mut parent = path;
            parent.pop();
            *selected_path = Some(parent);
        }
    }
}

pub fn select_skill_by_visible_row(
    nodes: &[SkillTreeNode],
    expanded: &ExpandedSkillPaths,
    selected_path: &mut Option<SkillPath>,
    offset: &mut usize,
    row_index: usize,
) {
    ensure_skill_selection_visible(nodes, expanded, selected_path, offset);
    let visible = visible_skill_paths(nodes, expanded);
    let absolute_index = offset.saturating_add(row_index);
    if let Some(path) = visible.get(absolute_index) {
        *selected_path = Some(path.clone());
    }
}
