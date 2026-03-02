use std::time::Instant;

use skills_tui::adapters::skills_sh::{SkillDetail, SkillListItem, SkillsShClient};

pub struct SearchState {
    pub search_client: Option<SkillsShClient>,
    pub search_query: String,
    pub search_results: Vec<SkillListItem>,
    pub search_selected: usize,
    pub search_detail: Option<SkillDetail>,
    pub search_status: String,
    pub pending_search_refresh_since: Option<Instant>,
    pub pending_search_detail_slug: Option<String>,
    pub pending_search_detail_since: Option<Instant>,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            search_client: SkillsShClient::new().ok(),
            search_query: String::new(),
            search_results: Vec::new(),
            search_selected: 0,
            search_detail: None,
            search_status: "Type to search skills.sh".to_string(),
            pending_search_refresh_since: None,
            pending_search_detail_slug: None,
            pending_search_detail_since: None,
        }
    }
}
