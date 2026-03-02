use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use skills_tui::adapters::skills_sh::{SkillDetail, SkillListItem, SkillsShClient};

pub struct SearchState {
    pub search_client: Option<SkillsShClient>,
    pub search_query: String,
    pub search_results: Vec<SkillListItem>,
    pub search_selected: usize,
    pub search_offset: usize,
    pub search_detail: Option<SkillDetail>,
    pub search_status: String,
    pub input_focused: bool,
    pub pending_search_refresh_since: Option<Instant>,
    pub pending_search_detail_slug: Option<String>,
    pub pending_search_detail_since: Option<Instant>,
    pub search_gh_cache: Arc<Mutex<HashMap<String, String>>>,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            search_client: SkillsShClient::new().ok(),
            search_query: String::new(),
            search_results: Vec::new(),
            search_selected: 0,
            search_offset: 0,
            search_detail: None,
            search_status: "Press / to focus search input".to_string(),
            input_focused: false,
            pending_search_refresh_since: None,
            pending_search_detail_slug: None,
            pending_search_detail_since: None,
            search_gh_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
