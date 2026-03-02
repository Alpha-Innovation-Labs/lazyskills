use std::time::{Duration, Instant};

use skills_tui::adapters::skills_sh::{SkillListItem, SkillsMode};

use super::state::SearchState;

pub fn selected_search_item(state: &SearchState) -> Option<&SkillListItem> {
    state.search_results.get(state.search_selected)
}

pub fn skill_slug(item: &SkillListItem) -> Option<String> {
    if let Some(id) = item.id.as_ref() {
        if !id.is_empty() {
            return Some(id.clone());
        }
    }
    let skill_id = item.skill_id.as_ref()?;
    if item.source.is_empty() {
        return None;
    }
    Some(format!("{}/{}", item.source, skill_id))
}

pub fn split_slug(slug: &str) -> Option<(&str, &str, &str)> {
    let mut parts = slug.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    let skill = parts.next()?;
    Some((owner, repo, skill))
}

pub fn refresh_search_results(state: &mut SearchState) {
    let Some(client) = state.search_client.as_ref() else {
        state.search_status = "Search unavailable: failed to initialize client".to_string();
        state.search_results.clear();
        state.search_detail = None;
        state.search_selected = 0;
        return;
    };

    let query = state.search_query.trim().to_string();
    let result = if query.is_empty() {
        client
            .fetch_catalog_page(SkillsMode::AllTime, 1)
            .map(|page| page.skills)
    } else {
        client
            .fetch_search(&query, 50)
            .map(|response| response.skills)
    };

    match result {
        Ok(skills) => {
            state.search_results = skills;
            state.search_selected = 0;
            state.search_detail = None;
            state.search_status = if query.is_empty() {
                "Showing top all-time skills (type to search)".to_string()
            } else {
                format!("Found {} results", state.search_results.len())
            };
            if !state.search_results.is_empty() {
                queue_selected_search_detail(state);
            }
        }
        Err(err) => {
            state.search_results.clear();
            state.search_detail = None;
            state.search_selected = 0;
            state.search_status = format!("Search failed: {err}");
        }
    }
}

pub fn queue_search_refresh(state: &mut SearchState) {
    state.pending_search_refresh_since = Some(Instant::now());
}

pub fn flush_pending_search_refresh_if_ready(state: &mut SearchState) -> bool {
    const SEARCH_QUERY_DEBOUNCE_MS: u64 = 220;

    let Some(pending_since) = state.pending_search_refresh_since else {
        return false;
    };
    if pending_since.elapsed() < Duration::from_millis(SEARCH_QUERY_DEBOUNCE_MS) {
        return false;
    }

    state.pending_search_refresh_since = None;
    refresh_search_results(state);
    true
}

pub fn queue_selected_search_detail(state: &mut SearchState) {
    let Some(item) = selected_search_item(state) else {
        state.pending_search_detail_slug = None;
        state.pending_search_detail_since = None;
        return;
    };
    let Some(slug) = skill_slug(item) else {
        state.pending_search_detail_slug = None;
        state.pending_search_detail_since = None;
        return;
    };
    state.pending_search_detail_slug = Some(slug);
    state.pending_search_detail_since = Some(Instant::now());
}

pub fn flush_pending_search_detail_if_ready(state: &mut SearchState) -> bool {
    const SEARCH_DETAIL_DEBOUNCE_MS: u64 = 200;

    let Some(pending_since) = state.pending_search_detail_since else {
        return false;
    };
    if pending_since.elapsed() < Duration::from_millis(SEARCH_DETAIL_DEBOUNCE_MS) {
        return false;
    }

    let Some(slug) = state.pending_search_detail_slug.take() else {
        state.pending_search_detail_since = None;
        return false;
    };
    state.pending_search_detail_since = None;

    let Some((owner, repo, skill)) = split_slug(&slug) else {
        state.search_status = format!("Selected slug is invalid: {slug}");
        return true;
    };
    let Some(client) = state.search_client.as_ref() else {
        state.search_status = "Detail fetch unavailable: failed to initialize client".to_string();
        return true;
    };

    match client.fetch_skill_detail(owner, repo, skill) {
        Ok(detail) => {
            state.search_detail = Some(detail);
        }
        Err(err) => {
            state.search_status = format!("Detail fetch failed: {err}");
            state.search_detail = None;
        }
    }

    true
}

pub fn install_selected_search_skill_slug(state: &SearchState) -> Result<String, String> {
    let Some(item) = selected_search_item(state) else {
        return Err("No search result selected".to_string());
    };
    let Some(slug) = skill_slug(item) else {
        return Err("Selected skill is missing a valid slug".to_string());
    };
    Ok(slug)
}
