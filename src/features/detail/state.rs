use std::time::Instant;

use skills_tui::adapters::skills_sh::SkillDetail;

pub struct DetailState {
    pub show_detail_pane: bool,
    pub project_global_detail: Option<SkillDetail>,
    pub pending_project_global_detail_slug: Option<String>,
    pub pending_project_global_detail_since: Option<Instant>,
}

impl DetailState {
    pub fn new() -> Self {
        Self {
            show_detail_pane: true,
            project_global_detail: None,
            pending_project_global_detail_slug: None,
            pending_project_global_detail_since: None,
        }
    }
}
