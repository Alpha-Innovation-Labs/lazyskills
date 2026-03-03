use std::time::{Duration, Instant};

use lazyskills::{adapters::skills_sh::SkillsShClient, config::FavoriteSkill};

use super::state::DetailState;

pub fn selected_project_global_slug(favorite: &FavoriteSkill) -> Option<String> {
    if let Some(source) = favorite.source.as_ref() {
        return Some(format!("{}/{}", source, favorite.install_skill));
    }

    let parts = favorite
        .display_slug
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() >= 3 {
        let n = parts.len();
        return Some(format!(
            "{}/{}/{}",
            parts[n - 3],
            parts[n - 2],
            parts[n - 1]
        ));
    }

    None
}

pub fn queue_project_global_detail_refresh(state: &mut DetailState, slug: Option<String>) {
    let Some(slug) = slug else {
        state.pending_project_global_detail_slug = None;
        state.pending_project_global_detail_since = None;
        return;
    };

    state.pending_project_global_detail_slug = Some(slug);
    state.pending_project_global_detail_since = Some(Instant::now());
}

pub fn fetch_project_global_detail_now(
    state: &mut DetailState,
    client: Option<&SkillsShClient>,
    slug: Option<&str>,
) -> bool {
    let Some(slug) = slug else {
        state.project_global_detail = None;
        return false;
    };
    let Some((owner, repo, skill)) = split_slug(slug) else {
        state.project_global_detail = None;
        return true;
    };
    let Some(client) = client else {
        state.project_global_detail = None;
        return true;
    };

    match client.fetch_skill_detail_cached_swr(owner, repo, skill) {
        Ok(detail) => {
            state.project_global_detail = Some(detail);
        }
        Err(_) => {
            state.project_global_detail = None;
        }
    }
    true
}

pub fn flush_pending_project_global_detail_if_ready(
    state: &mut DetailState,
    client: Option<&SkillsShClient>,
) -> bool {
    const DETAIL_DEBOUNCE_MS: u64 = 200;

    let Some(pending_since) = state.pending_project_global_detail_since else {
        return false;
    };
    if pending_since.elapsed() < Duration::from_millis(DETAIL_DEBOUNCE_MS) {
        return false;
    }

    let Some(slug) = state.pending_project_global_detail_slug.take() else {
        state.pending_project_global_detail_since = None;
        return false;
    };
    state.pending_project_global_detail_since = None;

    fetch_project_global_detail_now(state, client, Some(&slug))
}

pub fn toggle_project_global_detail_pane(state: &mut DetailState) -> bool {
    state.show_detail_pane = !state.show_detail_pane;
    state.show_detail_pane
}

fn split_slug(slug: &str) -> Option<(&str, &str, &str)> {
    let mut parts = slug.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    let skill = parts.next()?;
    Some((owner, repo, skill))
}

#[cfg(test)]
mod tests {
    use super::selected_project_global_slug;
    use lazyskills::config::FavoriteSkill;

    #[test]
    fn snapshot_global_slug_without_source_is_none() {
        let favorite = FavoriteSkill {
            display_slug: "find-skills".to_string(),
            install_skill: "find-skills".to_string(),
            source: None,
            source_type: Some("github".to_string()),
        };

        let slug = selected_project_global_slug(&favorite);
        insta::assert_snapshot!(format!("{:?}", slug), @"None");
    }

    #[test]
    fn snapshot_global_slug_with_source_is_resolved() {
        let favorite = FavoriteSkill {
            display_slug: "find-skills".to_string(),
            install_skill: "find-skills".to_string(),
            source: Some("vercel-labs/skills".to_string()),
            source_type: Some("github".to_string()),
        };

        let slug = selected_project_global_slug(&favorite);
        insta::assert_snapshot!(
            format!("{:?}", slug),
            @"Some(\"vercel-labs/skills/find-skills\")"
        );
    }
}
