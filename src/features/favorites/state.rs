use crate::app::skills_tree::SkillTreeNode;
use skills_tui::config::FavoriteSkill;

pub struct FavoritesState {
    pub entries: Vec<FavoriteSkill>,
    pub nodes: Vec<SkillTreeNode>,
}

impl FavoritesState {
    pub fn new(entries: Vec<FavoriteSkill>) -> Self {
        Self {
            entries,
            nodes: Vec::new(),
        }
    }
}
