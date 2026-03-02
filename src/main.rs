use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use std::{collections::HashSet, fs};

use crossterm::event::{
    KeyCode, KeyEvent as CrosstermKeyEvent, KeyEventState, MouseButton,
    MouseEvent as CrosstermMouseEvent, MouseEventKind,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use ratkit::prelude::{
    run, CoordinatorAction, CoordinatorApp, CoordinatorEvent, LayoutResult, MouseEvent,
    RunnerConfig,
};
use ratkit::primitives::menu_bar::{MenuBar, MenuItem};
use ratkit::primitives::pane::Pane;
use ratkit::primitives::resizable_grid::{
    PaneId, ResizableGrid, ResizableGridWidget, ResizableGridWidgetState,
};
use ratkit::widgets::markdown_preview::{
    CacheState, CollapseState, DisplaySettings, DoubleClickState, ExpandableState, GitStatsState,
    MarkdownEvent, MarkdownWidget, ScrollState, SelectionState, SourceState, VimState,
};
use ratkit::widgets::{Dialog, DialogWidget};
use ratkit::widgets::{HotkeyFooter, HotkeyItem};
use serde::{Deserialize, Serialize};
use skills_tui::adapters::skills_sh::{SkillDetail, SkillListItem, SkillsMode, SkillsShClient};

const DEFAULT_SKILL_PATH: &str = ".agents/skills/ratkit/SKILL.md";
const APP_CONFIG_PATH: &str = ".agents/skills-tui-config.json";
const ROOT_AGENTS_PATH: &str = ".agents";
const TERMINAL_ICON: &str = "";
const YAZI_CYAN: Color = Color::Rgb(3, 169, 244);
const SKILLS_CONFIG_VERSION: u8 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SkillsCommandMode {
    Global,
    Npx,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SkillsCommandConfig {
    mode: SkillsCommandMode,
    global_command: String,
    npx_command: String,
    npx_package: String,
    expected_identity_substring: String,
    global_command_verified: bool,
    global_command_version: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AppConfig {
    version: u8,
    skills_command: SkillsCommandConfig,
    #[serde(default)]
    favorite_skills: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: SKILLS_CONFIG_VERSION,
            skills_command: SkillsCommandConfig {
                mode: SkillsCommandMode::Global,
                global_command: "skills".to_string(),
                npx_command: "npx".to_string(),
                npx_package: "skills".to_string(),
                expected_identity_substring: "skills".to_string(),
                global_command_verified: false,
                global_command_version: None,
            },
            favorite_skills: Vec::new(),
        }
    }
}

struct StartupConfigOutcome {
    config: AppConfig,
    startup_dialog: Option<StartupDialogState>,
}

enum StartupDialogState {
    Info {
        title: String,
        message: String,
    },
    ChooseCommand {
        selected_button: usize,
        error_message: Option<String>,
    },
}

struct DeleteConfirmDialogState {
    selected_button: usize,
    skill_name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FocusPane {
    Tree,
    Preview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AppView {
    Project,
    Search,
    Global,
    Favorites,
    Config,
}

#[derive(Clone, Debug)]
struct SkillTreeNode {
    dir_name: String,
    display_name: String,
    skill_file: Option<PathBuf>,
    children: Vec<SkillTreeNode>,
}

struct SkillPreviewApp {
    app_config: AppConfig,
    startup_dialog: Option<StartupDialogState>,
    widget: MarkdownWidget<'static>,
    menu: MenuBar,
    project_skills_nodes: Vec<SkillTreeNode>,
    global_skills_nodes: Vec<SkillTreeNode>,
    favorite_skills_nodes: Vec<SkillTreeNode>,
    skills_selected_path: Option<Vec<usize>>,
    skills_expanded: HashSet<Vec<usize>>,
    skills_offset: usize,
    source_path: PathBuf,
    preview_title: String,
    show_toc: bool,
    current_view: AppView,
    focus: FocusPane,
    grid_layout: ResizableGrid,
    grid_state: ResizableGridWidgetState,
    navbar_area: Rect,
    grid_area: Rect,
    tree_pane_id: PaneId,
    preview_pane_id: PaneId,
    tree_area: Rect,
    tree_content_area: Rect,
    markdown_area: Rect,
    markdown_inner_area: Rect,
    last_move_processed: Instant,
    toast_message: Option<String>,
    toast_expires_at: Option<Instant>,
    show_hotkeys_modal: bool,
    delete_confirm_dialog: Option<DeleteConfirmDialogState>,
    pending_preview_path: Option<PathBuf>,
    pending_preview_since: Option<Instant>,
    search_client: Option<SkillsShClient>,
    search_query: String,
    search_results: Vec<SkillListItem>,
    search_selected: usize,
    search_detail: Option<SkillDetail>,
    search_status: String,
    pending_search_refresh_since: Option<Instant>,
    pending_search_detail_slug: Option<String>,
    pending_search_detail_since: Option<Instant>,
    config_text: String,
    config_cursor_line: usize,
    config_cursor_col: usize,
    config_scroll: usize,
    config_selected_field: usize,
    config_value_cursor: usize,
    config_status: String,
    config_dirty: bool,
}

impl SkillPreviewApp {
    fn build_menu() -> MenuBar {
        MenuBar::new(vec![
            MenuItem::new("Project [1]", 0),
            MenuItem::new("Global [2]", 1),
            MenuItem::new("Search [3]", 2),
            MenuItem::new("Favorites [4]", 3),
            MenuItem::new("Config [5]", 4),
        ])
        .with_selected(0)
    }

    fn set_menu_selection(&mut self, target: usize) {
        for (idx, item) in self.menu.items.iter_mut().enumerate() {
            item.selected = idx == target;
        }
    }

    fn set_view(&mut self, view: AppView) {
        self.current_view = view;
        match view {
            AppView::Project => self.set_menu_selection(0),
            AppView::Global => self.set_menu_selection(1),
            AppView::Search => self.set_menu_selection(2),
            AppView::Favorites => self.set_menu_selection(3),
            AppView::Config => self.set_menu_selection(4),
        }

        if matches!(
            view,
            AppView::Project | AppView::Global | AppView::Favorites
        ) {
            self.skills_offset = 0;
            self.skills_selected_path = Some(vec![0]);
            self.pending_preview_path = None;
            self.pending_preview_since = None;
            self.ensure_skill_selection_visible();
            self.open_selected_file_immediate();
        } else if matches!(view, AppView::Search) && self.search_results.is_empty() {
            self.refresh_search_results();
        } else if matches!(view, AppView::Config) {
            self.config_status = "Edit values. Ctrl+S save. Up/Down select field.".to_string();
        }
    }

    fn selected_search_item(&self) -> Option<&SkillListItem> {
        self.search_results.get(self.search_selected)
    }

    fn skill_slug(item: &SkillListItem) -> Option<String> {
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

    fn split_slug(slug: &str) -> Option<(&str, &str, &str)> {
        let mut parts = slug.split('/');
        let owner = parts.next()?;
        let repo = parts.next()?;
        let skill = parts.next()?;
        Some((owner, repo, skill))
    }

    fn refresh_search_results(&mut self) {
        let Some(client) = self.search_client.as_ref() else {
            self.search_status = "Search unavailable: failed to initialize client".to_string();
            self.search_results.clear();
            self.search_detail = None;
            self.search_selected = 0;
            return;
        };

        let query = self.search_query.trim().to_string();
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
                self.search_results = skills;
                self.search_selected = 0;
                self.search_detail = None;
                self.search_status = if query.is_empty() {
                    "Showing top all-time skills (type to search)".to_string()
                } else {
                    format!("Found {} results", self.search_results.len())
                };
                if !self.search_results.is_empty() {
                    self.queue_selected_search_detail();
                }
            }
            Err(err) => {
                self.search_results.clear();
                self.search_detail = None;
                self.search_selected = 0;
                self.search_status = format!("Search failed: {err}");
            }
        }
    }

    fn queue_search_refresh(&mut self) {
        self.pending_search_refresh_since = Some(Instant::now());
    }

    fn flush_pending_search_refresh_if_ready(&mut self) -> bool {
        const SEARCH_QUERY_DEBOUNCE_MS: u64 = 220;

        let Some(pending_since) = self.pending_search_refresh_since else {
            return false;
        };
        if pending_since.elapsed() < Duration::from_millis(SEARCH_QUERY_DEBOUNCE_MS) {
            return false;
        }

        self.pending_search_refresh_since = None;
        self.refresh_search_results();
        true
    }

    fn fetch_selected_search_detail(&mut self) {
        let Some(client) = self.search_client.as_ref() else {
            self.search_status =
                "Detail fetch unavailable: failed to initialize client".to_string();
            return;
        };
        let Some(item) = self.selected_search_item() else {
            return;
        };
        let Some(slug) = Self::skill_slug(item) else {
            self.search_status = "Selected skill is missing a valid slug".to_string();
            return;
        };
        let Some((owner, repo, skill)) = Self::split_slug(&slug) else {
            self.search_status = format!("Selected slug is invalid: {slug}");
            return;
        };

        match client.fetch_skill_detail(owner, repo, skill) {
            Ok(detail) => {
                self.search_detail = Some(detail);
            }
            Err(err) => {
                self.search_status = format!("Detail fetch failed: {err}");
                self.search_detail = None;
            }
        }
    }

    fn queue_selected_search_detail(&mut self) {
        let Some(item) = self.selected_search_item() else {
            self.pending_search_detail_slug = None;
            self.pending_search_detail_since = None;
            return;
        };
        let Some(slug) = Self::skill_slug(item) else {
            self.pending_search_detail_slug = None;
            self.pending_search_detail_since = None;
            return;
        };
        self.pending_search_detail_slug = Some(slug);
        self.pending_search_detail_since = Some(Instant::now());
    }

    fn install_selected_search_skill(&mut self) {
        let Some(item) = self.selected_search_item() else {
            self.search_status = "No search result selected".to_string();
            return;
        };
        let Some(slug) = Self::skill_slug(item) else {
            self.search_status = "Selected skill is missing a valid slug".to_string();
            return;
        };

        match self.run_configured_skills_command(&["add", &slug]) {
            Ok(_) => {
                self.refresh_skill_hierarchies();
                self.show_toast(format!("Installed {slug}"));
                self.search_status = format!("Installed {slug}");
            }
            Err(err) => {
                self.search_status = format!("Install failed: {err}");
            }
        }
    }

    fn flush_pending_search_detail_if_ready(&mut self) -> bool {
        const SEARCH_DETAIL_DEBOUNCE_MS: u64 = 200;

        let Some(pending_since) = self.pending_search_detail_since else {
            return false;
        };
        if pending_since.elapsed() < Duration::from_millis(SEARCH_DETAIL_DEBOUNCE_MS) {
            return false;
        }

        let Some(slug) = self.pending_search_detail_slug.take() else {
            self.pending_search_detail_since = None;
            return false;
        };
        self.pending_search_detail_since = None;

        let Some((owner, repo, skill)) = Self::split_slug(&slug) else {
            self.search_status = format!("Selected slug is invalid: {slug}");
            return true;
        };
        let Some(client) = self.search_client.as_ref() else {
            self.search_status =
                "Detail fetch unavailable: failed to initialize client".to_string();
            return true;
        };

        match client.fetch_skill_detail(owner, repo, skill) {
            Ok(detail) => {
                self.search_detail = Some(detail);
            }
            Err(err) => {
                self.search_status = format!("Detail fetch failed: {err}");
                self.search_detail = None;
            }
        }

        true
    }

    fn config_field_count(&self) -> usize {
        2
    }

    fn config_field_label(index: usize) -> &'static str {
        match index {
            0 => "mode",
            1 => "command",
            _ => "",
        }
    }

    fn config_field_value(&self, index: usize) -> String {
        match index {
            0 => match self.app_config.skills_command.mode {
                SkillsCommandMode::Global => "global".to_string(),
                SkillsCommandMode::Npx => "npx".to_string(),
            },
            1 => match self.app_config.skills_command.mode {
                SkillsCommandMode::Global => self.app_config.skills_command.global_command.clone(),
                SkillsCommandMode::Npx => self.app_config.skills_command.npx_command.clone(),
            },
            _ => String::new(),
        }
    }

    fn set_selected_config_value(&mut self, value: String) {
        match self.config_selected_field {
            1 => match self.app_config.skills_command.mode {
                SkillsCommandMode::Global => self.app_config.skills_command.global_command = value,
                SkillsCommandMode::Npx => self.app_config.skills_command.npx_command = value,
            },
            _ => {}
        }
    }

    fn is_text_config_field(&self) -> bool {
        self.config_selected_field == 1
    }

    fn toggle_config_field(&mut self) {
        match self.config_selected_field {
            0 => {
                self.app_config.skills_command.mode = match self.app_config.skills_command.mode {
                    SkillsCommandMode::Global => SkillsCommandMode::Npx,
                    SkillsCommandMode::Npx => SkillsCommandMode::Global,
                }
            }
            _ => {}
        }
        self.config_dirty = true;
    }

    fn render_config_value_with_cursor(&self, value: &str) -> String {
        if !self.is_text_config_field() {
            return value.to_string();
        }

        let chars: Vec<char> = value.chars().collect();
        let mut out = String::new();
        for (idx, ch) in chars.iter().enumerate() {
            if idx == self.config_value_cursor {
                out.push('▏');
            }
            out.push(*ch);
        }
        if self.config_value_cursor >= chars.len() {
            out.push('▏');
        }
        out
    }

    fn handle_config_key(&mut self, key: CrosstermKeyEvent) -> CoordinatorAction {
        if key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('s'))
        {
            match persist_app_config(&self.app_config) {
                Ok(_) => {
                    self.config_dirty = false;
                    self.config_status = format!("Saved {}", APP_CONFIG_PATH);
                }
                Err(err) => {
                    self.config_status = format!("Save failed: {err}");
                }
            }
            return CoordinatorAction::Redraw;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.config_selected_field = self.config_selected_field.saturating_sub(1);
                self.config_value_cursor = self
                    .config_field_value(self.config_selected_field)
                    .chars()
                    .count();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.config_selected_field = (self.config_selected_field + 1)
                    .min(self.config_field_count().saturating_sub(1));
                self.config_value_cursor = self
                    .config_field_value(self.config_selected_field)
                    .chars()
                    .count();
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if self.is_text_config_field() {
                    self.config_value_cursor = self.config_value_cursor.saturating_sub(1);
                } else {
                    self.toggle_config_field();
                }
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
                if self.is_text_config_field() {
                    let len = self
                        .config_field_value(self.config_selected_field)
                        .chars()
                        .count();
                    self.config_value_cursor = (self.config_value_cursor + 1).min(len);
                } else {
                    self.toggle_config_field();
                }
            }
            KeyCode::Backspace if self.is_text_config_field() => {
                let mut value = self.config_field_value(self.config_selected_field);
                if self.config_value_cursor > 0 {
                    let mut chars: Vec<char> = value.chars().collect();
                    let idx = self.config_value_cursor - 1;
                    chars.remove(idx);
                    value = chars.into_iter().collect();
                    self.config_value_cursor = idx;
                    self.set_selected_config_value(value);
                    self.config_dirty = true;
                }
            }
            KeyCode::Delete if self.is_text_config_field() => {
                let mut value = self.config_field_value(self.config_selected_field);
                let mut chars: Vec<char> = value.chars().collect();
                if self.config_value_cursor < chars.len() {
                    chars.remove(self.config_value_cursor);
                    value = chars.into_iter().collect();
                    self.set_selected_config_value(value);
                    self.config_dirty = true;
                }
            }
            KeyCode::Char(ch)
                if self.is_text_config_field()
                    && !key
                        .modifiers
                        .intersects(crossterm::event::KeyModifiers::CONTROL)
                    && !key
                        .modifiers
                        .intersects(crossterm::event::KeyModifiers::ALT) =>
            {
                let mut value = self.config_field_value(self.config_selected_field);
                let mut chars: Vec<char> = value.chars().collect();
                chars.insert(self.config_value_cursor, ch);
                value = chars.into_iter().collect();
                self.config_value_cursor += 1;
                self.set_selected_config_value(value);
                self.config_dirty = true;
            }
            _ => {}
        }

        CoordinatorAction::Redraw
    }

    fn config_lines(&self) -> Vec<&str> {
        let mut lines = self.config_text.lines().collect::<Vec<_>>();
        if lines.is_empty() {
            lines.push("");
        }
        lines
    }

    fn clamp_config_cursor(&mut self) {
        let line_count = self.config_lines().len().max(1);
        self.config_cursor_line = self.config_cursor_line.min(line_count.saturating_sub(1));
        let line_len = self
            .config_lines()
            .get(self.config_cursor_line)
            .map(|line| line.chars().count())
            .unwrap_or(0);
        self.config_cursor_col = self.config_cursor_col.min(line_len);
    }

    fn active_skills_nodes(&self) -> &[SkillTreeNode] {
        match self.current_view {
            AppView::Project | AppView::Search => &self.project_skills_nodes,
            AppView::Global => &self.global_skills_nodes,
            AppView::Favorites => &self.favorite_skills_nodes,
            AppView::Config => &self.project_skills_nodes,
        }
    }

    fn skill_key_from_path(path: &Path) -> String {
        let project_root = PathBuf::from(ROOT_AGENTS_PATH);
        if let Ok(relative) = path.strip_prefix(&project_root) {
            return format!("project:{}", relative.display());
        }

        let global_root = global_agents_skill_root();
        if let Ok(relative) = path.strip_prefix(&global_root) {
            return format!("global:{}", relative.display());
        }

        for (provider, root) in provider_global_skill_roots() {
            if let Ok(relative) = path.strip_prefix(&root) {
                return format!("global:{}/{}", provider, relative.display());
            }
        }

        format!("path:{}", path.display())
    }

    fn skill_remove_target_from_path(path: &Path) -> String {
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

    fn selected_skill_identity(&self) -> Option<(PathBuf, String, String)> {
        let node = self.selected_skill_node()?;
        let skill_file = node.skill_file.as_ref()?.clone();
        let key = Self::skill_key_from_path(&skill_file);
        let remove_target = Self::skill_remove_target_from_path(&skill_file);
        Some((skill_file, key, remove_target))
    }

    fn is_favorite_node(&self, node: &SkillTreeNode) -> bool {
        let Some(path) = node.skill_file.as_ref() else {
            return false;
        };
        let key = Self::skill_key_from_path(path);
        self.app_config
            .favorite_skills
            .iter()
            .any(|item| item == &key)
    }

    fn collect_favorite_nodes_from(
        nodes: &[SkillTreeNode],
        favorite_keys: &HashSet<String>,
        source: &str,
        out: &mut Vec<SkillTreeNode>,
        seen: &mut HashSet<String>,
    ) {
        for node in nodes {
            if let Some(path) = node.skill_file.as_ref() {
                let key = Self::skill_key_from_path(path);
                if favorite_keys.contains(&key) && !seen.contains(&key) {
                    seen.insert(key);
                    out.push(SkillTreeNode {
                        dir_name: node.dir_name.clone(),
                        display_name: format!("{} ({})", node.display_name, source),
                        skill_file: Some(path.clone()),
                        children: Vec::new(),
                    });
                }
            }
            Self::collect_favorite_nodes_from(&node.children, favorite_keys, source, out, seen);
        }
    }

    fn rebuild_favorites_nodes(&mut self) {
        let favorite_keys = self
            .app_config
            .favorite_skills
            .iter()
            .cloned()
            .collect::<HashSet<_>>();

        let mut nodes = Vec::new();
        let mut seen = HashSet::new();
        Self::collect_favorite_nodes_from(
            &self.project_skills_nodes,
            &favorite_keys,
            "project",
            &mut nodes,
            &mut seen,
        );
        Self::collect_favorite_nodes_from(
            &self.global_skills_nodes,
            &favorite_keys,
            "global",
            &mut nodes,
            &mut seen,
        );

        self.favorite_skills_nodes = nodes;
        self.app_config
            .favorite_skills
            .retain(|key| seen.contains(key));
    }

    fn refresh_skill_hierarchies(&mut self) {
        if let Ok(nodes) = load_project_skill_hierarchy() {
            self.project_skills_nodes = nodes;
        }
        if let Ok(nodes) = load_global_skill_hierarchy() {
            self.global_skills_nodes = nodes;
        }
        self.rebuild_favorites_nodes();
        self.ensure_skill_selection_visible();
        self.open_selected_file_immediate();
    }

    fn toggle_selected_favorite(&mut self) {
        let Some((_, key, _)) = self.selected_skill_identity() else {
            self.show_toast("No selectable skill");
            return;
        };

        if let Some(idx) = self
            .app_config
            .favorite_skills
            .iter()
            .position(|item| item == &key)
        {
            self.app_config.favorite_skills.remove(idx);
            self.show_toast("Removed from favorites");
        } else {
            self.app_config.favorite_skills.push(key);
            self.show_toast("Added to favorites");
        }

        if let Err(err) = persist_app_config(&self.app_config) {
            self.show_toast(format!("Failed to save config: {}", err));
        }
        self.rebuild_favorites_nodes();
    }

    fn delete_selected_skill(&mut self) {
        let Some((_, key, remove_target)) = self.selected_skill_identity() else {
            self.show_toast("Select a skill file to delete");
            return;
        };

        match self.run_configured_skills_command(&["remove", &remove_target]) {
            Ok(_) => {
                self.app_config.favorite_skills.retain(|item| item != &key);
                if let Err(err) = persist_app_config(&self.app_config) {
                    self.show_toast(format!("Deleted skill, but config update failed: {}", err));
                }
                self.refresh_skill_hierarchies();
                self.show_toast("Deleted selected skill");
            }
            Err(err) => {
                self.show_toast(format!("skills remove failed: {}", err));
            }
        }
    }

    fn open_delete_confirm_dialog(&mut self) {
        let Some((_, _, skill_name)) = self.selected_skill_identity() else {
            self.show_toast("Select a skill file to delete");
            return;
        };

        self.delete_confirm_dialog = Some(DeleteConfirmDialogState {
            selected_button: 1,
            skill_name,
        });
    }

    fn handle_delete_confirm_key(
        &mut self,
        key_code: KeyCode,
        _modifiers: crossterm::event::KeyModifiers,
    ) -> bool {
        let Some(state) = self.delete_confirm_dialog.as_mut() else {
            return false;
        };

        match key_code {
            KeyCode::Left | KeyCode::Char('h') => {
                state.selected_button = state.selected_button.saturating_sub(1)
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
                state.selected_button = (state.selected_button + 1).min(1)
            }
            KeyCode::Esc => self.delete_confirm_dialog = None,
            KeyCode::Char('d') => {
                self.delete_confirm_dialog = None;
                self.delete_selected_skill();
            }
            KeyCode::Enter => {
                let choice = state.selected_button;
                self.delete_confirm_dialog = None;
                if choice == 0 {
                    self.show_toast("Press d again to delete selected skill");
                }
            }
            _ => {}
        }

        true
    }

    fn run_configured_skills_command(&self, args: &[&str]) -> Result<String, String> {
        let mut command = if matches!(self.app_config.skills_command.mode, SkillsCommandMode::Npx) {
            let mut cmd = Command::new(&self.app_config.skills_command.npx_command);
            cmd.arg(&self.app_config.skills_command.npx_package);
            cmd
        } else {
            Command::new(&self.app_config.skills_command.global_command)
        };

        let output = command
            .args(args)
            .output()
            .map_err(|err| format!("Failed to run command: {}", err))?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if stdout.is_empty() {
                Ok("ok".to_string())
            } else {
                Ok(stdout)
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let msg = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("Command exited with status {}", output.status)
            };
            Err(msg)
        }
    }

    fn update_selected_skill(&mut self) {
        let Some((_, _, skill_name)) = self.selected_skill_identity() else {
            self.show_toast("Select a skill file to update");
            return;
        };

        let _ = self.run_configured_skills_command(&["check", &skill_name]);
        match self.run_configured_skills_command(&["update", &skill_name]) {
            Ok(_) => {
                self.refresh_skill_hierarchies();
                self.show_toast("Skill updated");
            }
            Err(err) => {
                self.show_toast(format!("Update failed: {}", err));
            }
        }
    }

    fn build_widget(source: SourceState, show_toc: bool) -> MarkdownWidget<'static> {
        let markdown_content = source.content().unwrap_or_default().to_owned();

        let mut scroll = ScrollState::default();
        scroll.update_total_lines(markdown_content.lines().count().max(1));

        let mut display = DisplaySettings::default();
        let _ = display.set_show_document_line_numbers(true);

        MarkdownWidget::new(
            markdown_content,
            scroll,
            source,
            CacheState::default(),
            display,
            CollapseState::default(),
            ExpandableState::default(),
            GitStatsState::default(),
            VimState::default(),
            SelectionState::default(),
            DoubleClickState::default(),
        )
        .with_has_pane(false)
        .show_toc(show_toc)
        .show_scrollbar(true)
        .show_statusline(true)
    }

    fn new(
        app_config: AppConfig,
        startup_dialog: Option<StartupDialogState>,
        source_path: PathBuf,
        source: SourceState,
        project_skills_nodes: Vec<SkillTreeNode>,
        global_skills_nodes: Vec<SkillTreeNode>,
    ) -> Self {
        let show_toc = true;
        let widget = Self::build_widget(source, show_toc);
        let preview_title = extract_skill_name_from_frontmatter(&source_path)
            .unwrap_or_else(|| fallback_title_from_path(&source_path));

        let mut skills_expanded = HashSet::new();
        collect_expanded_skill_paths(&project_skills_nodes, &mut Vec::new(), &mut skills_expanded);

        let mut grid_layout = ResizableGrid::new(0);
        let preview_pane_id = grid_layout.split_pane_vertically(0).unwrap_or(0);
        let _ = grid_layout.resize_divider(0, 50);

        let mut app = Self {
            app_config,
            startup_dialog,
            widget,
            menu: Self::build_menu(),
            project_skills_nodes,
            global_skills_nodes,
            favorite_skills_nodes: Vec::new(),
            skills_selected_path: Some(vec![0]),
            skills_expanded,
            skills_offset: 0,
            source_path,
            preview_title,
            show_toc,
            current_view: AppView::Project,
            focus: FocusPane::Tree,
            grid_layout,
            grid_state: ResizableGridWidgetState::default(),
            navbar_area: Rect::default(),
            grid_area: Rect::default(),
            tree_pane_id: 0,
            preview_pane_id,
            tree_area: Rect::default(),
            tree_content_area: Rect::default(),
            markdown_area: Rect::default(),
            markdown_inner_area: Rect::default(),
            last_move_processed: Instant::now(),
            toast_message: None,
            toast_expires_at: None,
            show_hotkeys_modal: false,
            delete_confirm_dialog: None,
            pending_preview_path: None,
            pending_preview_since: None,
            search_client: SkillsShClient::new().ok(),
            search_query: String::new(),
            search_results: Vec::new(),
            search_selected: 0,
            search_detail: None,
            search_status: "Type to search skills.sh".to_string(),
            pending_search_refresh_since: None,
            pending_search_detail_slug: None,
            pending_search_detail_since: None,
            config_text: load_config_text().unwrap_or_else(|_| "{\n}\n".to_string()),
            config_cursor_line: 0,
            config_cursor_col: 0,
            config_scroll: 0,
            config_selected_field: 0,
            config_value_cursor: 0,
            config_status: "Edit values. Ctrl+S to save.".to_string(),
            config_dirty: false,
        };

        app.rebuild_favorites_nodes();
        app.open_selected_file_immediate();
        app
    }

    fn show_toast(&mut self, message: impl Into<String>) {
        self.toast_message = Some(message.into());
        self.toast_expires_at = Some(Instant::now() + Duration::from_secs(2));
    }

    fn clear_expired_toast(&mut self) -> bool {
        if let Some(expires_at) = self.toast_expires_at {
            if Instant::now() >= expires_at {
                self.toast_message = None;
                self.toast_expires_at = None;
                return true;
            }
        }
        false
    }

    fn handle_grid_mouse(&mut self, mouse: MouseEvent) {
        let crossterm_mouse = CrosstermMouseEvent {
            kind: mouse.kind,
            column: mouse.column,
            row: mouse.row,
            modifiers: mouse.modifiers,
        };

        let mut widget =
            ResizableGridWidget::new(self.grid_layout.clone()).with_state(self.grid_state);
        widget.handle_mouse(crossterm_mouse, self.grid_area);
        self.grid_state = widget.state();
        self.grid_layout = widget.layout().clone();
    }

    fn update_pane_areas_from_grid(&mut self) {
        let panes = self.grid_layout.get_panes(self.grid_area);
        if let Some(pane) = panes.iter().find(|pane| pane.id == self.tree_pane_id) {
            self.tree_area = pane.area;
        }
        if let Some(pane) = panes.iter().find(|pane| pane.id == self.preview_pane_id) {
            self.markdown_area = pane.area;
        }
    }

    fn queue_open_selected_file(&mut self) {
        let selected_path = {
            let Some(node) = self.selected_skill_node() else {
                return;
            };
            let Some(path) = &node.skill_file else {
                return;
            };
            path.clone()
        };

        if selected_path == self.source_path {
            return;
        }

        if let Ok(source) = load_source_from_path(&selected_path) {
            self.source_path = selected_path;
            self.preview_title = extract_skill_name_from_frontmatter(&self.source_path)
                .unwrap_or_else(|| fallback_title_from_path(&self.source_path));
            self.widget = Self::build_widget(source, self.show_toc);
        }
    }

    fn open_selected_file_immediate(&mut self) {
        let Some(node) = self.selected_skill_node() else {
            return;
        };
        let Some(path) = &node.skill_file else {
            return;
        };
        if *path == self.source_path {
            return;
        }
        if let Ok(source) = load_source_from_path(path) {
            self.source_path = path.clone();
            self.preview_title = extract_skill_name_from_frontmatter(&self.source_path)
                .unwrap_or_else(|| fallback_title_from_path(&self.source_path));
            self.widget = Self::build_widget(source, self.show_toc);
        }
    }

    fn selected_skill_node(&self) -> Option<&SkillTreeNode> {
        let path = self.skills_selected_path.as_ref()?;
        let mut nodes = self.active_skills_nodes();
        let mut current = None;
        for idx in path {
            let node = nodes.get(*idx)?;
            current = Some(node);
            nodes = &node.children;
        }
        current
    }

    fn collect_visible_skill_paths(
        nodes: &[SkillTreeNode],
        expanded: &HashSet<Vec<usize>>,
        base: &mut Vec<usize>,
        out: &mut Vec<Vec<usize>>,
    ) {
        for (idx, node) in nodes.iter().enumerate() {
            base.push(idx);
            let path = base.clone();
            out.push(path.clone());
            if expanded.contains(&path) {
                Self::collect_visible_skill_paths(&node.children, expanded, base, out);
            }
            let _ = base.pop();
        }
    }

    fn visible_skill_paths(&self) -> Vec<Vec<usize>> {
        let mut out = Vec::new();
        Self::collect_visible_skill_paths(
            self.active_skills_nodes(),
            &self.skills_expanded,
            &mut Vec::new(),
            &mut out,
        );
        out
    }

    fn ensure_skill_selection_visible(&mut self) {
        let visible = self.visible_skill_paths();
        if visible.is_empty() {
            self.skills_selected_path = None;
            self.skills_offset = 0;
            return;
        }
        let valid = self
            .skills_selected_path
            .as_ref()
            .map(|p| visible.iter().any(|v| v == p))
            .unwrap_or(false);
        if !valid {
            self.skills_selected_path = Some(visible[0].clone());
        }
    }

    fn select_next_skill(&mut self) {
        let visible = self.visible_skill_paths();
        if visible.is_empty() {
            return;
        }
        let current = self
            .skills_selected_path
            .as_ref()
            .and_then(|p| visible.iter().position(|v| v == p))
            .unwrap_or(0);
        let next = (current + 1).min(visible.len().saturating_sub(1));
        self.skills_selected_path = Some(visible[next].clone());
    }

    fn select_prev_skill(&mut self) {
        let visible = self.visible_skill_paths();
        if visible.is_empty() {
            return;
        }
        let current = self
            .skills_selected_path
            .as_ref()
            .and_then(|p| visible.iter().position(|v| v == p))
            .unwrap_or(0);
        let prev = current.saturating_sub(1);
        self.skills_selected_path = Some(visible[prev].clone());
    }

    fn expand_selected_skill(&mut self) {
        if let Some(path) = self.skills_selected_path.clone() {
            self.skills_expanded.insert(path);
        }
    }

    fn collapse_selected_skill(&mut self) {
        if let Some(path) = self.skills_selected_path.clone() {
            if self.skills_expanded.contains(&path) {
                self.skills_expanded.remove(&path);
            } else if path.len() > 1 {
                let mut parent = path;
                parent.pop();
                self.skills_selected_path = Some(parent);
            }
        }
    }

    fn flush_pending_preview_if_ready(&mut self) -> bool {
        const PREVIEW_DEBOUNCE_MS: u64 = 200;

        let Some(pending_since) = self.pending_preview_since else {
            return false;
        };
        if pending_since.elapsed() < Duration::from_millis(PREVIEW_DEBOUNCE_MS) {
            return false;
        }

        let Some(selected_path) = self.pending_preview_path.take() else {
            self.pending_preview_since = None;
            return false;
        };
        self.pending_preview_since = None;

        if let Ok(source) = load_source_from_path(&selected_path) {
            self.source_path = selected_path;
            self.preview_title = extract_skill_name_from_frontmatter(&self.source_path)
                .unwrap_or_else(|| fallback_title_from_path(&self.source_path));
            self.widget = Self::build_widget(source, self.show_toc);
            return true;
        }
        false
    }

    fn is_in_rect(rect: Rect, x: u16, y: u16) -> bool {
        x >= rect.x
            && y >= rect.y
            && x < rect.x.saturating_add(rect.width)
            && y < rect.y.saturating_add(rect.height)
    }

    fn apply_translucent_shadow(frame: &mut Frame<'_>, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let bounds = frame.area();
        let start_x = area.x.max(bounds.x);
        let start_y = area.y.max(bounds.y);
        let end_x = area
            .x
            .saturating_add(area.width)
            .min(bounds.x.saturating_add(bounds.width));
        let end_y = area
            .y
            .saturating_add(area.height)
            .min(bounds.y.saturating_add(bounds.height));
        if start_x >= end_x || start_y >= end_y {
            return;
        }

        let buf = frame.buffer_mut();
        for y in start_y..end_y {
            for x in start_x..end_x {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(Style::default().bg(Color::Black).fg(Color::Black));
                }
            }
        }
    }

    fn startup_choice_message() -> &'static str {
        "Global 'skills' could not be verified as the expected skills CLI.\n\nChoose how this project should run skills commands."
    }

    fn apply_startup_choice(&mut self, selected_button: usize) {
        if selected_button == 0 {
            if let Err(message) = Self::install_global_skills_cli() {
                self.startup_dialog = Some(StartupDialogState::ChooseCommand {
                    selected_button,
                    error_message: Some(message),
                });
                return;
            }

            self.app_config.skills_command.mode = SkillsCommandMode::Global;
            let verified_version = verify_global_skills_command(&self.app_config.skills_command);
            self.app_config.skills_command.global_command_verified = verified_version.is_some();
            self.app_config.skills_command.global_command_version = verified_version;
        } else {
            self.app_config.skills_command.mode = SkillsCommandMode::Npx;
            self.app_config.skills_command.global_command_verified = false;
            self.app_config.skills_command.global_command_version = None;
        }

        if let Err(err) = persist_app_config(&self.app_config) {
            self.show_toast(format!("Failed to save config: {}", err));
            return;
        }

        self.startup_dialog = None;
    }

    fn install_global_skills_cli() -> Result<(), String> {
        let output = Command::new("npm")
            .args(["install", "-g", "skills"])
            .output()
            .map_err(|err| format!("Failed to run npm: {}", err))?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let details = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("npm exited with status {}", output.status)
        };

        let concise = details
            .lines()
            .take(4)
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(220)
            .collect::<String>();

        Err(format!(
            "Global install failed (`npm install -g skills`). {}",
            concise
        ))
    }

    fn handle_startup_dialog_key(
        &mut self,
        key_code: KeyCode,
        _modifiers: crossterm::event::KeyModifiers,
    ) -> bool {
        let mut close_dialog = false;
        let mut pending_choice: Option<usize> = None;

        if let Some(state) = self.startup_dialog.as_mut() {
            match state {
                StartupDialogState::Info { .. } => {
                    if matches!(key_code, KeyCode::Enter | KeyCode::Esc) {
                        close_dialog = true;
                    }
                }
                StartupDialogState::ChooseCommand {
                    selected_button,
                    error_message,
                } => match key_code {
                    KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('1') => {
                        *selected_button = selected_button.saturating_sub(1);
                        *error_message = None;
                    }
                    KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab | KeyCode::Char('2') => {
                        *selected_button = (*selected_button + 1).min(1);
                        *error_message = None;
                    }
                    KeyCode::Enter => pending_choice = Some(*selected_button),
                    _ => {}
                },
            }
        }

        if close_dialog {
            self.startup_dialog = None;
        }

        if let Some(choice) = pending_choice {
            self.apply_startup_choice(choice);
        }

        true
    }
}

impl CoordinatorApp for SkillPreviewApp {
    fn on_event(&mut self, event: CoordinatorEvent) -> LayoutResult<CoordinatorAction> {
        match event {
            CoordinatorEvent::Keyboard(key) => {
                if !key.is_key_down() {
                    return Ok(CoordinatorAction::Continue);
                }

                if self.startup_dialog.is_some() {
                    if key.key_code == KeyCode::Char('c')
                        && key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL)
                    {
                        return Ok(CoordinatorAction::Quit);
                    }
                    if self.handle_startup_dialog_key(key.key_code, key.modifiers) {
                        return Ok(CoordinatorAction::Redraw);
                    }
                }

                if self.delete_confirm_dialog.is_some() {
                    if self.handle_delete_confirm_key(key.key_code, key.modifiers) {
                        return Ok(CoordinatorAction::Redraw);
                    }
                }

                if key.key_code == KeyCode::Char('q') && self.current_view != AppView::Search {
                    return Ok(CoordinatorAction::Quit);
                }

                if key.key_code == KeyCode::Esc
                    && self.show_hotkeys_modal
                    && self.current_view != AppView::Search
                {
                    self.show_hotkeys_modal = false;
                    return Ok(CoordinatorAction::Redraw);
                }

                if key.key_code == KeyCode::Char('?') && self.current_view != AppView::Search {
                    self.show_hotkeys_modal = !self.show_hotkeys_modal;
                    return Ok(CoordinatorAction::Redraw);
                }

                if self.show_hotkeys_modal {
                    return Ok(CoordinatorAction::Continue);
                }

                if key.key_code == KeyCode::Char('/') && self.current_view != AppView::Search {
                    self.set_view(AppView::Search);
                    self.search_status =
                        "Global search mode (/api/search). Type to query skills.sh".to_string();
                    return Ok(CoordinatorAction::Redraw);
                }

                if key.key_code == KeyCode::Char('1') && self.current_view != AppView::Search {
                    self.set_view(AppView::Project);
                    return Ok(CoordinatorAction::Redraw);
                }

                if key.key_code == KeyCode::Char('2') && self.current_view != AppView::Search {
                    self.set_view(AppView::Global);
                    return Ok(CoordinatorAction::Redraw);
                }

                if key.key_code == KeyCode::Char('3') && self.current_view != AppView::Search {
                    self.set_view(AppView::Search);
                    return Ok(CoordinatorAction::Redraw);
                }

                if key.key_code == KeyCode::Char('4') && self.current_view != AppView::Search {
                    self.set_view(AppView::Favorites);
                    return Ok(CoordinatorAction::Redraw);
                }

                if key.key_code == KeyCode::Char('5') && self.current_view != AppView::Search {
                    self.set_view(AppView::Config);
                    return Ok(CoordinatorAction::Redraw);
                }

                if self.current_view == AppView::Search {
                    if key.key_code == KeyCode::Tab {
                        self.focus = match self.focus {
                            FocusPane::Tree => FocusPane::Preview,
                            FocusPane::Preview => FocusPane::Tree,
                        };
                        return Ok(CoordinatorAction::Redraw);
                    }

                    match key.key_code {
                        KeyCode::Char('/') => {
                            self.search_status =
                                "Global search mode (/api/search). Type to query skills.sh"
                                    .to_string();
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Esc => {
                            if !self.search_query.is_empty() {
                                self.search_query.clear();
                                self.queue_search_refresh();
                            }
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Backspace => {
                            if !self.search_query.is_empty() {
                                self.search_query.pop();
                                self.queue_search_refresh();
                                return Ok(CoordinatorAction::Redraw);
                            }
                        }
                        KeyCode::Char('r')
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            self.refresh_search_results();
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if self.search_selected > 0 {
                                self.search_selected -= 1;
                                self.queue_selected_search_detail();
                            }
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if self.search_selected + 1 < self.search_results.len() {
                                self.search_selected += 1;
                                self.queue_selected_search_detail();
                            }
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Enter => {
                            self.queue_selected_search_detail();
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Char('i')
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            self.install_selected_search_skill();
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Char(ch)
                            if !key
                                .modifiers
                                .intersects(crossterm::event::KeyModifiers::CONTROL)
                                && !key
                                    .modifiers
                                    .intersects(crossterm::event::KeyModifiers::ALT) =>
                        {
                            self.search_query.push(ch);
                            self.queue_search_refresh();
                            return Ok(CoordinatorAction::Redraw);
                        }
                        _ => {
                            return Ok(CoordinatorAction::Continue);
                        }
                    }
                }

                if self.current_view == AppView::Config {
                    let key_event = CrosstermKeyEvent {
                        code: key.key_code,
                        modifiers: key.modifiers,
                        kind: key.kind,
                        state: KeyEventState::NONE,
                    };
                    return Ok(self.handle_config_key(key_event));
                }

                if key.key_code == KeyCode::Char(']') {
                    self.show_toc = !self.show_toc;
                    if let Ok(source) = load_source_from_path(&self.source_path) {
                        self.widget = Self::build_widget(source, self.show_toc);
                    }
                    return Ok(CoordinatorAction::Redraw);
                }

                if key.key_code == KeyCode::Tab {
                    self.focus = match self.focus {
                        FocusPane::Tree => FocusPane::Preview,
                        FocusPane::Preview => FocusPane::Tree,
                    };
                    return Ok(CoordinatorAction::Redraw);
                }

                if self.focus == FocusPane::Tree {
                    self.ensure_skill_selection_visible();
                    let selected_before = self.skills_selected_path.clone();
                    match key.key_code {
                        KeyCode::Delete | KeyCode::Char('d') => self.open_delete_confirm_dialog(),
                        KeyCode::Char('u') => self.update_selected_skill(),
                        KeyCode::Char('f') => self.toggle_selected_favorite(),
                        KeyCode::Down | KeyCode::Char('j') => self.select_next_skill(),
                        KeyCode::Up | KeyCode::Char('k') => self.select_prev_skill(),
                        KeyCode::Right | KeyCode::Char('l') => self.expand_selected_skill(),
                        KeyCode::Left | KeyCode::Char('h') => self.collapse_selected_skill(),
                        _ => {}
                    }
                    let selected_after = self.skills_selected_path.clone();
                    if selected_after != selected_before || key.key_code == KeyCode::Enter {
                        self.queue_open_selected_file();
                    }
                    return Ok(CoordinatorAction::Redraw);
                }

                let key_event = CrosstermKeyEvent {
                    code: key.key_code,
                    modifiers: key.modifiers,
                    kind: key.kind,
                    state: KeyEventState::NONE,
                };

                let markdown_event = self.widget.handle_key(key_event);
                let copied_chars = match &markdown_event {
                    MarkdownEvent::Copied { text } => Some(text.chars().count()),
                    _ => None,
                };
                if let Some(copied_chars) = copied_chars {
                    self.show_toast(format!("Copied {} chars to clipboard", copied_chars));
                }
                if matches!(markdown_event, MarkdownEvent::None) {
                    Ok(CoordinatorAction::Continue)
                } else {
                    Ok(CoordinatorAction::Redraw)
                }
            }
            CoordinatorEvent::Mouse(mouse) => {
                if self.show_hotkeys_modal {
                    return Ok(CoordinatorAction::Continue);
                }

                if self.startup_dialog.is_some() {
                    return Ok(CoordinatorAction::Continue);
                }

                if self.delete_confirm_dialog.is_some() {
                    return Ok(CoordinatorAction::Continue);
                }

                if Self::is_in_rect(self.navbar_area, mouse.column, mouse.row) {
                    match mouse.kind {
                        MouseEventKind::Moved => self.menu.update_hover(mouse.column, mouse.row),
                        MouseEventKind::Down(MouseButton::Left) => {
                            if let Some(index) = self.menu.handle_click(mouse.column, mouse.row) {
                                match index {
                                    0 => self.set_view(AppView::Project),
                                    1 => self.set_view(AppView::Global),
                                    2 => self.set_view(AppView::Search),
                                    3 => self.set_view(AppView::Favorites),
                                    4 => self.set_view(AppView::Config),
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                    return Ok(CoordinatorAction::Redraw);
                }

                if self.current_view == AppView::Search {
                    let was_grid_dragging = self.grid_layout.is_dragging();
                    self.handle_grid_mouse(mouse);
                    self.update_pane_areas_from_grid();
                    if was_grid_dragging || self.grid_layout.is_dragging() {
                        return Ok(CoordinatorAction::Redraw);
                    }
                    return Ok(CoordinatorAction::Continue);
                }

                if self.current_view == AppView::Config {
                    return Ok(CoordinatorAction::Continue);
                }

                let was_grid_dragging = self.grid_layout.is_dragging();
                self.handle_grid_mouse(mouse);
                self.update_pane_areas_from_grid();

                if was_grid_dragging || self.grid_layout.is_dragging() {
                    return Ok(CoordinatorAction::Redraw);
                }

                if !Self::is_in_rect(self.markdown_inner_area, mouse.column, mouse.row) {
                    return Ok(CoordinatorAction::Continue);
                }

                let is_moved = matches!(mouse.kind, crossterm::event::MouseEventKind::Moved);
                if is_moved {
                    if self.last_move_processed.elapsed() < Duration::from_millis(24) {
                        return Ok(CoordinatorAction::Continue);
                    }
                    self.last_move_processed = Instant::now();
                }

                let mouse_event = CrosstermMouseEvent {
                    kind: mouse.kind,
                    column: mouse.column,
                    row: mouse.row,
                    modifiers: mouse.modifiers,
                };

                let markdown_event = self
                    .widget
                    .handle_mouse(mouse_event, self.markdown_inner_area);
                let copied_chars = match &markdown_event {
                    MarkdownEvent::Copied { text } => Some(text.chars().count()),
                    _ => None,
                };
                if let Some(copied_chars) = copied_chars {
                    self.show_toast(format!("Copied {} chars to clipboard", copied_chars));
                }

                if is_moved {
                    if matches!(markdown_event, MarkdownEvent::TocHoverChanged { .. }) {
                        Ok(CoordinatorAction::Redraw)
                    } else {
                        Ok(CoordinatorAction::Continue)
                    }
                } else {
                    Ok(CoordinatorAction::Redraw)
                }
            }
            CoordinatorEvent::Tick(_) => {
                let toast_changed = self.clear_expired_toast();
                let preview_changed = self.flush_pending_preview_if_ready();
                let search_refresh_changed = self.flush_pending_search_refresh_if_ready();
                let search_detail_changed = self.flush_pending_search_detail_if_ready();
                if toast_changed
                    || preview_changed
                    || search_refresh_changed
                    || search_detail_changed
                {
                    Ok(CoordinatorAction::Redraw)
                } else {
                    Ok(CoordinatorAction::Continue)
                }
            }
            CoordinatorEvent::Resize(_) => Ok(CoordinatorAction::Redraw),
            _ => Ok(CoordinatorAction::Continue),
        }
    }

    fn on_draw(&mut self, frame: &mut Frame) {
        let root_area = frame.area();
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(root_area);
        self.navbar_area = rows[0];
        self.grid_area = rows[1];
        let footer_area = rows[2];

        self.menu.render(frame, self.navbar_area);

        let footer = HotkeyFooter::new(vec![
            HotkeyItem::new("q", "quit"),
            HotkeyItem::new(
                "]",
                if self.show_toc {
                    "hide toc"
                } else {
                    "show toc"
                },
            ),
            HotkeyItem::new("?", "hotkeys"),
        ]);
        footer.render(frame, footer_area);

        if self.current_view == AppView::Search {
            let grid_widget =
                ResizableGridWidget::new(self.grid_layout.clone()).with_state(self.grid_state);
            self.grid_state = grid_widget.state();
            self.grid_layout = grid_widget.layout().clone();
            frame.render_widget(grid_widget, self.grid_area);
            self.update_pane_areas_from_grid();

            let left_border = if self.focus == FocusPane::Tree {
                Color::Blue
            } else {
                Color::White
            };
            let right_border = if self.focus == FocusPane::Preview {
                Color::Blue
            } else {
                Color::White
            };

            let left_pane = Pane::new("Search Skills")
                .with_icon(TERMINAL_ICON)
                .border_style(Style::default().fg(left_border));
            let (left_inner, _) = left_pane.render_block(frame, self.tree_area);
            let left_rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(2),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ])
                .split(left_inner);

            frame.render_widget(
                Paragraph::new(format!("Query: {}", self.search_query))
                    .style(Style::default().fg(Color::White)),
                left_rows[0],
            );
            frame.render_widget(
                Paragraph::new(self.search_status.clone())
                    .style(Style::default().fg(Color::DarkGray)),
                left_rows[1],
            );

            let items = self
                .search_results
                .iter()
                .map(|item| {
                    let skill_id = item.skill_id.as_deref().unwrap_or("<missing>");
                    let source = if item.source.is_empty() {
                        "<unknown>"
                    } else {
                        &item.source
                    };
                    ListItem::new(format!(
                        "{}  ({} installs)\n{}/{}",
                        item.name, item.installs, source, skill_id
                    ))
                })
                .collect::<Vec<_>>();

            let list = List::new(items)
                .highlight_symbol("▶ ")
                .highlight_style(Style::default().fg(Color::Black).bg(YAZI_CYAN));
            let mut list_state = ListState::default();
            if !self.search_results.is_empty() {
                list_state.select(Some(
                    self.search_selected.min(self.search_results.len() - 1),
                ));
            }
            frame.render_stateful_widget(list, left_rows[2], &mut list_state);

            let right_pane = Pane::new("Skill Details")
                .with_icon(TERMINAL_ICON)
                .border_style(Style::default().fg(right_border));
            let (right_inner, _) = right_pane.render_block(frame, self.markdown_area);

            let mut detail_lines = Vec::new();
            if let Some(detail) = &self.search_detail {
                detail_lines.push(Line::from("WEEKLY INSTALLS"));
                detail_lines.push(Line::from(detail.weekly_installs.clone()));
                detail_lines.push(Line::from(""));
                detail_lines.push(Line::from("REPOSITORY"));
                detail_lines.push(Line::from(detail.repository.clone()));
                detail_lines.push(Line::from(""));
                detail_lines.push(Line::from("GITHUB STARS"));
                detail_lines.push(Line::from(detail.github_stars.clone()));
                detail_lines.push(Line::from(""));
                detail_lines.push(Line::from("FIRST SEEN"));
                detail_lines.push(Line::from(detail.first_seen.clone()));
                detail_lines.push(Line::from(""));

                if !detail.security_audits.is_empty() {
                    detail_lines.push(Line::from("SECURITY AUDITS"));
                    for audit in &detail.security_audits {
                        detail_lines.push(Line::from(format!("{}  {}", audit.name, audit.status)));
                    }
                    detail_lines.push(Line::from(""));
                }

                detail_lines.push(Line::from("INSTALLED ON"));
                for install in &detail.installed_on {
                    detail_lines.push(Line::from(format!(
                        "{}  {}",
                        install.agent, install.installs
                    )));
                }
            } else {
                detail_lines.push(Line::from("Select a skill to load details."));
                detail_lines.push(Line::from("Use Up/Down to navigate results."));
                detail_lines.push(Line::from("Press Enter to refresh selected details."));
            }

            frame.render_widget(
                Paragraph::new(detail_lines)
                    .style(Style::default().fg(Color::White))
                    .wrap(Wrap { trim: true }),
                right_inner,
            );
        } else if self.current_view == AppView::Config {
            let pane = Pane::new("Config (.agents/skills-tui-config.json)")
                .with_icon(TERMINAL_ICON)
                .border_style(Style::default().fg(Color::White));
            let (inner, _) = pane.render_block(frame, self.grid_area);

            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(2)])
                .split(inner);
            let list_area = layout[0];
            let status_area = layout[1];

            let mut items = Vec::new();
            for i in 0..self.config_field_count() {
                let label = if i == 1 {
                    match self.app_config.skills_command.mode {
                        SkillsCommandMode::Global => "global_command",
                        SkillsCommandMode::Npx => "npx_command",
                    }
                } else {
                    Self::config_field_label(i)
                };
                let raw_value = self.config_field_value(i);
                let value = if i == self.config_selected_field {
                    self.render_config_value_with_cursor(&raw_value)
                } else {
                    raw_value
                };
                items.push(ListItem::new(format!("{:<28} {}", label, value)));
            }

            let list = List::new(items)
                .highlight_symbol("▶ ")
                .highlight_style(Style::default().fg(Color::Black).bg(YAZI_CYAN));
            let mut list_state = ListState::default();
            list_state.select(Some(self.config_selected_field));
            frame.render_stateful_widget(list, list_area, &mut list_state);

            let dirty_mark = if self.config_dirty { "*" } else { "" };
            let status = format!("{}{}", self.config_status, dirty_mark);
            let status_rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Length(1)])
                .split(status_area);
            frame.render_widget(
                Paragraph::new(status).style(Style::default().fg(Color::DarkGray)),
                status_rows[0],
            );
            let config_hotkeys = HotkeyFooter::new(vec![
                HotkeyItem::new("j/k", "field"),
                HotkeyItem::new("h/l", "cursor/toggle"),
                HotkeyItem::new("type", "edit"),
                HotkeyItem::new("ctrl+s", "save"),
            ]);
            config_hotkeys.render(frame, status_rows[1]);
        } else {
            let grid_widget =
                ResizableGridWidget::new(self.grid_layout.clone()).with_state(self.grid_state);
            self.grid_state = grid_widget.state();
            self.grid_layout = grid_widget.layout().clone();
            frame.render_widget(grid_widget, self.grid_area);

            self.update_pane_areas_from_grid();
            self.markdown_inner_area = Rect::default();

            if self.tree_area.width > 0 && self.tree_area.height > 0 {
                let tree_border = if self.focus == FocusPane::Tree {
                    Color::Blue
                } else {
                    Color::White
                };
                let tree_title = if self.current_view == AppView::Global {
                    "Global Skills"
                } else if self.current_view == AppView::Favorites {
                    "Favorite Skills"
                } else {
                    "Skills"
                };
                let tree_pane = Pane::new(tree_title)
                    .with_icon(TERMINAL_ICON)
                    .border_style(Style::default().fg(tree_border));
                let (tree_inner, _) = tree_pane.render_block(frame, self.tree_area);
                let tree_rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(1)])
                    .split(tree_inner);
                self.tree_content_area = tree_rows[0];

                self.ensure_skill_selection_visible();
                let visible = self.visible_skill_paths();
                let max_rows = self.tree_content_area.height as usize;
                if let Some(selected) = &self.skills_selected_path {
                    if let Some(idx) = visible.iter().position(|p| p == selected) {
                        if idx < self.skills_offset {
                            self.skills_offset = idx;
                        } else if idx >= self.skills_offset.saturating_add(max_rows) {
                            self.skills_offset = idx.saturating_sub(max_rows.saturating_sub(1));
                        }
                    }
                }

                let mut lines = Vec::new();
                let line_width = self.tree_content_area.width as usize;
                for path in visible.iter().skip(self.skills_offset).take(max_rows) {
                    if let Some(node) = skill_node_at_path(self.active_skills_nodes(), path) {
                        let depth = path.len().saturating_sub(1);
                        let indent = "  ".repeat(depth);
                        let is_selected = self
                            .skills_selected_path
                            .as_ref()
                            .map(|p| p == path)
                            .unwrap_or(false);
                        let is_expanded = self.skills_expanded.contains(path);
                        let disclosure = if node.children.is_empty() {
                            " "
                        } else if is_expanded {
                            "▾"
                        } else {
                            "▸"
                        };
                        let row_style = if is_selected {
                            Style::default().fg(Color::Black).bg(YAZI_CYAN)
                        } else {
                            Style::default().fg(Color::White)
                        };
                        let favorite_marker = if self.is_favorite_node(node) {
                            "*"
                        } else {
                            " "
                        };
                        let text = format!(
                            "{}{} {} {} {}",
                            indent, disclosure, favorite_marker, TERMINAL_ICON, node.display_name
                        );
                        let padded = if line_width > 0 {
                            format!("{:<width$}", text, width = line_width)
                        } else {
                            text
                        };
                        lines.push(Line::from(Span::styled(padded, row_style)));
                    }
                }

                frame.render_widget(Paragraph::new(lines), self.tree_content_area);
                frame.render_widget(
                    Paragraph::new("d delete  f favorite  u update")
                        .style(Style::default().fg(Color::DarkGray)),
                    tree_rows[1],
                );
            }

            if self.markdown_area.width > 0 && self.markdown_area.height > 0 {
                let preview_border = if self.focus == FocusPane::Preview {
                    Color::Blue
                } else {
                    Color::White
                };
                let preview_title = if self.focus == FocusPane::Preview {
                    preview_relative_to_skills(&self.source_path)
                } else {
                    self.preview_title.clone()
                };
                let preview_pane = Pane::new(preview_title)
                    .with_icon(TERMINAL_ICON)
                    .border_style(Style::default().fg(preview_border));
                let (preview_inner, _) = preview_pane.render_block(frame, self.markdown_area);
                self.markdown_inner_area = preview_inner;
                if preview_inner.width > 0 && preview_inner.height > 0 {
                    frame.render_widget(&mut self.widget, preview_inner);
                }
            }

            if let Some(message) = &self.toast_message {
                if self.markdown_inner_area.height > 0 {
                    let toast_width =
                        (message.chars().count() as u16 + 2).min(self.markdown_inner_area.width);
                    let toast_area = Rect {
                        x: self.markdown_inner_area.x
                            + self.markdown_inner_area.width.saturating_sub(toast_width) / 2,
                        y: self.markdown_inner_area.y
                            + self.markdown_inner_area.height.saturating_sub(1),
                        width: toast_width,
                        height: 1,
                    };
                    frame.render_widget(
                        Paragraph::new(Line::from(format!(" {}", message)))
                            .style(Style::default().fg(Color::Black).bg(Color::LightGreen)),
                        toast_area,
                    );
                }
            }
        }

        if self.show_hotkeys_modal {
            let modal_width = root_area.width.min(68);
            let modal_height = root_area.height.min(18);
            let modal_area = Rect {
                x: root_area.x + (root_area.width.saturating_sub(modal_width)) / 2,
                y: root_area.y + (root_area.height.saturating_sub(modal_height)) / 2,
                width: modal_width,
                height: modal_height,
            };

            frame.render_widget(Clear, modal_area);

            let hotkeys_pane = Pane::new("Hotkeys")
                .with_icon(TERMINAL_ICON)
                .border_style(Style::default().fg(Color::Green));
            let hotkeys = vec![
                Line::from("Global"),
                Line::from(" q           quit"),
                Line::from(" ]           toggle toc"),
                Line::from(" ? / esc     open/close hotkeys"),
                Line::from(" 1 / 2 / 3 / 4 / 5 switch Project/Global/Search/Favorites/Config"),
                Line::from(" d/delete    remove skill"),
                Line::from(" u           update skill"),
                Line::from(" f           toggle favorite"),
                Line::from(" tab         switch tree/preview focus"),
                Line::from(""),
                Line::from("File Tree"),
                Line::from(" up/down     move selection"),
                Line::from(" enter       expand/collapse or open file"),
                Line::from(" /           filter tree"),
                Line::from(""),
                Line::from("Markdown / Config"),
                Line::from(" mouse wheel scroll"),
                Line::from(" drag/select copy (shows toast)"),
                Line::from(" Ctrl+S save config (Config tab)"),
            ];
            hotkeys_pane.render_paragraph(frame, modal_area, hotkeys);
        }

        if let Some(state) = self.startup_dialog.as_mut() {
            let focused_pane_border = match self.focus {
                FocusPane::Tree => Color::Blue,
                FocusPane::Preview => Color::Blue,
            };
            match state {
                StartupDialogState::Info { title, message } => {
                    let mut dialog = Dialog::success(title, message)
                        .buttons(vec!["OK"])
                        .width_percent(0.62)
                        .height_percent(0.38)
                        .border_color(focused_pane_border)
                        .overlay(true)
                        .footer("Press Enter to continue");
                    frame.render_widget(DialogWidget::new(&mut dialog), root_area);
                }
                StartupDialogState::ChooseCommand {
                    selected_button,
                    error_message,
                } => {
                    let modal_width = root_area.width.saturating_mul(58) / 100;
                    let modal_height = root_area.height.saturating_mul(34) / 100;
                    let modal_area = Rect {
                        x: root_area.x + (root_area.width.saturating_sub(modal_width)) / 2,
                        y: root_area.y + (root_area.height.saturating_sub(modal_height)) / 2,
                        width: modal_width,
                        height: modal_height,
                    };

                    let right_shadow = Rect {
                        x: modal_area.x.saturating_add(modal_area.width),
                        y: modal_area.y.saturating_add(1),
                        width: 1,
                        height: modal_area.height.saturating_sub(1),
                    };
                    Self::apply_translucent_shadow(frame, right_shadow);

                    let bottom_shadow = Rect {
                        x: modal_area.x.saturating_add(1),
                        y: modal_area.y.saturating_add(modal_area.height),
                        width: modal_area.width.saturating_sub(1),
                        height: 1,
                    };
                    Self::apply_translucent_shadow(frame, bottom_shadow);

                    frame.render_widget(Clear, modal_area);

                    let modal_pane = Pane::new("Configuration")
                        .border_style(Style::default().fg(focused_pane_border));
                    let (inner, _) = modal_pane.render_block(frame, modal_area);

                    let inner_with_vpad = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(1),
                            Constraint::Min(0),
                            Constraint::Length(1),
                        ])
                        .split(inner);
                    let inner_with_padding = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([
                            Constraint::Length(2),
                            Constraint::Min(0),
                            Constraint::Length(2),
                        ])
                        .split(inner_with_vpad[1]);
                    let content_area = inner_with_padding[1];

                    let rows = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Min(3),
                            Constraint::Length(if error_message.is_some() { 3 } else { 0 }),
                            Constraint::Length(1),
                            Constraint::Length(2),
                            Constraint::Min(0),
                        ])
                        .split(content_area);

                    frame.render_widget(
                        Paragraph::new(Self::startup_choice_message())
                            .style(Style::default().fg(Color::White))
                            .wrap(Wrap { trim: true }),
                        rows[0],
                    );

                    if let Some(error_message) = error_message {
                        frame.render_widget(
                            Paragraph::new(error_message.as_str())
                                .style(Style::default().fg(Color::LightRed))
                                .wrap(Wrap { trim: true }),
                            rows[1],
                        );
                    }

                    let items = vec![
                        ListItem::new("1) Install Global Skills"),
                        ListItem::new("2) Use npx skills"),
                    ];
                    let list = List::new(items)
                        .highlight_symbol("  ")
                        .highlight_style(Style::default().fg(Color::Black).bg(YAZI_CYAN));
                    let mut list_state = ListState::default();
                    list_state.select(Some(*selected_button));
                    frame.render_stateful_widget(list, rows[3], &mut list_state);
                }
            }
        }

        if let Some(state) = self.delete_confirm_dialog.as_mut() {
            let message = format!(
                "Delete skill '{}' ?\n\nThis action cannot be undone.",
                state.skill_name
            );
            let mut dialog = Dialog::confirm("Configuration", &message)
                .buttons(vec!["Delete", "Cancel"])
                .width_percent(0.52)
                .height_percent(0.32)
                .overlay(true)
                .footer("Press d again to delete, Esc to cancel");
            dialog.selected_button = state.selected_button;
            frame.render_widget(DialogWidget::new(&mut dialog), root_area);
            state.selected_button = dialog.selected_button;
        }
    }
}

fn load_source_from_path(path: impl AsRef<Path>) -> io::Result<SourceState> {
    let path = path.as_ref();

    let mut source = SourceState::default();
    source.set_source_file(path)?;
    Ok(source)
}

fn fallback_title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Preview")
        .to_string()
}

fn preview_relative_to_skills(path: &Path) -> String {
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

fn extract_skill_name_from_frontmatter(path: &Path) -> Option<String> {
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

fn skill_node_at_path<'a>(nodes: &'a [SkillTreeNode], path: &[usize]) -> Option<&'a SkillTreeNode> {
    let mut current_nodes = nodes;
    let mut current_node: Option<&SkillTreeNode> = None;
    for idx in path {
        let node = current_nodes.get(*idx)?;
        current_node = Some(node);
        current_nodes = &node.children;
    }
    current_node
}

fn collect_expanded_skill_paths(
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

fn load_project_skill_hierarchy() -> io::Result<Vec<SkillTreeNode>> {
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

fn global_agents_skill_root() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("~"));
    home.join(".agents/skills")
}

fn provider_global_skill_roots() -> Vec<(String, PathBuf)> {
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

fn load_global_skill_hierarchy() -> io::Result<Vec<SkillTreeNode>> {
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

fn first_skill_file(nodes: &[SkillTreeNode]) -> Option<PathBuf> {
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

fn app_config_path() -> PathBuf {
    PathBuf::from(APP_CONFIG_PATH)
}

fn load_config_text() -> io::Result<String> {
    let path = app_config_path();
    if !path.exists() {
        return Ok("{\n}\n".to_string());
    }
    fs::read_to_string(path)
}

fn save_config_text(text: &str) -> io::Result<()> {
    let path = app_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

fn write_app_config(path: &Path, config: &AppConfig) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_string_pretty(config)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    fs::write(path, format!("{}\n", serialized))
}

fn load_app_config() -> io::Result<AppConfig> {
    let path = app_config_path();
    let raw = fs::read_to_string(&path)?;
    let config: AppConfig = serde_json::from_str(&raw)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    Ok(config)
}

fn persist_app_config(config: &AppConfig) -> io::Result<()> {
    write_app_config(&app_config_path(), config)
}

fn run_command_for_output(bin: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(bin).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return Some(stdout);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return Some(stderr);
    }

    None
}

fn looks_like_semverish(text: &str) -> bool {
    let compact = text
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '.' || *ch == '-')
        .collect::<String>();
    compact.chars().any(|ch| ch.is_ascii_digit()) && compact.contains('.')
}

fn verify_global_skills_command(cfg: &SkillsCommandConfig) -> Option<String> {
    let identity = cfg.expected_identity_substring.to_ascii_lowercase();

    for args in [["--version"].as_slice(), ["version"].as_slice()] {
        if let Some(output) = run_command_for_output(&cfg.global_command, args) {
            let lowered = output.to_ascii_lowercase();
            if lowered.contains(&identity) || looks_like_semverish(&output) {
                return Some(output.lines().next().unwrap_or("skills").trim().to_string());
            }
        }
    }

    None
}

fn initialize_skills_command_config() -> io::Result<StartupConfigOutcome> {
    let path = app_config_path();
    if path.exists() {
        return Ok(StartupConfigOutcome {
            config: load_app_config()?,
            startup_dialog: None,
        });
    }

    let mut config = AppConfig::default();
    let verified_version = verify_global_skills_command(&config.skills_command);
    config.skills_command.global_command_verified = verified_version.is_some();
    config.skills_command.global_command_version = verified_version.clone();
    if config.skills_command.global_command_verified {
        config.skills_command.mode = SkillsCommandMode::Global;
        persist_app_config(&config)?;
    }

    let startup_dialog = if config.skills_command.global_command_verified {
        Some(StartupDialogState::Info {
            title: "Configuration".to_string(),
            message: format!(
                "Created {} and verified global '{}' ({})",
                APP_CONFIG_PATH,
                config.skills_command.global_command,
                verified_version.unwrap_or_else(|| "version unknown".to_string())
            ),
        })
    } else {
        Some(StartupDialogState::ChooseCommand {
            selected_button: 0,
            error_message: None,
        })
    };

    Ok(StartupConfigOutcome {
        config,
        startup_dialog,
    })
}

fn main() -> io::Result<()> {
    let startup = initialize_skills_command_config()?;

    let project_skills_nodes = load_project_skill_hierarchy()?;
    let global_skills_nodes = load_global_skill_hierarchy()?;

    let source_path = first_skill_file(&project_skills_nodes)
        .or_else(|| first_skill_file(&global_skills_nodes))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SKILL_PATH));
    if !source_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No SKILL.md found in project (.agents) or supported global skill directories",
        ));
    }
    let source = load_source_from_path(&source_path)?;
    let mut app = SkillPreviewApp::new(
        startup.config,
        startup.startup_dialog,
        source_path,
        source,
        project_skills_nodes,
        global_skills_nodes,
    );
    if app.project_skills_nodes.is_empty() && !app.global_skills_nodes.is_empty() {
        app.set_view(AppView::Global);
    }
    let config = RunnerConfig {
        tick_rate: Duration::from_millis(250),
        ..RunnerConfig::default()
    };
    run(app, config)
}
